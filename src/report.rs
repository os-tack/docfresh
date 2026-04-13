use crate::audit::AuditSummary;
use crate::concept_graph::ConceptGraph;
use crate::coverage::CoverageReport;
use crate::manifest::{Manifest, Status};
use colored::Colorize;

#[derive(Clone, Copy)]
pub enum Format {
    Text,
    Json,
    Markdown,
}

pub fn format_status_table(manifest: &Manifest, format: Format) -> String {
    match format {
        Format::Json => format_status_json(manifest),
        Format::Text => format_status_text(manifest),
        Format::Markdown => format_status_markdown(manifest),
    }
}

fn format_status_text(manifest: &Manifest) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "{:<30} {:<12} {:<12} {:<6}",
        "ROUTE", "STATUS", "VERIFIED", "SRCS"
    ));
    lines.push("-".repeat(62));

    for page in &manifest.pages {
        let status_str = colorize_status(&page.status);
        let verified = page
            .verified_at
            .as_ref()
            .map_or_else(|| "-".to_string(), |v| v.sha.clone());
        lines.push(format!(
            "{:<30} {:<12} {:<12} {:<6}",
            truncate(&page.route, 30),
            status_str,
            verified,
            page.sources.len()
        ));
    }

    let counts = count_statuses(manifest);
    lines.push(String::new());
    lines.push(format!(
        "{} current, {} stale, {} unverified, {} missing, {} outdated ({} total)",
        counts.current,
        counts.stale,
        counts.unverified,
        counts.missing,
        counts.outdated,
        manifest.pages.len()
    ));

    lines.join("\n")
}

fn format_status_json(manifest: &Manifest) -> String {
    let entries: Vec<serde_json::Value> = manifest
        .pages
        .iter()
        .map(|p| {
            serde_json::json!({
                "route": p.route,
                "title": p.title,
                "status": p.status.to_string(),
                "verified_at": p.verified_at.as_ref().map(|v| &v.sha),
                "sources": p.sources.len(),
                "tags": p.tags,
            })
        })
        .collect();
    serde_json::to_string_pretty(&entries).unwrap_or_default()
}

fn format_status_markdown(manifest: &Manifest) -> String {
    let mut lines = Vec::new();
    lines.push("| Route | Status | Verified | Sources |".to_string());
    lines.push("|-------|--------|----------|---------|".to_string());

    for page in &manifest.pages {
        let verified = page
            .verified_at
            .as_ref()
            .map_or_else(|| "-".to_string(), |v| v.sha.clone());
        let status_emoji = match page.status {
            Status::Current => "ok",
            Status::Stale => "STALE",
            Status::Outdated => "OUTDATED",
            Status::Unverified => "unverified",
            Status::Missing => "MISSING",
        };
        lines.push(format!(
            "| {} | {} | {} | {} |",
            page.route,
            status_emoji,
            verified,
            page.sources.len()
        ));
    }

    lines.join("\n")
}

pub fn format_audit(summary: &AuditSummary, format: Format) -> String {
    match format {
        Format::Json => format_audit_json(summary),
        Format::Text => format_audit_text(summary),
        Format::Markdown => format_audit_markdown(summary),
    }
}

