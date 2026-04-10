use crate::manifest::Manifest;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

pub struct CoverageReport {
    pub undocumented: Vec<String>,
    pub orphan_pages: Vec<OrphanPage>,
    pub shared_sources: Vec<SharedSource>,
    pub stats: CoverageStats,
}

pub struct OrphanPage {
    pub route: String,
    pub reason: String,
}

pub struct SharedSource {
    pub path: String,
    pub pages: Vec<String>,
}

pub struct CoverageStats {
    pub total_source_files: usize,
    pub documented_files: usize,
    pub undocumented_files: usize,
    pub total_pages: usize,
    pub pages_with_sources: usize,
    pub orphan_pages: usize,
}

pub fn compute_coverage(
    manifest: &Manifest,
    source_repo: &Path,
    scan_patterns: &[&str],
) -> Result<CoverageReport, Box<dyn std::error::Error>> {
    // Collect all source files matching the patterns
    let source_files = scan_source_files(source_repo, scan_patterns)?;
    let source_set: HashSet<&str> = source_files
        .iter()
        .map(std::string::String::as_str)
        .collect();

    // Collect all sources referenced in the manifest
    let mut documented: HashSet<String> = HashSet::new();
    let mut source_to_pages: HashMap<String, Vec<String>> = HashMap::new();

    for page in &manifest.pages {
        for source in &page.sources {
            documented.insert(source.path.clone());
            source_to_pages
                .entry(source.path.clone())
                .or_default()
                .push(page.route.clone());
        }
    }

    // Undocumented: source files not referenced by any page
    let mut undocumented: Vec<String> = source_set
        .iter()
        .filter(|f| !documented.contains(**f))
        .map(|f| (*f).to_string())
        .collect();
    undocumented.sort();

    // Orphan pages: pages with no sources or with sources that don't exist
    let mut orphan_pages = Vec::new();
    for page in &manifest.pages {
        if page.sources.is_empty() {
            orphan_pages.push(OrphanPage {
                route: page.route.clone(),
                reason: "no sources listed".to_string(),
            });
            continue;
        }
        let missing: Vec<&str> = page
            .sources
            .iter()
            .filter(|s| {
                let full_path = source_repo.join(&s.path);
                !full_path.exists()
            })
            .map(|s| s.path.as_str())
            .collect();
        if !missing.is_empty() {
            orphan_pages.push(OrphanPage {
                route: page.route.clone(),
                reason: format!("sources not found: {}", missing.join(", ")),
            });
        }
    }

    // Shared sources: files referenced by multiple pages
    let mut shared_sources: Vec<SharedSource> = source_to_pages
        .into_iter()
        .filter(|(_, pages)| pages.len() > 1)
        .map(|(path, pages)| SharedSource { path, pages })
        .collect();
    shared_sources.sort_by(|a, b| b.pages.len().cmp(&a.pages.len()));

    let documented_count = documented.len();
    let pages_with_sources = manifest
        .pages
        .iter()
        .filter(|p| !p.sources.is_empty())
        .count();

    let stats = CoverageStats {
        total_source_files: source_files.len(),
        documented_files: documented_count,
        undocumented_files: undocumented.len(),
        total_pages: manifest.pages.len(),
        pages_with_sources,
        orphan_pages: orphan_pages.len(),
    };

    Ok(CoverageReport {
        undocumented,
        orphan_pages,
        shared_sources,
        stats,
    })
}

fn scan_source_files(
    repo_path: &Path,
    patterns: &[&str],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut all_files = Vec::new();

    for pattern in patterns {
        // Use git ls-files to respect .gitignore and get tracked files
        let output = Command::new("git")
            .args(["ls-files", pattern])
            .current_dir(repo_path)
            .output()?;
        if output.status.success() {
            let stdout = String::from_utf8(output.stdout)?;
            for line in stdout.lines() {
                if !line.is_empty() {
                    all_files.push(line.to_string());
                }
            }
        }
    }

    all_files.sort();
    all_files.dedup();
    Ok(all_files)
}

/// Default scan patterns for Rust projects
pub fn default_scan_patterns() -> Vec<&'static str> {
    vec!["src/**/*.rs", "docs/spec/**/*.md"]
}
