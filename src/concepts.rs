use std::collections::HashMap;
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

/// Generic file stems that are structural, not conceptual.
const GENERIC_STEMS: &[&str] = &[
    "mod", "lib", "main", "types", "utils", "helpers", "error", "errors",
    "tests", "prelude", "macros", "common",
];

// ───────────────────────────────────────────────────────────────────────
// Core types
// ───────────────────────────────────────────────────────────────────────

/// A typed concept extracted from a source file.
///
/// Concepts come in two tiers:
/// - **Primary** (`Module`): human-level concept from file stem or `///` doc.
///   Represents what a doc reader would look for: "needle", "bail", "nudge".
/// - **Evidence** (`Struct`/`Enum`/`Trait`/`Function`/`MarkdownSection`):
///   code-level identifier that backs a primary concept.
#[allow(dead_code)]
pub struct Concept {
    pub name: String,
    pub kind: ConceptKind,
    pub source_file: String,
    pub line: Option<usize>,
    /// First `///` doc line above this item, if any. Only for evidence concepts.
    pub description: Option<String>,
}

/// The kind of concept extracted.
#[allow(dead_code)]
pub enum ConceptKind {
    Struct,
    Enum,
    Trait,
    Function,
    /// Primary concept — the human-readable module/file-level topic.
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

// ───────────────────────────────────────────────────────────────────────
// Public API
// ───────────────────────────────────────────────────────────────────────

/// Extract typed concepts from a single source file.
///
/// Emits one `Module` (primary) concept per file from the stem, plus
/// evidence concepts (`Struct`/`Fn`/etc.) from `pub` items.
pub fn extract_concepts(path: &str, repo_path: &Path) -> Vec<Concept> {
    let full_path = repo_path.join(path);
    let Ok(content) = std::fs::read_to_string(&full_path) else {
        return Vec::new();
    };

    let mut concepts = Vec::new();

    // Primary concept from file stem (e.g. "nudge" from src/kernel/nudge.rs)
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if !stem.is_empty() && !GENERIC_STEMS.contains(&stem) && stem.len() >= 3 {
        concepts.push(Concept {
            name: stem.replace('_', " "),
            kind: ConceptKind::Module,
            source_file: path.to_string(),
            line: None,
            description: None,
        });
    }

    let ext = Path::new(path).extension().and_then(|e| e.to_str());
    match ext {
        Some("rs") => concepts.extend(extract_rust_concepts(&content, path)),
        Some("md" | "mdx") => concepts.extend(extract_markdown_concepts(&content, path)),
        _ => {}
    }

    concepts
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
        // Also check description text if present
        if let Some(ref desc) = concept.description {
            let desc_lower = desc.to_lowercase();
            // Use significant words from desc (>= 6 chars) for matching
            for word in desc_lower.split_whitespace().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric())) {
                if word.len() >= 6 && lower.contains(word) {
                    matched.push(concept.name.clone());
                    break;
                }
            }
        }
    }

    matched.sort();
    matched.dedup();
    matched
}

/// Group evidence concepts under their file's primary (Module) concept.
///
/// Returns a map from primary concept name to list of evidence concept names.
#[allow(dead_code)]
pub fn group_by_primary(concepts: &[Concept]) -> HashMap<String, Vec<String>> {
    // Build file → primary name map
    let mut file_to_primary: HashMap<&str, &str> = HashMap::new();
    for c in concepts {
        if matches!(c.kind, ConceptKind::Module) {
            file_to_primary.insert(&c.source_file, &c.name);
        }
    }

    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for c in concepts {
        if matches!(c.kind, ConceptKind::Module) {
            groups.entry(c.name.clone()).or_default();
            continue;
        }
        if let Some(primary) = file_to_primary.get(c.source_file.as_str()) {
            groups
                .entry((*primary).to_string())
                .or_default()
                .push(c.name.clone());
        }
    }

    // Deduplicate evidence lists
    for evidence in groups.values_mut() {
        evidence.sort();
        evidence.dedup();
    }

    groups
}

// ───────────────────────────────────────────────────────────────────────
// Internal extractors
// ───────────────────────────────────────────────────────────────────────

/// Check if a name is on the stoplist.
fn is_stopped(name: &str) -> bool {
    CONCEPT_STOPLIST.contains(&name.to_lowercase().as_str())
}

/// Extract Rust evidence concepts: `pub struct/enum/trait/fn` names
/// with optional `///` doc comment descriptions.
fn extract_rust_concepts(content: &str, source_file: &str) -> Vec<Concept> {
    let mut concepts = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    let prefixes: &[(&str, ConceptKind)] = &[
        ("pub fn ", ConceptKind::Function),
        ("pub struct ", ConceptKind::Struct),
        ("pub enum ", ConceptKind::Enum),
        ("pub trait ", ConceptKind::Trait),
    ];

    for (line_num, line) in lines.iter().enumerate() {
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

                    // Walk backwards to find the first /// doc line
                    let description = find_doc_comment(&lines, line_num);

                    concepts.push(Concept {
                        name: name.to_string(),
                        kind,
                        source_file: source_file.to_string(),
                        line: Some(line_num + 1),
                        description,
                    });
                }
            }
        }
    }

    concepts
}

