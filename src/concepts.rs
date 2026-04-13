use std::fmt;
use std::path::Path;

/// Common names that appear everywhere and carry no semantic value as concepts.
const CONCEPT_STOPLIST: &[&str] = &[
    "new", "run", "fmt", "display", "default", "from", "into", "try_from",
    "error", "result", "build", "init", "drop", "clone", "debug", "serialize",
    "deserialize", "write", "read", "flush", "close", "open", "get", "set",
    "len", "is_empty", "iter", "next", "map", "filter", "collect", "push",
    "pop", "insert", "remove", "contains", "main", "test", "setup", "handle",
    "process", "update", "create", "delete", "list", "show", "help", "status",
    "config", "context", "state", "data", "info", "value", "entry", "item",
    "node", "path", "name", "kind", "type", "mode", "level", "action",
    "format", "parse", "load", "save", "start", "stop", "with", "apply",
];

/// Generic markdown headings that are structural, not conceptual.
const GENERIC_HEADINGS: &[&str] = &[
    "overview", "usage", "example", "examples", "notes", "see also",
    "references", "introduction", "summary", "background", "context",
    "future work", "non-goals", "design decision", "how it works",
    "why this module exists", "safety / correctness", "top-level contract",
    "purpose", "modes", "gating", "actions", "implementation",
    "testing", "changelog", "license", "contributing", "appendix",
    "getting started",
];

/// A typed concept extracted from a source file.
#[allow(dead_code)]
pub struct Concept {
    pub name: String,
    pub kind: ConceptKind,
    pub source_file: String,
    pub line: Option<usize>,
}

/// The kind of concept extracted.
#[allow(dead_code)]
pub enum ConceptKind {
    Struct,
    Enum,
    Trait,
    Function,
    Module,
    DocComment,
    MarkdownSection,
}

impl fmt::Display for ConceptKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConceptKind::Struct => write!(f, "struct"),
            ConceptKind::Enum => write!(f, "enum"),
            ConceptKind::Trait => write!(f, "trait"),
            ConceptKind::Function => write!(f, "function"),
            ConceptKind::Module => write!(f, "module"),
            ConceptKind::DocComment => write!(f, "doc_comment"),
            ConceptKind::MarkdownSection => write!(f, "section"),
        }
    }
}

/// Extract typed concepts from a single source file.
pub fn extract_concepts(path: &str, repo_path: &Path) -> Vec<Concept> {
    let full_path = repo_path.join(path);
    let Ok(content) = std::fs::read_to_string(&full_path) else {
        return Vec::new();
    };

    let ext = Path::new(path).extension().and_then(|e| e.to_str());
    match ext {
        Some("rs") => extract_rust_concepts(&content, path),
        Some("md" | "mdx") => extract_markdown_concepts(&content, path),
        _ => Vec::new(),
    }
}

/// Extract concepts from all source files.
pub fn extract_all_concepts(source_files: &[String], repo_path: &Path) -> Vec<Concept> {
    source_files
        .iter()
        .flat_map(|f| extract_concepts(f, repo_path))
        .collect()
}

/// Case-insensitive scan of page content for concept names.
/// Returns names of matched concepts.
pub fn scan_page_for_concepts(page_content: &str, concepts: &[Concept]) -> Vec<String> {
    let lower = page_content.to_lowercase();
    let mut matched = Vec::new();

    for concept in concepts {
        let name_lower = concept.name.to_lowercase();
        if lower.contains(&name_lower) {
            matched.push(concept.name.clone());
        }
    }

    matched.sort();
    matched.dedup();
    matched
}

/// Check if a name is on the stoplist.
fn is_stopped(name: &str) -> bool {
    CONCEPT_STOPLIST.contains(&name.to_lowercase().as_str())
}

/// Extract Rust concepts: `pub struct/enum/trait/fn` names only.
///
/// Dropped from v1: module docstrings (`//!` lines) were too noisy
/// (prose sentences, not concept names). `CamelCase` splits (e.g.
/// `AuthConfig` generating separate Auth and Config concepts) flooded
/// the graph with generic words.
fn extract_rust_concepts(content: &str, source_file: &str) -> Vec<Concept> {
    let mut concepts = Vec::new();

    let prefixes: &[(&str, ConceptKind)] = &[
        ("pub fn ", ConceptKind::Function),
        ("pub struct ", ConceptKind::Struct),
        ("pub enum ", ConceptKind::Enum),
        ("pub trait ", ConceptKind::Trait),
    ];

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        for (prefix, kind_template) in prefixes {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let name = rest
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if name.len() >= 4 && !is_stopped(name) {
                    let kind = match kind_template {
                        ConceptKind::Function => ConceptKind::Function,
                        ConceptKind::Struct => ConceptKind::Struct,
                        ConceptKind::Enum => ConceptKind::Enum,
                        ConceptKind::Trait => ConceptKind::Trait,
                        _ => continue,
                    };
                    concepts.push(Concept {
                        name: name.to_string(),
                        kind,
                        source_file: source_file.to_string(),
                        line: Some(line_num + 1),
                    });
                }
            }
        }
    }

    concepts
}

