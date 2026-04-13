use crate::concepts::{self, Concept};
use crate::coverage;
use crate::manifest::Manifest;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct ConceptGraph {
    pub nodes: Vec<ConceptNode>,
    pub orphans: Vec<OrphanDetail>,
    pub thin_coverage: Vec<ThinCoverage>,
    pub stale_siblings: Vec<StaleSibling>,
    pub stats: GraphStats,
}

#[derive(Debug, Serialize)]
pub struct ConceptNode {
    pub name: String,
    pub source_files: Vec<String>,
    pub pages: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct OrphanDetail {
    pub name: String,
    pub kind: String,
    pub source_file: String,
}

#[derive(Debug, Serialize)]
pub struct ThinCoverage {
    pub route: String,
    pub source_file: String,
    pub missing_concepts: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct StaleSibling {
    pub concept: String,
    pub pages: Vec<(String, Option<String>)>,
}

#[derive(Debug, Serialize)]
pub struct GraphStats {
    pub total_concepts: usize,
    pub total_pages: usize,
    pub orphan_count: usize,
    pub thin_coverage_count: usize,
    pub stale_sibling_count: usize,
}

/// Build the concept graph from a manifest and source repository.
///
/// 1. Extract all concepts from source files matching `scan_patterns`.
/// 2. For each page with a `file` field, read page content and scan for concept mentions.
/// 3. Build `ConceptNode` entries mapping concept -> source files + pages.
/// 4. Compute orphans, thin coverage, and stale siblings.
pub fn build_graph(
    manifest: &Manifest,
    source_repo: &Path,
    scan_patterns: &[&str],
    page_content_dir: Option<&Path>,
) -> Result<ConceptGraph, Box<dyn std::error::Error>> {
    let source_files = coverage::scan_source_files_pub(source_repo, scan_patterns)?;
    let all_concepts = concepts::extract_all_concepts(&source_files, source_repo);
    Ok(build_graph_from_concepts(
        manifest,
        &all_concepts,
        page_content_dir,
    ))
}

/// Core graph builder that works from a pre-extracted concept list.
fn build_graph_from_concepts(
    manifest: &Manifest,
    all_concepts: &[Concept],
    page_content_dir: Option<&Path>,
) -> ConceptGraph {
    // Build a map: concept_name -> Vec<&Concept> (may appear in multiple files)
    let mut concept_sources: HashMap<String, Vec<&Concept>> = HashMap::new();
    for concept in all_concepts {
        concept_sources
            .entry(concept.name.clone())
            .or_default()
            .push(concept);
    }

    // Scan each page for concept mentions
    let mut page_concepts: HashMap<String, HashSet<String>> = HashMap::new();

    for page in &manifest.pages {
        let Some(file_path) = &page.file else {
            continue;
        };

        let content = if let Some(dir) = page_content_dir {
            let full = dir.join(file_path);
            std::fs::read_to_string(&full).ok()
        } else {
            std::fs::read_to_string(file_path).ok()
        };

        let Some(content) = content else {
            continue;
        };

        let found = concepts::scan_page_for_concepts(&content, all_concepts);
        page_concepts.insert(page.route.clone(), found.into_iter().collect());
    }

    // Build ConceptNode entries
    let concept_names: Vec<String> = {
        let mut names: Vec<String> = concept_sources.keys().cloned().collect();
        names.sort();
        names
    };

    let mut nodes: Vec<ConceptNode> = Vec::new();
    for name in &concept_names {
        let sources = concept_sources.get(name).map_or_else(Vec::new, |cs| {
            let mut files: Vec<String> = cs.iter().map(|c| c.source_file.clone()).collect();
            files.sort();
            files.dedup();
            files
        });

        let mut pages: Vec<String> = page_concepts
            .iter()
            .filter(|(_, concepts)| concepts.contains(name))
            .map(|(route, _)| route.clone())
            .collect();
        pages.sort();

        nodes.push(ConceptNode {
            name: name.clone(),
            source_files: sources,
            pages,
        });
    }

    // Compute orphans -- concepts that appear on no pages
    let orphans: Vec<OrphanDetail> = all_concepts
        .iter()
        .filter(|c| {
            !page_concepts
                .values()
                .any(|concepts| concepts.contains(&c.name))
        })
        .map(|c| OrphanDetail {
            name: c.name.clone(),
            kind: c.kind.to_string(),
            source_file: c.source_file.clone(),
        })
        .collect();
    let orphans = dedup_orphans(orphans);

    let thin_coverage = find_thin_coverage(manifest, all_concepts, &page_concepts);
    let stale_siblings = find_stale_siblings(manifest, &nodes);

    let stats = GraphStats {
        total_concepts: concept_names.len(),
        total_pages: manifest.pages.len(),
        orphan_count: orphans.len(),
        thin_coverage_count: thin_coverage.len(),
        stale_sibling_count: stale_siblings.len(),
    };

    ConceptGraph {
        nodes,
        orphans,
        thin_coverage,
        stale_siblings,
        stats,
    }
}

fn find_thin_coverage(
    manifest: &Manifest,
    all_concepts: &[Concept],
    page_concepts: &HashMap<String, HashSet<String>>,
) -> Vec<ThinCoverage> {
    let mut file_concepts: HashMap<String, Vec<String>> = HashMap::new();
    for concept in all_concepts {
        file_concepts
            .entry(concept.source_file.clone())
            .or_default()
            .push(concept.name.clone());
    }
    for names in file_concepts.values_mut() {
        names.sort();
        names.dedup();
    }

    let mut thin_coverage = Vec::new();
    for page in &manifest.pages {
        let found = page_concepts.get(&page.route).cloned().unwrap_or_default();
        for source in &page.sources {
            if let Some(concepts_in_file) = file_concepts.get(&source.path) {
                let missing: Vec<String> = concepts_in_file
                    .iter()
                    .filter(|c| !found.contains(*c))
                    .cloned()
                    .collect();
                if !missing.is_empty() {
                    thin_coverage.push(ThinCoverage {
                        route: page.route.clone(),
                        source_file: source.path.clone(),
                        missing_concepts: missing,
                    });
                }
            }
        }
    }
    thin_coverage
}

fn find_stale_siblings(manifest: &Manifest, nodes: &[ConceptNode]) -> Vec<StaleSibling> {
    let page_sha: HashMap<&str, Option<&str>> = manifest
        .pages
        .iter()
        .map(|p| {
            (
                p.route.as_str(),
                p.verified_at.as_ref().map(|v| v.sha.as_str()),
            )
        })
        .collect();

    let mut stale_siblings = Vec::new();
    for node in nodes {
        if node.pages.len() < 2 {
            continue;
        }
        let shas: Vec<(String, Option<String>)> = node
            .pages
            .iter()
            .map(|route| {
                let sha = page_sha
                    .get(route.as_str())
                    .copied()
                    .flatten()
                    .map(String::from);
                (route.clone(), sha)
            })
            .collect();
        let unique_shas: HashSet<Option<&str>> =
            shas.iter().map(|(_, s)| s.as_deref()).collect();
        if unique_shas.len() > 1 {
            stale_siblings.push(StaleSibling {
                concept: node.name.clone(),
                pages: shas,
            });
        }
    }
    stale_siblings
}

fn dedup_orphans(mut orphans: Vec<OrphanDetail>) -> Vec<OrphanDetail> {
    orphans.sort_by(|a, b| (&a.name, &a.source_file).cmp(&(&b.name, &b.source_file)));
    orphans.dedup_by(|a, b| a.name == b.name && a.source_file == b.source_file);
    orphans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::concepts::ConceptKind;
    use crate::manifest::{Manifest, Page, Source, SourceRepo, Status, VerifiedAt};

    fn make_manifest(pages: Vec<Page>) -> Manifest {
        Manifest {
            version: 1,
            source_repo: SourceRepo {
                path: ".".to_string(),
                remote: None,
                default_branch: "main".to_string(),
            },
            exclude_patterns: vec![],
            pages,
        }
    }

    #[test]
    fn orphans_detected_for_concepts_not_on_any_page() {
        let dir = tempfile::tempdir().unwrap();

        let pages_dir = dir.path().join("pages");
        std::fs::create_dir_all(&pages_dir).unwrap();
        std::fs::write(
            pages_dir.join("auth.md"),
            "# Auth\n\nThis page talks about authentication.\n",
        )
        .unwrap();

        let all_concepts = vec![Concept {
            name: "TokenValidator".to_string(),
            kind: ConceptKind::Struct,
            source_file: "src/auth.rs".to_string(),
            line: Some(1), description: None,
        }];

        let manifest = make_manifest(vec![Page {
            route: "/docs/auth".to_string(),
            file: Some("pages/auth.md".to_string()),
            title: "Auth".to_string(),
            tags: vec![],
            sources: vec![Source {
                path: "src/auth.rs".to_string(),
                sections: vec![],
            }],
            related: vec![],
            verified_at: None,
            status: Status::Unverified,
            concepts: vec![],
        }]);

        let graph = build_graph_from_concepts(&manifest, &all_concepts, Some(dir.path()));

        assert!(
            graph.orphans.iter().any(|o| o.name == "TokenValidator"),
            "Expected TokenValidator as orphan, got: {:?}",
            graph.orphans
        );
    }

    #[test]
    fn thin_coverage_detected_for_missing_concept_in_page() {
        let dir = tempfile::tempdir().unwrap();

        let pages_dir = dir.path().join("pages");
        std::fs::create_dir_all(&pages_dir).unwrap();
        std::fs::write(
            pages_dir.join("engine.md"),
            "# Engine\n\nCall start_engine to begin.\n",
        )
        .unwrap();

        let all_concepts = vec![
            Concept {
                name: "start_engine".to_string(),
                kind: ConceptKind::Function,
                source_file: "src/engine.rs".to_string(),
                line: Some(1), description: None,
            },
            Concept {
                name: "stop_engine".to_string(),
                kind: ConceptKind::Function,
                source_file: "src/engine.rs".to_string(),
                line: Some(2), description: None,
            },
        ];

        let manifest = make_manifest(vec![Page {
            route: "/docs/engine".to_string(),
            file: Some("pages/engine.md".to_string()),
            title: "Engine".to_string(),
            tags: vec![],
            sources: vec![Source {
                path: "src/engine.rs".to_string(),
                sections: vec![],
            }],
            related: vec![],
            verified_at: None,
            status: Status::Unverified,
            concepts: vec![],
        }]);

        let graph = build_graph_from_concepts(&manifest, &all_concepts, Some(dir.path()));

        let thin = graph
            .thin_coverage
            .iter()
            .find(|t| t.route == "/docs/engine");
        assert!(
            thin.is_some(),
            "Expected thin coverage for /docs/engine, got: {:?}",
            graph.thin_coverage
        );
        let thin = thin.unwrap();
        assert!(
            thin.missing_concepts.contains(&"stop_engine".to_string()),
            "Expected stop_engine in missing_concepts, got: {:?}",
            thin.missing_concepts
        );
    }

    #[test]
    fn stale_siblings_detected_for_different_verified_shas() {
        let dir = tempfile::tempdir().unwrap();

        let pages_dir = dir.path().join("pages");
        std::fs::create_dir_all(&pages_dir).unwrap();
        std::fs::write(
            pages_dir.join("page_a.md"),
            "# Page A\n\nUse Config to set up.\n",
        )
        .unwrap();
        std::fs::write(
            pages_dir.join("page_b.md"),
            "# Page B\n\nConfig controls behavior.\n",
        )
        .unwrap();

        let all_concepts = vec![Concept {
            name: "Config".to_string(),
            kind: ConceptKind::Struct,
            source_file: "src/shared.rs".to_string(),
            line: Some(1), description: None,
        }];

        let manifest = make_manifest(vec![
            Page {
                route: "/docs/a".to_string(),
                file: Some("pages/page_a.md".to_string()),
                title: "Page A".to_string(),
                tags: vec![],
                sources: vec![Source {
                    path: "src/shared.rs".to_string(),
                    sections: vec![],
                }],
                related: vec![],
                verified_at: Some(VerifiedAt {
                    sha: "aaa1111".to_string(),
                    timestamp: "2025-01-01T00:00:00Z".to_string(),
                }),
                status: Status::Current,
                concepts: vec![],
            },
            Page {
                route: "/docs/b".to_string(),
                file: Some("pages/page_b.md".to_string()),
                title: "Page B".to_string(),
                tags: vec![],
                sources: vec![Source {
                    path: "src/shared.rs".to_string(),
                    sections: vec![],
                }],
                related: vec![],
                verified_at: Some(VerifiedAt {
                    sha: "bbb2222".to_string(),
                    timestamp: "2025-06-01T00:00:00Z".to_string(),
                }),
                status: Status::Current,
                concepts: vec![],
            },
        ]);

        let graph = build_graph_from_concepts(&manifest, &all_concepts, Some(dir.path()));

        let sibling = graph
            .stale_siblings
            .iter()
            .find(|s| s.concept == "Config");
        assert!(
            sibling.is_some(),
            "Expected Config as stale sibling, got: {:?}",
            graph.stale_siblings
        );
        assert_eq!(sibling.unwrap().pages.len(), 2);
    }

    #[test]
    fn graph_stats_are_accurate() {
        let all_concepts = vec![Concept {
            name: "helper".to_string(),
            kind: ConceptKind::Function,
            source_file: "src/lib.rs".to_string(),
            line: Some(1), description: None,
        }];

        let manifest = make_manifest(vec![]);

        let graph = build_graph_from_concepts(&manifest, &all_concepts, None);

        assert_eq!(graph.stats.total_pages, 0);
        assert_eq!(graph.stats.total_concepts, 1);
        assert_eq!(graph.stats.orphan_count, graph.orphans.len());
        assert_eq!(graph.stats.thin_coverage_count, graph.thin_coverage.len());
        assert_eq!(
            graph.stats.stale_sibling_count,
            graph.stale_siblings.len()
        );
    }

    #[test]
    fn dot_output_has_valid_syntax() {
        let graph = ConceptGraph {
            nodes: vec![
                ConceptNode {
                    name: "AuthConfig".to_string(),
                    source_files: vec!["src/auth.rs".to_string()],
                    pages: vec!["/docs/auth".to_string()],
                },
                ConceptNode {
                    name: "OrphanThing".to_string(),
                    source_files: vec!["src/orphan.rs".to_string()],
                    pages: vec![],
                },
            ],
            orphans: vec![OrphanDetail {
                name: "OrphanThing".to_string(),
                kind: "struct".to_string(),
                source_file: "src/orphan.rs".to_string(),
            }],
            thin_coverage: vec![],
            stale_siblings: vec![],
            stats: GraphStats {
                total_concepts: 2,
                total_pages: 1,
                orphan_count: 1,
                thin_coverage_count: 0,
                stale_sibling_count: 0,
            },
        };

        let dot = crate::report::format_concept_graph_dot(&graph);
        assert!(dot.contains("digraph concepts"), "must start with digraph");
        assert!(dot.contains("->"), "must have edges");
        assert!(
            dot.contains("color=red"),
            "orphans must be highlighted in red"
        );
        assert!(dot.contains("shape=box"), "pages must be boxes");
        assert!(dot.contains("shape=circle"), "concepts must be circles");
    }
}