fn format_audit_text(summary: &AuditSummary) -> String {
    let mut lines = Vec::new();

    let stale: Vec<_> = summary
        .results
        .iter()
        .filter(|r| r.new_status == Status::Stale)
        .collect();
    let errors: Vec<_> = summary
        .results
        .iter()
        .filter(|r| r.error.is_some())
        .collect();
    let unverified: Vec<_> = summary
        .results
        .iter()
        .filter(|r| r.new_status == Status::Unverified || r.new_status == Status::Missing)
        .collect();
    let current: Vec<_> = summary
        .results
        .iter()
        .filter(|r| r.new_status == Status::Current)
        .collect();

    if !stale.is_empty() {
        lines.push(format!(
            "{}",
            "STALE (source changed since verification):".red().bold()
        ));
        for r in &stale {
            lines.push(format!("  {} — {}", r.route.yellow(), r.title));
            for src in &r.changed_sources {
                lines.push(format!("    changed: {src}"));
            }
            for entry in r.log_entries.iter().take(5) {
                lines.push(format!("    {} {}", entry.sha.dimmed(), entry.message));
            }
            if r.log_entries.len() > 5 {
                lines.push(format!(
                    "    ... and {} more commits",
                    r.log_entries.len() - 5
                ));
            }
        }
        lines.push(String::new());
    }

    if !errors.is_empty() {
        lines.push(format!("{}", "ERRORS:".red().bold()));
        for r in &errors {
            lines.push(format!(
                "  {} — {}",
                r.route,
                r.error.as_deref().unwrap_or("unknown error")
            ));
        }
        lines.push(String::new());
    }

    if !summary.related_warnings.is_empty() {
        lines.push(format!(
            "{}",
            "REVIEW RECOMMENDED (related to stale pages):"
                .yellow()
                .bold()
        ));
        for w in &summary.related_warnings {
            lines.push(format!(
                "  {} — references stale {}",
                w.route, w.stale_dependency
            ));
        }
        lines.push(String::new());
    }

    if !unverified.is_empty() {
        lines.push(format!("{}", "UNVERIFIED / MISSING:".dimmed()));
        for r in &unverified {
            lines.push(format!("  {} ({})", r.route, r.new_status));
        }
        lines.push(String::new());
    }

    lines.push(format!(
        "{} current, {} stale, {} unverified/missing, {} errors",
        current.len().to_string().green(),
        stale.len().to_string().red(),
        unverified.len().to_string().yellow(),
        errors.len()
    ));

    lines.join("\n")
}

fn format_audit_json(summary: &AuditSummary) -> String {
    let results: Vec<serde_json::Value> = summary
        .results
        .iter()
        .map(|r| {
            serde_json::json!({
                "route": r.route,
                "title": r.title,
                "old_status": r.old_status.to_string(),
                "new_status": r.new_status.to_string(),
                "changed_sources": r.changed_sources,
                "commits": r.log_entries.iter().map(|e| {
                    serde_json::json!({"sha": e.sha, "message": e.message})
                }).collect::<Vec<_>>(),
                "error": r.error,
            })
        })
        .collect();

    let warnings: Vec<serde_json::Value> = summary
        .related_warnings
        .iter()
        .map(|w| {
            serde_json::json!({
                "route": w.route,
                "stale_dependency": w.stale_dependency,
            })
        })
        .collect();

    let output = serde_json::json!({
        "results": results,
        "related_warnings": warnings,
    });

    serde_json::to_string_pretty(&output).unwrap_or_default()
}

fn format_audit_markdown(summary: &AuditSummary) -> String {
    let mut lines = Vec::new();
    lines.push("## Documentation Freshness Audit".to_string());
    lines.push(String::new());

    let stale: Vec<_> = summary
        .results
        .iter()
        .filter(|r| r.new_status == Status::Stale)
        .collect();

    if stale.is_empty() {
        lines.push("All documented pages are current.".to_string());
    } else {
        lines.push(format!("### Stale Pages ({})", stale.len()));
        lines.push(String::new());
        for r in &stale {
            lines.push(format!("- **{}** — {}", r.route, r.title));
            for src in &r.changed_sources {
                lines.push(format!("  - `{src}`"));
            }
        }
    }

    if !summary.related_warnings.is_empty() {
        lines.push(String::new());
        lines.push("### Review Recommended".to_string());
        lines.push(String::new());
        for w in &summary.related_warnings {
            lines.push(format!(
                "- **{}** — references stale `{}`",
                w.route, w.stale_dependency
            ));
        }
    }

    lines.join("\n")
}

pub fn format_coverage(report: &CoverageReport, format: Format) -> String {
    match format {
        Format::Json => format_coverage_json(report),
        Format::Text => format_coverage_text(report),
        Format::Markdown => format_coverage_markdown(report),
    }
}