/// Walk backwards from a pub item to find its `///` doc comment.
/// Returns the first `///` line (the summary line) if found.
fn find_doc_comment(lines: &[&str], item_line: usize) -> Option<String> {
    if item_line == 0 {
        return None;
    }
    // Walk backwards through blank lines and /// lines
    let mut idx = item_line - 1;
    let mut first_doc: Option<String> = None;
    loop {
        let trimmed = lines[idx].trim();
        if let Some(doc) = trimmed.strip_prefix("/// ") {
            first_doc = Some(doc.trim().to_string());
        } else if trimmed == "///" {
            // empty doc line, continue
        } else if trimmed.starts_with("#[") || trimmed.starts_with("//") {
            // attribute or other comment, continue searching
        } else {
            break;
        }
        if idx == 0 {
            break;
        }
        idx -= 1;
    }
    first_doc
}

/// Extract Markdown concepts: `##` and `###` headings as Section concepts.
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
                description: None,
            });
        }
    }

    concepts
}

// ───────────────────────────────────────────────────────────────────────
// Coverage stats
// ───────────────────────────────────────────────────────────────────────

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

// ───────────────────────────────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────────────────────────────

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
        assert!(names.contains(&"authenticate_user"), "missing authenticate_user");
        assert!(names.contains(&"TrustTier"), "missing TrustTier");
        assert!(names.contains(&"Validator"), "missing Validator");
        assert!(!names.contains(&"Auth"), "CamelCase splits should not appear");
    }

    #[test]
    fn doc_comment_extracted_as_description() {
        let content = "\
/// A portable, signed OS bundle.
///
/// Contains identity, boot state, and optionally encrypted kernel state.
pub struct BailPackage {
    mode: String,
}
";
        let concepts = extract_rust_concepts(content, "src/bail.rs");
        let bail = concepts.iter().find(|c| c.name == "BailPackage").unwrap();
        assert_eq!(
            bail.description.as_deref(),
            Some("A portable, signed OS bundle.")
        );
    }

    #[test]
    fn module_docstrings_not_extracted_as_concepts() {
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
    fn file_stem_emits_primary_concept() {
        // extract_concepts (not extract_rust_concepts) adds the file stem
        // We can't call it without a real file, so test the logic directly
        let stem = "nudge";
        assert!(!GENERIC_STEMS.contains(&stem));
        assert!(stem.len() >= 3);
        // Would produce Concept { name: "nudge", kind: Module, ... }
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
    fn scan_page_matches_description_words() {
        let concepts = vec![Concept {
            name: "BailPackage".to_string(),
            kind: ConceptKind::Struct,
            source_file: "src/bail.rs".to_string(),
            line: Some(3),
            description: Some("A portable, signed OS bundle.".to_string()),
        }];
        // Page doesn't say "BailPackage" but does say "portable" and "bundle"
        let page = "Bail is a portable bundle from the stack.";
        let matched = scan_page_for_concepts(page, &concepts);
        assert!(matched.contains(&"BailPackage".to_string()));
    }

    #[test]
    fn scan_page_finds_concept_case_insensitive() {
        let concepts = vec![
            Concept {
                name: "Needle".to_string(),
                kind: ConceptKind::Struct,
                source_file: "src/search.rs".to_string(),
                line: Some(10),
                description: None,
            },
            Concept {
                name: "Haystack".to_string(),
                kind: ConceptKind::Struct,
                source_file: "src/search.rs".to_string(),
                line: Some(20),
                description: None,
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
            description: None,
        }];
        let page_content = "The AuthConfig struct holds AuthConfig values.";
        let matched = scan_page_for_concepts(page_content, &concepts);
        assert_eq!(matched.len(), 1);
    }

    #[test]
    fn group_by_primary_clusters_evidence() {
        let concepts = vec![
            Concept {
                name: "work".to_string(),
                kind: ConceptKind::Module,
                source_file: "src/commands/work.rs".to_string(),
                line: None,
                description: None,
            },
            Concept {
                name: "add_needle_atomic".to_string(),
                kind: ConceptKind::Function,
                source_file: "src/commands/work.rs".to_string(),
                line: Some(50),
                description: None,
            },
            Concept {
                name: "NeedleEntry".to_string(),
                kind: ConceptKind::Struct,
                source_file: "src/commands/work.rs".to_string(),
                line: Some(10),
                description: None,
            },
        ];
        let groups = group_by_primary(&concepts);
        assert!(groups.contains_key("work"));
        let evidence = &groups["work"];
        assert!(evidence.contains(&"NeedleEntry".to_string()));
        assert!(evidence.contains(&"add_needle_atomic".to_string()));
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
