use crate::git;
use crate::manifest::{Manifest, Status};
use std::path::Path;

pub struct AuditResult {
    pub route: String,
    pub title: String,
    pub old_status: Status,
    pub new_status: Status,
    pub changed_sources: Vec<String>,
    pub log_entries: Vec<git::LogEntry>,
    pub error: Option<String>,
}

pub struct AuditSummary {
    pub results: Vec<AuditResult>,
    pub related_warnings: Vec<RelatedWarning>,
}

pub struct RelatedWarning {
    pub route: String,
    pub stale_dependency: String,
}

pub fn audit_all(
    manifest: &Manifest,
    source_repo: &Path,
    tag_filter: Option<&str>,
) -> AuditSummary {
    let mut results = Vec::new();

    for page in &manifest.pages {
        if let Some(tag) = tag_filter {
            if !page.tags.contains(&tag.to_string()) {
                continue;
            }
        }

        let result = audit_page(page, source_repo);
        results.push(result);
    }

    // Compute transitive staleness warnings
    let stale_routes: Vec<String> = results
        .iter()
        .filter(|r| r.new_status == Status::Stale)
        .map(|r| r.route.clone())
        .collect();

    let mut related_warnings = Vec::new();
    for page in &manifest.pages {
        for related in &page.related {
            if stale_routes.contains(related) {
                // Only warn if this page isn't already stale itself
                let already_stale = results
                    .iter()
                    .any(|r| r.route == page.route && r.new_status == Status::Stale);
                if !already_stale {
                    related_warnings.push(RelatedWarning {
                        route: page.route.clone(),
                        stale_dependency: related.clone(),
                    });
                }
            }
        }
    }

    AuditSummary {
        results,
        related_warnings,
    }
}

fn audit_page(page: &crate::manifest::Page, source_repo: &Path) -> AuditResult {
    let old_status = page.status.clone();

    // Pages with no verification can't be audited
    let Some(verified) = &page.verified_at else {
        return AuditResult {
            route: page.route.clone(),
            title: page.title.clone(),
            old_status: old_status.clone(),
            new_status: old_status,
            changed_sources: vec![],
            log_entries: vec![],
            error: None,
        };
    };

    // Missing pages stay missing
    if page.file.is_none() {
        return AuditResult {
            route: page.route.clone(),
            title: page.title.clone(),
            old_status: old_status.clone(),
            new_status: Status::Missing,
            changed_sources: vec![],
            log_entries: vec![],
            error: None,
        };
    }

    if page.sources.is_empty() {
        return AuditResult {
            route: page.route.clone(),
            title: page.title.clone(),
            old_status: old_status.clone(),
            new_status: old_status,
            changed_sources: vec![],
            log_entries: vec![],
            error: None,
        };
    }

    let source_paths: Vec<&str> = page.sources.iter().map(|s| s.path.as_str()).collect();

    // Check which files changed
    let changed = match git::changed_files_between(source_repo, &verified.sha, &source_paths) {
        Ok(files) => files,
        Err(e) => {
            return AuditResult {
                route: page.route.clone(),
                title: page.title.clone(),
                old_status,
                new_status: Status::Unverified,
                changed_sources: vec![],
                log_entries: vec![],
                error: Some(e.to_string()),
            };
        }
    };

    if changed.is_empty() {
        return AuditResult {
            route: page.route.clone(),
            title: page.title.clone(),
            old_status,
            new_status: Status::Current,
            changed_sources: vec![],
            log_entries: vec![],
            error: None,
        };
    }

    // Get the commit log for changed files
    let log_entries = git::file_log(source_repo, &verified.sha, &source_paths).unwrap_or_default();

    AuditResult {
        route: page.route.clone(),
        title: page.title.clone(),
        old_status,
        new_status: Status::Stale,
        changed_sources: changed,
        log_entries,
        error: None,
    }
}