fn format_coverage_text(report: &CoverageReport) -> String {
    let mut lines = Vec::new();
    let s = &report.stats;

    lines.push(format!(
        "Coverage: {}/{} source files documented ({:.0}%)",
        s.documented_files,
        s.total_source_files,
        if s.total_source_files > 0 {
            (s.documented_files as f64 / s.total_source_files as f64) * 100.0
        } else {
            100.0
        }
    ));

    if let Some(cs) = &report.concept_stats {
        let pct = if cs.total > 0 {
            (cs.covered as f64 / cs.total as f64) * 100.0
        } else {
            100.0
        };
        lines.push(format!(
            "Concepts: {}/{} covered ({:.0}%)",
            cs.covered, cs.total, pct
        ));
    }

    lines.push(format!(
        "Pages: {}/{} have source mappings",
        s.pages_with_sources, s.total_pages
    ));
    lines.push(String::new());

    if !report.undocumented.is_empty() {
        lines.push(format!(
            "{}",
            format!("UNDOCUMENTED ({}):", report.undocumented.len())
                .yellow()
                .bold()
        ));
        for f in &report.undocumented {
            lines.push(format!("  {f}"));
        }
        lines.push(String::new());
    }

    if !report.orphan_pages.is_empty() {
        lines.push(format!(
            "{}",
            format!("ORPHAN PAGES ({}):", report.orphan_pages.len())
                .red()
                .bold()
        ));
        for o in &report.orphan_pages {
            lines.push(format!("  {} — {}", o.route, o.reason));
        }
        lines.push(String::new());
    }

    if !report.shared_sources.is_empty() {
        lines.push(format!(
            "{}",
            format!("SHARED SOURCES ({}):", report.shared_sources.len()).dimmed()
        ));
        for s in &report.shared_sources {
            lines.push(format!("  {} ({})", s.path, s.pages.join(", ")));
        }
        lines.push(String::new());
    }

    if let Some(cs) = &report.concept_stats {
        if !cs.orphans.is_empty() {
            lines.push(format!(
                "{}",
                format!("ORPHAN CONCEPTS ({}):", cs.orphans.len())
                    .yellow()
                    .bold()
            ));
            // Group orphans by source file
            let mut by_file: std::collections::BTreeMap<&str, Vec<&crate::concepts::OrphanConcept>> =
                std::collections::BTreeMap::new();
            for oc in &cs.orphans {
                by_file.entry(&oc.source_file).or_default().push(oc);
            }
            for (file, orphans) in &by_file {
                lines.push(format!("  {file}:"));
                for oc in orphans {
                    lines.push(format!("    {} ({})", oc.name, oc.kind));
                }
            }
            lines.push(String::new());
        }

        let orphan_count = cs.orphans.len();
        let source_files: std::collections::HashSet<&str> = cs
            .orphans
            .iter()
            .map(|c| c.source_file.as_str())
            .collect();
        // Count unique source files across all concepts, not just orphans
        lines.push(format!(
            "{} concepts in {} source files; {} covered by docs, {} orphaned",
            cs.total,
            source_files.len(),
            cs.covered,
            orphan_count,
        ));
    }

    lines.join("\n")
}