/// Extract Markdown concepts: `##` and `###` headings as Section concepts.
///
/// Filters: skip headings with > 6 words (prose descriptions, not concept
/// names) and generic structural headings (Overview, Usage, etc.).
fn extract_markdown_concepts(content: &str, source_file: &str) -> Vec<Concept> {
    let mut concepts = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        if let Some(heading) = trimmed
            .strip_prefix("### ")
            .or_else(|| trimmed.strip_prefix("## "))
        {
            let heading = heading.trim();
            if heading.is_empty() {
                continue;
            }

            let word_count = heading.split_whitespace().count();
            if word_count > 6 {
                continue;
            }

            let lower = heading.to_lowercase();
            if GENERIC_HEADINGS
                .iter()
                .any(|g| lower == *g || lower.starts_with(g))
            {
                continue;
            }

            concepts.push(Concept {
                name: heading.to_string(),
                kind: ConceptKind::MarkdownSection,
                source_file: source_file.to_string(),
                line: Some(line_num + 1),
            });
        }
    }

    concepts
}

/// Stats for concept-level coverage.
pub struct ConceptCoverageStats {
    pub total: usize,
    pub covered: usize,
    pub orphans: Vec<OrphanConcept>,
}

/// A concept that exists in source but is not mentioned in any doc page.
pub struct OrphanConcept {
    pub name: String,
    pub kind: String,
    pub source_file: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_rust_struct_and_fn() {
        let content = "\
pub struct AuthConfig {
    token: String,
}

pub fn authenticate_user(req: &Request) -> bool {
    true
}

pub enum TrustTier {
    Active,
    Inactive,
}

pub trait Validator {
    fn validate(&self) -> bool;
}
";
        let concepts = extract_rust_concepts(content, "src/auth.rs");
        let names: Vec<&str> = concepts.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"AuthConfig"), "missing AuthConfig");
        assert!(
            names.contains(&"authenticate_user"),
            "missing authenticate_user"
        );
        assert!(names.contains(&"TrustTier"), "missing TrustTier");
        assert!(names.contains(&"Validator"), "missing Validator");

        // No CamelCase splits (dropped in v2)
        assert!(!names.contains(&"Auth"), "CamelCase splits should not appear");

        let auth_config = concepts.iter().find(|c| c.name == "AuthConfig").unwrap();
        assert_eq!(auth_config.line, Some(1));
        assert!(matches!(auth_config.kind, ConceptKind::Struct));
    }

    #[test]
    fn module_docstrings_not_extracted() {
        let content = "\
//! This module handles authentication flows.
//! It supports OAuth and API keys.

pub fn login() {}
";
        let concepts = extract_rust_concepts(content, "src/auth.rs");
        let modules: Vec<&Concept> = concepts
            .iter()
            .filter(|c| matches!(c.kind, ConceptKind::Module))
            .collect();
        assert_eq!(modules.len(), 0);
    }

    #[test]
    fn extract_markdown_sections() {
        let content = "\
# Top Level
## Getting Started
Some text here.
### Installation
More text.
## API Reference
## Overview
## This heading has way too many words to be a concept name
";
        let concepts = extract_markdown_concepts(content, "docs/guide.md");
        let names: Vec<&str> = concepts.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Installation"));
        assert!(names.contains(&"API Reference"));
        assert!(!names.contains(&"Top Level"));
        assert!(!names.contains(&"Getting Started"));
        assert!(!names.contains(&"Overview"));
        assert!(
            !names.iter().any(|n| n.contains("way too many")),
            "long headings should be filtered"
        );
    }

    #[test]
    fn scan_page_finds_concept_case_insensitive() {
        let concepts = vec![
            Concept {
                name: "Needle".to_string(),
                kind: ConceptKind::Struct,
                source_file: "src/search.rs".to_string(),
                line: Some(10),
            },
            Concept {
                name: "Haystack".to_string(),
                kind: ConceptKind::Struct,
                source_file: "src/search.rs".to_string(),
                line: Some(20),
            },
        ];

        let page_content = "A needle is an atomic work item used for searching.";
        let matched = scan_page_for_concepts(page_content, &concepts);
        assert!(matched.contains(&"Needle".to_string()));
        assert!(!matched.contains(&"Haystack".to_string()));
    }

    #[test]
    fn scan_page_deduplicates() {
        let concepts = vec![Concept {
            name: "AuthConfig".to_string(),
            kind: ConceptKind::Struct,
            source_file: "src/config.rs".to_string(),
            line: Some(1),
        }];

        let page_content = "The AuthConfig struct holds AuthConfig values.";
        let matched = scan_page_for_concepts(page_content, &concepts);
        assert_eq!(matched.len(), 1);
    }

    #[test]
    fn concept_kind_display() {
        assert_eq!(ConceptKind::Struct.to_string(), "struct");
        assert_eq!(ConceptKind::Enum.to_string(), "enum");
        assert_eq!(ConceptKind::Trait.to_string(), "trait");
        assert_eq!(ConceptKind::Function.to_string(), "function");
        assert_eq!(ConceptKind::Module.to_string(), "module");
        assert_eq!(ConceptKind::DocComment.to_string(), "doc_comment");
        assert_eq!(ConceptKind::MarkdownSection.to_string(), "section");
    }

    #[test]
    fn stoplist_filters_generic_names() {
        let content = "pub fn new() {}\npub fn init() {}\npub struct Config {}\npub fn build_scheduler() {}\n";
        let concepts = extract_rust_concepts(content, "src/generic.rs");
        let names: Vec<&str> = concepts.iter().map(|c| c.name.as_str()).collect();
        assert!(!names.contains(&"init"));
        assert!(names.contains(&"build_scheduler"));
    }

    #[test]
    fn short_names_skipped() {
        let content = "pub fn go() {}\npub struct OK {}\npub fn run() {}\n";
        let concepts = extract_rust_concepts(content, "src/tiny.rs");
        assert!(
            concepts.is_empty(),
            "names shorter than 4 chars or on stoplist should be skipped"
        );
    }
}
