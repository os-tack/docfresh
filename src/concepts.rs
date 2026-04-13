use std::fmt;
use std::path::Path;

use crate::suggest::split_camel_case;

/// A typed concept extracted from a source file.
#[allow(dead_code)]
pub struct Concept {
    pub name: String,
    pub kind: ConceptKind,
    pub source_file: String,
    pub line: Option<usize>,
}

/// The kind of concept extracted.
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

/// Extract Rust concepts: pub struct/enum/trait/fn and module docstrings.
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

        // Public items — reuses the pattern from suggest.rs extract_rust_terms
        for (prefix, kind_template) in prefixes {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let name = rest
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if name.len() >= 3 {
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

                    // Also add CamelCase splits as DocComment concepts
                    for word in split_camel_case(name) {
                        if word.len() >= 4 {
                            concepts.push(Concept {
                                name: word,
                                kind: ConceptKind::DocComment,
                                source_file: source_file.to_string(),
                                line: Some(line_num + 1),
                            });
                        }
                    }
                }
            }
        }

        // Module docstrings (//! lines)
        if let Some(doc) = trimmed.strip_prefix("//! ") {
            let doc = doc.trim();
            if doc.len() >= 4 {
                concepts.push(Concept {
                    name: doc.to_string(),
                    kind: ConceptKind::Module,
                    source_file: source_file.to_string(),
                    line: Some(line_num + 1),
                });
            }
        }
    }

    concepts
}

/// Extract Markdown concepts: ## and ### headings as Section concepts.
fn extract_markdown_concepts(content: &str, source_file: &str) -> Vec<Concept> {
    let mut concepts = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        if let Some(heading) = trimmed
            .strip_prefix("### ")
            .or_else(|| trimmed.strip_prefix("## "))
        {
            let heading = heading.trim();
            if !heading.is_empty() {
                concepts.push(Concept {
                    name: heading.to_string(),
                    kind: ConceptKind::MarkdownSection,
                    source_file: source_file.to_string(),
                    line: Some(line_num + 1),
                });
            }
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

pub enum Status {
    Active,
    Inactive,
}

pub trait Validator {
    fn validate(&self) -> bool;
}
";
        let concepts = extract_rust_concepts(content, "src/auth.rs");
        let names: Vec<&str> = concepts
            .iter()
            .filter(|c| !matches!(c.kind, ConceptKind::DocComment))
            .map(|c| c.name.as_str())
            .collect();
        assert!(names.contains(&"AuthConfig"), "missing AuthConfig");
        assert!(
            names.contains(&"authenticate_user"),
            "missing authenticate_user"
        );
        assert!(names.contains(&"Status"), "missing Status");
        assert!(names.contains(&"Validator"), "missing Validator");

        // Check line numbers
        let auth_config = concepts.iter().find(|c| c.name == "AuthConfig").unwrap();
        assert_eq!(auth_config.line, Some(1));
        assert!(matches!(auth_config.kind, ConceptKind::Struct));

        let auth_fn = concepts
            .iter()
            .find(|c| c.name == "authenticate_user")
            .unwrap();
        assert_eq!(auth_fn.line, Some(5));
        assert!(matches!(auth_fn.kind, ConceptKind::Function));
    }

    #[test]
    fn extract_module_docstrings() {
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
        assert_eq!(modules.len(), 2);
        assert_eq!(
            modules[0].name,
            "This module handles authentication flows."
        );
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
";
        let concepts = extract_markdown_concepts(content, "docs/guide.md");
        let names: Vec<&str> = concepts.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Getting Started"));
        assert!(names.contains(&"Installation"));
        assert!(names.contains(&"API Reference"));
        // H1 is not extracted
        assert!(!names.contains(&"Top Level"));
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
            name: "Config".to_string(),
            kind: ConceptKind::Struct,
            source_file: "src/config.rs".to_string(),
            line: Some(1),
        }];

        let page_content = "The Config struct holds Config values for Config.";
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
    fn extract_concepts_skips_short_names() {
        let content = "pub fn go() {}\npub struct OK {}\n";
        let concepts = extract_rust_concepts(content, "src/tiny.rs");
        let primary: Vec<&Concept> = concepts
            .iter()
            .filter(|c| !matches!(c.kind, ConceptKind::DocComment))
            .collect();
        assert!(primary.is_empty(), "names shorter than 3 chars should be skipped");
    }
}