fn format_coverage_json(report: &CoverageReport) -> String {
    let concept_stats = report.concept_stats.as_ref().map(|cs| {
        serde_json::json!({
            "total_concepts": cs.total,
            "covered_concepts": cs.covered,
            "orphan_concepts": cs.orphans.iter().map(|oc| {
                serde_json::json!({
                    "name": oc.name,
                    "kind": oc.kind,
                    "source_file": oc.source_file,
                })
            }).collect::<Vec<_>>(),
        })
    });

    let output = serde_json::json!({
        "stats": {
            "total_source_files": report.stats.total_source_files,
            "documented_files": report.stats.documented_files,
            "undocumented_files": report.stats.undocumented_files,
            "total_pages": report.stats.total_pages,
            "pages_with_sources": report.stats.pages_with_sources,
            "orphan_pages": report.stats.orphan_pages,
        },
        "concept_stats": concept_stats,
        "undocumented": report.undocumented,
        "orphan_pages": report.orphan_pages.iter().map(|o| {
            serde_json::json!({"route": o.route, "reason": o.reason})
        }).collect::<Vec<_>>(),
        "shared_sources": report.shared_sources.iter().map(|s| {
            serde_json::json!({"path": s.path, "pages": s.pages})
        }).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&output).unwrap_or_default()
}

fn format_coverage_markdown(report: &CoverageReport) -> String {
    let mut lines = Vec::new();
    let s = &report.stats;

    lines.push("## Coverage Report".to_string());
    lines.push(String::new());
    lines.push(format!(
        "- **{}/{}** source files documented ({:.0}%)",
        s.documented_files,
        s.total_source_files,
        if s.total_source_files > 0 {
            (s.documented_files as f64 / s.total_source_files as f64) * 100.0
        } else {
            100.0
        }
    ));
    if let Some(cs) = &report.concept_stats {
        let pct = if cs.total > 0 {
            (cs.covered as f64 / cs.total as f64) * 100.0
        } else {
            100.0
        };
        lines.push(format!(
            "- **{}/{}** concepts covered ({:.0}%)",
            cs.covered, cs.total, pct
        ));
    }

    lines.push(format!(
        "- **{}/{}** pages have source mappings",
        s.pages_with_sources, s.total_pages
    ));

    if !report.undocumented.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "### Undocumented Files ({})",
            report.undocumented.len()
        ));
        lines.push(String::new());
        for f in &report.undocumented {
            lines.push(format!("- `{f}`"));
        }
    }

    if let Some(cs) = &report.concept_stats {
        if !cs.orphans.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "### Orphan Concepts ({})",
                cs.orphans.len()
            ));
            lines.push(String::new());
            for oc in &cs.orphans {
                lines.push(format!(
                    "- `{}` ({}) in `{}`",
                    oc.name, oc.kind, oc.source_file
                ));
            }
        }
    }

    lines.join("\n")
}

pub fn format_diff(
    route: &str,
    log_entries: &[crate::git::LogEntry],
    changed_files: &[String],
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Changes for {} since last verification:",
        route.bold()
    ));
    lines.push(String::new());

    if changed_files.is_empty() && log_entries.is_empty() {
        lines.push("  No changes detected.".to_string());
        return lines.join("\n");
    }

    if !changed_files.is_empty() {
        lines.push("Files changed:".to_string());
        for f in changed_files {
            lines.push(format!("  {f}"));
        }
        lines.push(String::new());
    }

    if !log_entries.is_empty() {
        lines.push("Commits:".to_string());
        for entry in log_entries {
            lines.push(format!("  {} {}", entry.sha.dimmed(), entry.message));
        }
    }

    lines.join("\n")
}

struct StatusCounts {
    current: usize,
    stale: usize,
    unverified: usize,
    missing: usize,
    outdated: usize,
}

fn count_statuses(manifest: &Manifest) -> StatusCounts {
    let mut counts = StatusCounts {
        current: 0,
        stale: 0,
        unverified: 0,
        missing: 0,
        outdated: 0,
    };
    for page in &manifest.pages {
        match page.status {
            Status::Current => counts.current += 1,
            Status::Stale => counts.stale += 1,
            Status::Unverified => counts.unverified += 1,
            Status::Missing => counts.missing += 1,
            Status::Outdated => counts.outdated += 1,
        }
    }
    counts
}

