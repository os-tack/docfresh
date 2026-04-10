mod audit;
mod config;
mod coverage;
mod git;
mod manifest;
mod presets;
mod report;
mod suggest;

use crate::config::Config;
use crate::manifest::{Manifest, Page, Source, SourceRepo, Status, VerifiedAt};
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

        /// Site framework preset (auto-detected if omitted)
        #[arg(long)]
        site: Option<String>,

        /// Source language preset (auto-detected if omitted)
        #[arg(long)]
        lang: Option<String>,

        /// Override glob pattern for page files
        #[arg(long)]
        pattern: Option<String>,

        /// Override base directory for route generation
        #[arg(long)]
        base_dir: Option<String>,

        /// List available presets
        #[arg(long)]
        list_presets: bool,
    },

    /// Check all pages for staleness
    Audit {
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,

        /// Maximum stale pages before failure (overrides config)
        #[arg(long)]
        max_stale: Option<usize>,

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

        /// Minimum coverage percentage before failure (overrides config)
        #[arg(long)]
        min_coverage: Option<usize>,

        /// Fail if any source file is unmapped and not excluded
        #[arg(long)]
        fail_on_unmapped: bool,
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

    /// Suggest source mappings for unmapped files
    Suggest {
        /// Minimum confidence threshold (0.0-1.0)
        #[arg(long, default_value = "0.2")]
        min_confidence: f64,

        /// Auto-apply suggestions above this confidence
        #[arg(long)]
        apply: Option<f64>,

        /// Only suggest for a specific source file
        #[arg(long)]
        file: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Add a source mapping to a page
    Map {
        /// Page route
        route: String,

        /// Source file path (relative to source repo)
        source: String,

        /// Optional section markers
        #[arg(long)]
        sections: Vec<String>,
    },

    /// Run audit + coverage with threshold enforcement (designed for CI)
    Ci {
        /// Maximum stale pages before failure (overrides config)
        #[arg(long)]
        max_stale: Option<usize>,

        /// Minimum coverage percentage (overrides config)
        #[arg(long)]
        min_coverage: Option<usize>,

        /// Fail if any unmapped source files exist (overrides config)
        #[arg(long)]
        fail_on_unmapped: Option<bool>,

        /// Output format: text, markdown, json (overrides config)
        #[arg(long)]
        format: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Init {
            source_repo,
            site,
            lang,
            pattern,
            base_dir,
            list_presets,
        } => cmd_init(
            &cli.manifest,
            source_repo.as_deref(),
            site.as_deref(),
            lang.as_deref(),
            pattern.as_deref(),
            base_dir.as_deref(),
            *list_presets,
        ),
        Commands::Audit {
            tag,
            max_stale,
            json,
        } => cmd_audit(&cli, tag.as_deref(), *max_stale, *json),
        Commands::Coverage {
            json,
            scan,
            min_coverage,
            fail_on_unmapped,
        } => cmd_coverage(&cli, *json, scan, *min_coverage, *fail_on_unmapped),
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
        Commands::Suggest {
            min_confidence,
            apply,
            file,
            json,
        } => cmd_suggest(&cli, *min_confidence, *apply, file.as_deref(), *json),
        Commands::Map {
            route,
            source,
            sections,
        } => cmd_map(&cli, route, source, sections),
        Commands::Ci {
            max_stale,
            min_coverage,
            fail_on_unmapped,
            format,
        } => cmd_ci(
            &cli,
            *max_stale,
            *min_coverage,
            *fail_on_unmapped,
            format.as_deref(),
        ),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        process::exit(2);
    }
}

fn cmd_init(
    manifest_path: &Path,
    source_repo: Option<&str>,
    site_preset_name: Option<&str>,
    lang_preset_name: Option<&str>,
    pattern_override: Option<&str>,
    base_dir_override: Option<&str>,
    list_presets: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if list_presets {
        println!("Site frameworks:");
        for p in presets::all_site_presets() {
            println!(
                "  {:<16} pages: {:<35} base: {}",
                p.name,
                p.page_patterns.join(", "),
                p.base_dir
            );
        }
        println!("\nSource languages:");
        for p in presets::all_source_presets() {
            println!("  {:<16} scan: {}", p.name, p.scan_patterns.join(", "));
        }
        return Ok(());
    }

    if manifest_path.exists() {
        return Err(format!(
            "'{}' already exists. Delete it first to re-initialize.",
            manifest_path.display()
        )
        .into());
    }

    let cwd = std::env::current_dir()?;

    // Resolve site preset
    let site_preset = if let Some(name) = site_preset_name {
        presets::find_site_preset(name).ok_or_else(|| {
            format!("unknown site preset '{name}'. Use --list-presets to see available presets.")
        })?
    } else {
        let detected = presets::detect_site_framework(&cwd);
        if let Some(p) = detected {
            println!("Detected site framework: {}", p.name);
            p
        } else {
            println!("No site framework detected, using markdown defaults.");
            println!("Use --site <preset> to specify. --list-presets shows options.");
            &presets::MARKDOWN
        }
    };

    // Use overrides or preset defaults
    let pattern = pattern_override.unwrap_or(site_preset.page_patterns[0]);
    let base_dir = base_dir_override.unwrap_or(site_preset.base_dir);

    // Scan pages using all preset patterns if no override
    let pages = if pattern_override.is_some() {
        scan_pages(pattern, base_dir, site_preset)?
    } else {
        let mut all_pages = Vec::new();
        for pat in site_preset.page_patterns {
            if let Ok(mut pages) = scan_pages(pat, base_dir, site_preset) {
                all_pages.append(&mut pages);
            }
        }
        all_pages.sort_by(|a, b| a.route.cmp(&b.route));
        all_pages.dedup_by(|a, b| a.route == b.route);
        all_pages
    };

    println!("Found {} pages", pages.len());

    // Detect source language for scan_patterns hint
    let source_path = source_repo.unwrap_or("../source");
    let resolved_source = cwd.join(source_path);
    let detected_lang = lang_preset_name
        .and_then(presets::find_source_preset)
        .or_else(|| {
            if resolved_source.exists() {
                let detected = presets::detect_source_language(&resolved_source);
                if let Some(p) = detected {
                    println!("Detected source language: {}", p.name);
                }
                detected
            } else {
                None
            }
        });

    let _ = detected_lang; // Used in future for scan_patterns in manifest

    let manifest = Manifest {
        version: 1,
        source_repo: SourceRepo {
            path: source_path.to_string(),
            remote: None,
            default_branch: "main".to_string(),
        },
        exclude_patterns: vec![],
        pages,
    };

    manifest.save(manifest_path)?;
    println!("Created {}", manifest_path.display());
    Ok(())
}

fn scan_pages(
    pattern: &str,
    base_dir: &str,
    preset: &presets::SitePreset,
) -> Result<Vec<Page>, Box<dyn std::error::Error>> {
    let mut pages = Vec::new();

    for entry in glob::glob(pattern).map_err(|e| format!("invalid glob pattern: {e}"))? {
        let path = entry?;
        // Normalize to forward slashes for cross-platform route consistency
        let path_str = path.to_string_lossy().replace('\\', "/");

        // Strip base dir and extension to get route
        let relative = path_str
            .strip_prefix(base_dir)
            .unwrap_or(&path_str)
            .trim_start_matches('/');
        let route = presets::strip_page_extension(relative, preset);

        // Handle Next.js app router: page.tsx → parent directory is the route
        let route = if preset.name == "nextjs-app" {
            route.replace("/page", "")
        } else {
            route
        };

        let route = if route.is_empty() || route == "index" {
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
    let mut in_frontmatter = false;

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // YAML front matter (Hugo, Jekyll, Docusaurus, VitePress, MkDocs)
        if i == 0 && trimmed == "---" {
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if trimmed == "---" {
                in_frontmatter = false;
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("title:") {
                let title = rest.trim().trim_matches(|c| c == '"' || c == '\'');
                if !title.is_empty() {
                    return Some(title.to_string());
                }
            }
            continue;
        }

        // Astro/JSX component props
        if let Some(rest) = trimmed.strip_prefix("title=") {
            let title = rest.trim_matches(|c| c == '"' || c == '\'' || c == '{' || c == '}');
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }

        // TOML front matter (Hugo alternative)
        if i == 0 && trimmed == "+++" {
            // TOML front matter — scan for title = "..."
            for inner in content.lines().skip(1) {
                let t = inner.trim();
                if t == "+++" {
                    break;
                }
                if let Some(rest) = t.strip_prefix("title") {
                    let rest = rest.trim().strip_prefix('=').unwrap_or(rest).trim();
                    let title = rest.trim_matches(|c| c == '"' || c == '\'');
                    if !title.is_empty() {
                        return Some(title.to_string());
                    }
                }
            }
            return None;
        }

        // Markdown H1 heading as fallback
        if let Some(heading) = trimmed.strip_prefix("# ") {
            return Some(heading.trim().to_string());
        }
    }
    None
}

fn cmd_audit(
    cli: &Cli,
    tag: Option<&str>,
    max_stale_override: Option<usize>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = Manifest::load(&cli.manifest)?;
    let manifest_dir = cli.manifest.parent().unwrap_or(Path::new("."));
    let source_repo = resolve_source_repo(cli, &manifest, manifest_dir)?;
    let config = Config::load(manifest_dir);

    let summary = audit::audit_all(&manifest, &source_repo, tag);
    let format = if json { Format::Json } else { Format::Text };
    println!("{}", report::format_audit(&summary, format));

    let stale_count = summary
        .results
        .iter()
        .filter(|r| r.new_status == Status::Stale)
        .count();
    let max_stale = max_stale_override.unwrap_or(config.ci.max_stale);
    if stale_count > max_stale {
        process::exit(1);
    }
    Ok(())
}

fn cmd_coverage(
    cli: &Cli,
    json: bool,
    extra_patterns: &[String],
    min_coverage_override: Option<usize>,
    fail_on_unmapped: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = Manifest::load(&cli.manifest)?;
    let manifest_dir = cli.manifest.parent().unwrap_or(Path::new("."));
    let source_repo = resolve_source_repo(cli, &manifest, manifest_dir)?;
    let config = Config::load(manifest_dir);

    let defaults = coverage::default_scan_patterns();
    let mut patterns = config.scan_patterns(&defaults);
    let extra_refs: Vec<&str> = extra_patterns
        .iter()
        .map(std::string::String::as_str)
        .collect();
    patterns.extend(extra_refs);

    let cov_report = coverage::compute_coverage(&manifest, &source_repo, &patterns)?;
    let format = if json { Format::Json } else { Format::Text };
    println!("{}", report::format_coverage(&cov_report, format));

    let mut failed = false;

    // Check minimum coverage threshold
    let min_cov = min_coverage_override.unwrap_or(config.ci.min_coverage);
    if min_cov > 0 && cov_report.stats.total_source_files > 0 {
        let pct = (cov_report.stats.documented_files * 100) / cov_report.stats.total_source_files;
        if pct < min_cov {
            eprintln!("coverage {pct}% is below minimum {min_cov}%");
            failed = true;
        }
    }

    // Check for unmapped files
    let check_unmapped = fail_on_unmapped || config.ci.fail_on_unmapped;
    if check_unmapped {
        let excludes = config.exclude_patterns(&manifest.exclude_patterns);
        let unmapped = filter_excluded(&cov_report.undocumented, &excludes);
        if !unmapped.is_empty() {
            eprintln!("{} unmapped source files (not excluded):", unmapped.len());
            for f in unmapped.iter().take(10) {
                eprintln!("  {f}");
            }
            if unmapped.len() > 10 {
                eprintln!("  ... and {} more", unmapped.len() - 10);
            }
            failed = true;
        }
    }

    if failed {
        process::exit(1);
    }
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

fn cmd_suggest(
    cli: &Cli,
    min_confidence: f64,
    apply_threshold: Option<f64>,
    file_filter: Option<&str>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = Manifest::load(&cli.manifest)?;
    let manifest_dir = cli.manifest.parent().unwrap_or(Path::new("."));
    let source_repo = resolve_source_repo(cli, &manifest, manifest_dir)?;

    // Get source files to suggest for
    let source_files = if let Some(file) = file_filter {
        vec![file.to_string()]
    } else {
        let patterns = coverage::default_scan_patterns();
        let all_files = coverage::scan_source_files_pub(&source_repo, &patterns)?;
        // Filter by exclude patterns
        filter_excluded(&all_files, &manifest.exclude_patterns)
    };

    let report = suggest::suggest_mappings(
        &manifest,
        &source_repo,
        manifest_dir,
        &source_files,
        min_confidence,
    );

    if json {
        let output = format_suggest_json(&report);
        println!("{output}");
    } else {
        format_suggest_text(&report);
    }

    // Auto-apply if requested
    if let Some(threshold) = apply_threshold {
        let mut applied = 0;
        for s in &report.suggestions {
            if s.confidence >= threshold {
                if let Some(idx) = manifest.find_page(&s.route) {
                    let already = manifest.pages[idx]
                        .sources
                        .iter()
                        .any(|src| src.path == s.source_path);
                    if !already {
                        manifest.pages[idx].sources.push(Source {
                            path: s.source_path.clone(),
                            sections: vec![],
                        });
                        applied += 1;
                    }
                }
            }
        }
        if applied > 0 {
            manifest.save(&cli.manifest)?;
            println!("\nApplied {applied} mappings (threshold: {threshold:.1})");
        }
    }

    Ok(())
}

fn filter_excluded(files: &[String], patterns: &[String]) -> Vec<String> {
    if patterns.is_empty() {
        return files.to_vec();
    }
    let compiled: Vec<glob::Pattern> = patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();
    files
        .iter()
        .filter(|f| !compiled.iter().any(|pat| pat.matches(f)))
        .cloned()
        .collect()
}

fn format_suggest_json(report: &suggest::SuggestReport) -> String {
    let suggestions: Vec<serde_json::Value> = report
        .suggestions
        .iter()
        .map(|s| {
            serde_json::json!({
                "source": s.source_path,
                "route": s.route,
                "confidence": (s.confidence * 100.0).round() / 100.0,
                "reasons": s.reasons,
            })
        })
        .collect();
    let output = serde_json::json!({
        "suggestions": suggestions,
        "no_match": report.no_match,
    });
    serde_json::to_string_pretty(&output).unwrap_or_default()
}

fn format_suggest_text(report: &suggest::SuggestReport) {
    use colored::Colorize;

    if report.suggestions.is_empty() && report.no_match.is_empty() {
        println!("All source files are already mapped.");
        return;
    }

    if !report.suggestions.is_empty() {
        println!(
            "{}",
            format!("SUGGESTED MAPPINGS ({}):", report.suggestions.len())
                .green()
                .bold()
        );
        for s in &report.suggestions {
            let pct = format!("{:.0}%", s.confidence * 100.0);
            println!(
                "  {} -> {} ({})",
                s.source_path,
                s.route.cyan(),
                pct.yellow()
            );
            for reason in &s.reasons {
                println!("    {reason}");
            }
        }
        println!();
        println!("  Apply with: docfresh map <route> <source>  (or --apply <threshold>)");
    }

    if !report.no_match.is_empty() {
        println!();
        println!(
            "{}",
            format!("NO MATCH ({}):", report.no_match.len()).dimmed()
        );
        for f in &report.no_match {
            println!("  {f}");
        }
    }
}

fn cmd_map(
    cli: &Cli,
    route: &str,
    source: &str,
    sections: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = Manifest::load(&cli.manifest)?;

    let idx = manifest
        .find_page(route)
        .ok_or_else(|| format!("page '{route}' not found in manifest"))?;

    // Check for duplicates
    if manifest.pages[idx].sources.iter().any(|s| s.path == source) {
        println!("{source} is already mapped to {route}");
        return Ok(());
    }

    manifest.pages[idx].sources.push(Source {
        path: source.to_string(),
        sections: sections.to_vec(),
    });

    manifest.save(&cli.manifest)?;
    println!("Mapped {source} -> {route}");
    Ok(())
}

fn cmd_ci(
    cli: &Cli,
    max_stale_override: Option<usize>,
    min_coverage_override: Option<usize>,
    fail_on_unmapped_override: Option<bool>,
    format_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = Manifest::load(&cli.manifest)?;
    let manifest_dir = cli.manifest.parent().unwrap_or(Path::new("."));
    let source_repo = resolve_source_repo(cli, &manifest, manifest_dir)?;
    let config = Config::load(manifest_dir);

    let max_stale = max_stale_override.unwrap_or(config.ci.max_stale);
    let min_coverage = min_coverage_override.unwrap_or(config.ci.min_coverage);
    let fail_on_unmapped = fail_on_unmapped_override.unwrap_or(config.ci.fail_on_unmapped);
    let format_str = format_override.unwrap_or(&config.ci.format);
    let format = match format_str {
        "json" => Format::Json,
        "markdown" | "md" => Format::Markdown,
        _ => Format::Text,
    };

    let mut failures: Vec<String> = Vec::new();

    // 1. Audit
    let summary = audit::audit_all(&manifest, &source_repo, None);
    println!("{}", report::format_audit(&summary, format));

    let stale_count = summary
        .results
        .iter()
        .filter(|r| r.new_status == Status::Stale)
        .count();
    if stale_count > max_stale {
        failures.push(format!(
            "audit: {stale_count} stale pages (max: {max_stale})"
        ));
    }

    println!();

    // 2. Coverage
    let defaults = coverage::default_scan_patterns();
    let patterns = config.scan_patterns(&defaults);
    let cov_report = coverage::compute_coverage(&manifest, &source_repo, &patterns)?;
    println!("{}", report::format_coverage(&cov_report, format));

    if min_coverage > 0 && cov_report.stats.total_source_files > 0 {
        let pct = (cov_report.stats.documented_files * 100) / cov_report.stats.total_source_files;
        if pct < min_coverage {
            failures.push(format!("coverage: {pct}% (min: {min_coverage}%)"));
        }
    }

    if fail_on_unmapped {
        let excludes = config.exclude_patterns(&manifest.exclude_patterns);
        let unmapped = filter_excluded(&cov_report.undocumented, &excludes);
        if !unmapped.is_empty() {
            failures.push(format!("unmapped: {} source files", unmapped.len()));
        }
    }

    // 3. Summary
    if failures.is_empty() {
        println!("\nCI check passed.");
    } else {
        println!();
        for f in &failures {
            eprintln!("FAIL: {f}");
        }
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
