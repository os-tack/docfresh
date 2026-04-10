mod audit;
mod coverage;
mod git;
mod manifest;
mod report;

use crate::manifest::{Manifest, Page, SourceRepo, Status, VerifiedAt};
use crate::report::Format;
use chrono::Utc;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process;

#[derive(Parser)]
#[command(
    name = "docfresh",
    version,
    about = "Track documentation freshness against source code"
)]
struct Cli {
    /// Path to site-manifest.json
    #[arg(short, long, default_value = "site-manifest.json")]
    manifest: PathBuf,

    /// Override source repo path
    #[arg(long, env = "DOCFRESH_SOURCE_REPO")]
    source_repo: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Bootstrap a manifest by scanning page files
    Init {
        /// Source repo path
        #[arg(long)]
        source_repo: Option<String>,

        /// Glob pattern for page files
        #[arg(long, default_value = "src/pages/**/*.astro")]
        pattern: String,

        /// Base directory to strip from file paths for route generation
        #[arg(long, default_value = "src/pages")]
        base_dir: String,
    },

    /// Check all pages for staleness
    Audit {
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show documentation coverage vs source files
    Coverage {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Additional file patterns to scan
        #[arg(long)]
        scan: Vec<String>,
    },

    /// Mark page(s) as verified at current source HEAD
    Verify {
        /// Route to verify, or --all
        route: Option<String>,

        /// Verify all pages
        #[arg(long)]
        all: bool,

        /// Override SHA (default: source repo HEAD)
        #[arg(long)]
        sha: Option<String>,
    },

    /// Show what changed in sources since last verification
    Diff {
        /// Page route
        route: String,
    },

    /// Show status of all pages
    Status {
        /// Specific route
        route: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Output as markdown
        #[arg(long)]
        markdown: bool,
    },

    /// Full audit + coverage report
    Report {
        /// Output format
        #[arg(long, default_value = "text")]
        format: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Init {
            source_repo,
            pattern,
            base_dir,
        } => cmd_init(&cli.manifest, source_repo.as_deref(), pattern, base_dir),
        Commands::Audit { tag, json } => cmd_audit(&cli, tag.as_deref(), *json),
        Commands::Coverage { json, scan } => cmd_coverage(&cli, *json, scan),
        Commands::Verify { route, all, sha } => {
            cmd_verify(&cli, route.as_deref(), *all, sha.as_deref())
        }
        Commands::Diff { route } => cmd_diff(&cli, route),
        Commands::Status {
            route,
            json,
            markdown,
        } => cmd_status(&cli, route.as_deref(), *json, *markdown),
        Commands::Report { format } => cmd_report(&cli, format),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        process::exit(2);
    }
}

fn cmd_init(
    manifest_path: &Path,
    source_repo: Option<&str>,
    pattern: &str,
    base_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if manifest_path.exists() {
        return Err(format!(
            "'{}' already exists. Delete it first to re-initialize.",
            manifest_path.display()
        )
        .into());
    }

    let pages = scan_pages(pattern, base_dir)?;
    println!("Found {} pages", pages.len());

    let manifest = Manifest {
        version: 1,
        source_repo: SourceRepo {
            path: source_repo.unwrap_or("../source").to_string(),
            remote: None,
            default_branch: "main".to_string(),
        },
        pages,
    };

    manifest.save(manifest_path)?;
    println!("Created {}", manifest_path.display());
    Ok(())
}

fn scan_pages(pattern: &str, base_dir: &str) -> Result<Vec<Page>, Box<dyn std::error::Error>> {
    let mut pages = Vec::new();

    for entry in glob::glob(pattern).map_err(|e| format!("invalid glob pattern: {e}"))? {
        let path = entry?;
        let path_str = path.to_string_lossy().to_string();

        let route = path_str
            .strip_prefix(base_dir)
            .unwrap_or(&path_str)
            .trim_start_matches('/')
            .replace(".astro", "")
            .replace(".md", "")
            .replace(".html", "");

        let route = if route == "index" {
            "/".to_string()
        } else if route.ends_with("/index") {
            format!("/{}", route.strip_suffix("/index").unwrap_or(&route))
        } else {
            format!("/{route}")
        };

        let title = extract_title(&path).unwrap_or_default();

        pages.push(Page {
            route,
            file: Some(path_str),
            title,
            tags: vec![],
            sources: vec![],
            related: vec![],
            verified_at: None,
            status: Status::Unverified,
        });
    }

    pages.sort_by(|a, b| a.route.cmp(&b.route));
    Ok(pages)
}

fn extract_title(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("title=") {
            let title = rest.trim_matches(|c| c == '"' || c == '\'' || c == '{' || c == '}');
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
        if let Some(rest) = trimmed.strip_prefix("title:") {
            let title = rest.trim().trim_matches(|c| c == '"' || c == '\'');
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

fn cmd_audit(cli: &Cli, tag: Option<&str>, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = Manifest::load(&cli.manifest)?;
    let manifest_dir = cli.manifest.parent().unwrap_or(Path::new("."));
    let source_repo = resolve_source_repo(cli, &manifest, manifest_dir)?;

    let summary = audit::audit_all(&manifest, &source_repo, tag);
    let format = if json { Format::Json } else { Format::Text };
    println!("{}", report::format_audit(&summary, format));

    let has_stale = summary
        .results
        .iter()
        .any(|r| r.new_status == Status::Stale);
    if has_stale {
        process::exit(1);
    }
    Ok(())
}

fn cmd_coverage(
    cli: &Cli,
    json: bool,
    extra_patterns: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = Manifest::load(&cli.manifest)?;
    let manifest_dir = cli.manifest.parent().unwrap_or(Path::new("."));
    let source_repo = resolve_source_repo(cli, &manifest, manifest_dir)?;

    let mut patterns = coverage::default_scan_patterns();
    let extra_refs: Vec<&str> = extra_patterns
        .iter()
        .map(std::string::String::as_str)
        .collect();
    patterns.extend(extra_refs);

    let report = coverage::compute_coverage(&manifest, &source_repo, &patterns)?;
    let format = if json { Format::Json } else { Format::Text };
    println!("{}", report::format_coverage(&report, format));
    Ok(())
}

fn cmd_verify(
    cli: &Cli,
    route: Option<&str>,
    all: bool,
    sha_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = Manifest::load(&cli.manifest)?;
    let manifest_dir = cli.manifest.parent().unwrap_or(Path::new("."));
    let source_repo = resolve_source_repo(cli, &manifest, manifest_dir)?;

    let sha = match sha_override {
        Some(s) => s.to_string(),
        None => git::head_sha(&source_repo)?,
    };
    let timestamp = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    if all {
        let count = manifest
            .pages
            .iter()
            .filter(|p| p.status != Status::Missing)
            .count();
        for page in &mut manifest.pages {
            if page.status != Status::Missing {
                page.verified_at = Some(VerifiedAt {
                    sha: sha.clone(),
                    timestamp: timestamp.clone(),
                });
                page.status = Status::Current;
            }
        }
        manifest.save(&cli.manifest)?;
        println!("Verified {count} pages at {sha}");
    } else if let Some(route) = route {
        let idx = manifest
            .find_page(route)
            .ok_or_else(|| format!("page '{route}' not found in manifest"))?;
        manifest.pages[idx].verified_at = Some(VerifiedAt {
            sha: sha.clone(),
            timestamp: timestamp.clone(),
        });
        manifest.pages[idx].status = Status::Current;
        manifest.save(&cli.manifest)?;
        println!("Verified {route} at {sha}");
    } else {
        return Err("specify a route or --all".into());
    }

    Ok(())
}

fn cmd_diff(cli: &Cli, route: &str) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = Manifest::load(&cli.manifest)?;
    let manifest_dir = cli.manifest.parent().unwrap_or(Path::new("."));
    let source_repo = resolve_source_repo(cli, &manifest, manifest_dir)?;

    let idx = manifest
        .find_page(route)
        .ok_or_else(|| format!("page '{route}' not found in manifest"))?;
    let page = &manifest.pages[idx];

    let verified = page
        .verified_at
        .as_ref()
        .ok_or_else(|| format!("page '{route}' has never been verified"))?;

    let source_paths: Vec<&str> = page.sources.iter().map(|s| s.path.as_str()).collect();
    let log_entries = git::file_log(&source_repo, &verified.sha, &source_paths)?;
    let changed_files = git::changed_files_between(&source_repo, &verified.sha, &source_paths)?;

    println!(
        "{}",
        report::format_diff(route, &log_entries, &changed_files)
    );
    Ok(())
}

fn cmd_status(
    cli: &Cli,
    route: Option<&str>,
    json: bool,
    markdown: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = Manifest::load(&cli.manifest)?;

    if let Some(route) = route {
        let idx = manifest
            .find_page(route)
            .ok_or_else(|| format!("page '{route}' not found in manifest"))?;
        let page = &manifest.pages[idx];
        if json {
            println!("{}", serde_json::to_string_pretty(page)?);
        } else {
            println!("Route:    {}", page.route);
            println!("Title:    {}", page.title);
            println!("Status:   {}", page.status);
            println!("File:     {}", page.file.as_deref().unwrap_or("-"));
            println!("Sources:  {}", page.sources.len());
            for src in &page.sources {
                let sections = if src.sections.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", src.sections.join(", "))
                };
                println!("  - {}{}", src.path, sections);
            }
            println!("Related:  {}", page.related.join(", "));
            println!("Tags:     {}", page.tags.join(", "));
            if let Some(v) = &page.verified_at {
                println!("Verified: {} ({})", v.sha, v.timestamp);
            } else {
                println!("Verified: never");
            }
        }
    } else {
        let format = if json {
            Format::Json
        } else if markdown {
            Format::Markdown
        } else {
            Format::Text
        };
        println!("{}", report::format_status_table(&manifest, format));
    }
    Ok(())
}

fn cmd_report(cli: &Cli, format_str: &str) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = Manifest::load(&cli.manifest)?;
    let manifest_dir = cli.manifest.parent().unwrap_or(Path::new("."));
    let source_repo = resolve_source_repo(cli, &manifest, manifest_dir)?;

    let format = match format_str {
        "json" => Format::Json,
        "markdown" | "md" => Format::Markdown,
        _ => Format::Text,
    };

    let summary = audit::audit_all(&manifest, &source_repo, None);
    println!("{}", report::format_audit(&summary, format));
    println!();

    let patterns = coverage::default_scan_patterns();
    let coverage_report = coverage::compute_coverage(&manifest, &source_repo, &patterns)?;
    println!("{}", report::format_coverage(&coverage_report, format));

    let has_stale = summary
        .results
        .iter()
        .any(|r| r.new_status == Status::Stale);
    if has_stale {
        process::exit(1);
    }
    Ok(())
}

fn resolve_source_repo(
    cli: &Cli,
    manifest: &Manifest,
    manifest_dir: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(override_path) = &cli.source_repo {
        let path = override_path.canonicalize().map_err(|_| {
            format!(
                "source repo override path '{}' not found",
                override_path.display()
            )
        })?;
        return Ok(path);
    }
    manifest.resolve_source_repo(manifest_dir)
}