fn colorize_status(status: &Status) -> String {
    match status {
        Status::Current => "current".green().to_string(),
        Status::Stale => "stale".red().to_string(),
        Status::Outdated => "outdated".red().bold().to_string(),
        Status::Unverified => "unverified".yellow().to_string(),
        Status::Missing => "missing".yellow().dimmed().to_string(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

pub fn format_concept_graph_text(graph: &ConceptGraph) -> String {
    let mut lines = Vec::new();
    let s = &graph.stats;

    lines.push(format!(
        "Concept Graph: {} concepts across {} pages",
        s.total_concepts, s.total_pages
    ));
    lines.push(format!(
        "  Orphans: {}  Thin coverage: {}  Stale siblings: {}",
        s.orphan_count, s.thin_coverage_count, s.stale_sibling_count
    ));
    lines.push(String::new());

    if !graph.orphans.is_empty() {
        lines.push(format!(
            "{}",
            format!("ORPHAN CONCEPTS ({}):", graph.orphans.len())
                .red()
                .bold()
        ));
        for o in &graph.orphans {
            lines.push(format!("  {} ({}) in {}", o.name, o.kind, o.source_file));
        }
        lines.push(String::new());
    }

    if !graph.thin_coverage.is_empty() {
        lines.push(format!(
            "{}",
            format!("THIN COVERAGE ({}):", graph.thin_coverage.len())
                .yellow()
                .bold()
        ));
        for t in &graph.thin_coverage {
            lines.push(format!(
                "  {} <- {} missing: {}",
                t.route,
                t.source_file,
                t.missing_concepts.join(", ")
            ));
        }
        lines.push(String::new());
    }

    if !graph.stale_siblings.is_empty() {
        lines.push(format!(
            "{}",
            format!("STALE SIBLINGS ({}):", graph.stale_siblings.len())
                .yellow()
                .bold()
        ));
        for s in &graph.stale_siblings {
            lines.push(format!("  concept: {}", s.concept));
            for (route, sha) in &s.pages {
                let sha_str = sha.as_deref().unwrap_or("unverified");
                lines.push(format!("    {route} @ {sha_str}"));
            }
        }
        lines.push(String::new());
    }

    if graph.orphans.is_empty()
        && graph.thin_coverage.is_empty()
        && graph.stale_siblings.is_empty()
    {
        lines.push("All concepts are documented with full coverage.".to_string());
    }

    lines.join("\n")
}

pub fn format_concept_graph_json(graph: &ConceptGraph) -> String {
    serde_json::to_string_pretty(graph).unwrap_or_default()
}

pub fn format_concept_graph_dot(graph: &ConceptGraph) -> String {
    let mut lines = vec![
        "digraph concepts {".to_string(),
        "  rankdir=LR;".to_string(),
        "  node [fontname=\"Helvetica\"];".to_string(),
        String::new(),
    ];

    // Collect orphan names for highlighting
    let orphan_names: std::collections::HashSet<&str> =
        graph.orphans.iter().map(|o| o.name.as_str()).collect();

    // Collect all unique page routes used by nodes
    let mut page_set: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for node in &graph.nodes {
        for page in &node.pages {
            page_set.insert(page.as_str());
        }
    }

    // Page nodes (boxes)
    lines.push("  // Pages".to_string());
    for page in &page_set {
        let id = dot_id(page);
        lines.push(format!("  {id} [label=\"{page}\", shape=box];"));
    }
    lines.push(String::new());

    // Concept nodes (circles), orphans in red
    lines.push("  // Concepts".to_string());
    for node in &graph.nodes {
        let id = dot_id(&node.name);
        if orphan_names.contains(node.name.as_str()) {
            lines.push(format!(
                "  {id} [label=\"{}\", shape=circle, color=red, fontcolor=red];",
                node.name
            ));
        } else {
            lines.push(format!(
                "  {id} [label=\"{}\", shape=circle];",
                node.name
            ));
        }
    }
    lines.push(String::new());

    // Edges: concept -> page
    lines.push("  // Edges".to_string());
    for node in &graph.nodes {
        let concept_id = dot_id(&node.name);
        for page in &node.pages {
            let page_id = dot_id(page);
            lines.push(format!("  {concept_id} -> {page_id};"));
        }
    }

    lines.push("}".to_string());
    lines.join("\n")
}

/// Convert a string to a valid DOT node identifier.
fn dot_id(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    format!("n_{cleaned}")
}
