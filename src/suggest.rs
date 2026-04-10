use crate::manifest::Manifest;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A suggestion mapping a source file to a page with a confidence score.
pub struct Suggestion {
    pub source_path: String,
    pub route: String,
    pub confidence: f64,
    pub reasons: Vec<String>,
}

/// Result of running suggest across all unmapped source files.
pub struct SuggestReport {
    pub suggestions: Vec<Suggestion>,
    pub no_match: Vec<String>,
}

/// Extract terms from a source file for matching against page content.
fn extract_source_terms(path: &str, repo_path: &Path) -> Vec<String> {
    let mut terms = Vec::new();

    // Path stem — the most identifying part
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    // Skip generic stems
    let generic = [
        "mod", "lib", "main", "types", "utils", "helpers", "error", "errors",
    ];
    if !generic.contains(&stem) && !stem.is_empty() {
        terms.push(stem.to_lowercase());
        // Also add with underscores replaced by hyphens (bail_merge -> bail-merge)
        if stem.contains('_') {
            terms.push(stem.replace('_', "-").to_lowercase());
            // And individual segments (bail_merge -> bail, merge)
            for part in stem.split('_') {
                if part.len() >= 3 {
                    terms.push(part.to_lowercase());
                }
            }
        }
    }

    // Parent directory as context
    if let Some(parent) = Path::new(path).parent() {
        if let Some(dir_name) = parent.file_name().and_then(|s| s.to_str()) {
            let skip_dirs = [
                "src", "docs", "spec", "commands", "kernel", "serve", "tools",
            ];
            if !skip_dirs.contains(&dir_name) {
                terms.push(dir_name.to_lowercase());
            }
        }
    }

    // Read file content for public item names and doc comments
    let full_path = repo_path.join(path);
    if let Ok(content) = std::fs::read_to_string(&full_path) {
        let ext = Path::new(path).extension().and_then(|e| e.to_str());
        match ext {
            Some("rs") => extract_rust_terms(&content, &mut terms),
            Some("go") => extract_go_terms(&content, &mut terms),
            Some("py") => extract_python_terms(&content, &mut terms),
            Some("ts" | "tsx" | "js" | "jsx") => extract_typescript_terms(&content, &mut terms),
            Some("java") => extract_java_terms(&content, &mut terms),
            Some("cs") => extract_csharp_terms(&content, &mut terms),
            Some("rb") => extract_ruby_terms(&content, &mut terms),
            Some("php") => extract_php_terms(&content, &mut terms),
            Some("cpp" | "cc" | "h" | "hpp") => extract_cpp_terms(&content, &mut terms),
            Some("md" | "mdx" | "rst") => extract_markdown_terms(&content, &mut terms),
            _ => {}
        }
    }

    terms.sort();
    terms.dedup();
    terms
}

fn extract_rust_terms(content: &str, terms: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        // Public items
        for prefix in ["pub fn ", "pub struct ", "pub enum ", "pub trait "] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let name = rest
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if name.len() >= 3 {
                    terms.push(name.to_lowercase());
                    // Also split CamelCase
                    for word in split_camel_case(name) {
                        if word.len() >= 3 {
                            terms.push(word.to_lowercase());
                        }
                    }
                }
            }
        }
        // Doc comments
        if let Some(doc) = trimmed
            .strip_prefix("/// ")
            .or(trimmed.strip_prefix("//! "))
        {
            for word in doc.split_whitespace() {
                let clean = word
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase();
                if clean.len() >= 4 {
                    terms.push(clean);
                }
            }
        }
    }
}

fn extract_markdown_terms(content: &str, terms: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ").or(trimmed.strip_prefix("## ")) {
            for word in heading.split_whitespace() {
                let clean = word
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase();
                if clean.len() >= 3 {
                    terms.push(clean);
                }
            }
        }
    }
}

fn extract_go_terms(content: &str, terms: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        // Exported functions and types (uppercase first letter)
        for prefix in ["func ", "type "] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                // Skip methods: func (r *Receiver) Name(...)
                let name_part = if rest.starts_with('(') {
                    // Method — find the name after the receiver
                    rest.split(')').nth(1).unwrap_or("").trim()
                } else {
                    rest
                };
                let name = name_part
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if name.len() >= 3 && name.starts_with(|c: char| c.is_uppercase()) {
                    terms.push(name.to_lowercase());
                    for word in split_camel_case(name) {
                        if word.len() >= 3 {
                            terms.push(word.to_lowercase());
                        }
                    }
                }
            }
        }
        // Go doc comments
        if let Some(doc) = trimmed.strip_prefix("// ") {
            extract_doc_words(doc, terms);
        }
    }
}

fn extract_python_terms(content: &str, terms: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        // Functions and classes
        for prefix in ["def ", "class "] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let name = rest
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                // Skip private (single underscore is conventional private)
                if name.len() >= 3 && !name.starts_with('_') {
                    terms.push(name.to_lowercase());
                    for part in name.split('_') {
                        if part.len() >= 3 {
                            terms.push(part.to_lowercase());
                        }
                    }
                }
            }
        }
        // Docstrings (first line of triple-quoted strings)
        if let Some(doc) = trimmed
            .strip_prefix("\"\"\"")
            .or(trimmed.strip_prefix("'''"))
        {
            extract_doc_words(doc, terms);
        }
    }
}

fn extract_typescript_terms(content: &str, terms: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        // Exported items
        if let Some(rest) = trimmed.strip_prefix("export ") {
            let rest = rest.strip_prefix("default ").unwrap_or(rest);
            for keyword in [
                "function ",
                "class ",
                "interface ",
                "type ",
                "const ",
                "enum ",
                "async function ",
            ] {
                if let Some(name_rest) = rest.strip_prefix(keyword) {
                    let name = name_rest
                        .split(|c: char| !c.is_alphanumeric() && c != '_')
                        .next()
                        .unwrap_or("");
                    if name.len() >= 3 {
                        terms.push(name.to_lowercase());
                        for word in split_camel_case(name) {
                            if word.len() >= 3 {
                                terms.push(word.to_lowercase());
                            }
                        }
                    }
                    break;
                }
            }
        }
        // JSDoc comments
        if let Some(doc) = trimmed.strip_prefix("* ").or(trimmed.strip_prefix("/** ")) {
            if !doc.starts_with('@') {
                extract_doc_words(doc, terms);
            }
        }
    }
}

fn extract_java_terms(content: &str, terms: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        // Public items
        if let Some(rest) = trimmed.strip_prefix("public ") {
            let rest = rest
                .strip_prefix("static ")
                .unwrap_or(rest)
                .strip_prefix("final ")
                .unwrap_or(rest)
                .strip_prefix("abstract ")
                .unwrap_or(rest);
            for keyword in ["class ", "interface ", "enum ", "record "] {
                if let Some(name_rest) = rest.strip_prefix(keyword) {
                    let name = name_rest
                        .split(|c: char| !c.is_alphanumeric() && c != '_')
                        .next()
                        .unwrap_or("");
                    if name.len() >= 3 {
                        terms.push(name.to_lowercase());
                        for word in split_camel_case(name) {
                            if word.len() >= 3 {
                                terms.push(word.to_lowercase());
                            }
                        }
                    }
                    break;
                }
            }
        }
        // Javadoc
        if let Some(doc) = trimmed.strip_prefix("* ").or(trimmed.strip_prefix("/** ")) {
            if !doc.starts_with('@') {
                extract_doc_words(doc, terms);
            }
        }
    }
}

fn extract_csharp_terms(content: &str, terms: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("public ") {
            let rest = rest
                .strip_prefix("static ")
                .unwrap_or(rest)
                .strip_prefix("sealed ")
                .unwrap_or(rest)
                .strip_prefix("abstract ")
                .unwrap_or(rest)
                .strip_prefix("partial ")
                .unwrap_or(rest);
            for keyword in ["class ", "interface ", "enum ", "struct ", "record "] {
                if let Some(name_rest) = rest.strip_prefix(keyword) {
                    let name = name_rest
                        .split(|c: char| !c.is_alphanumeric() && c != '_')
                        .next()
                        .unwrap_or("");
                    if name.len() >= 3 {
                        terms.push(name.to_lowercase());
                        for word in split_camel_case(name) {
                            if word.len() >= 3 {
                                terms.push(word.to_lowercase());
                            }
                        }
                    }
                    break;
                }
            }
        }
        // XML doc comments
        if let Some(doc) = trimmed.strip_prefix("/// ") {
            // Strip XML tags
            let text = doc
                .replace(['<', '>'], " ")
                .replace("/summary", "")
                .replace("summary", "");
            extract_doc_words(&text, terms);
        }
    }
}

fn extract_ruby_terms(content: &str, terms: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        for prefix in ["def ", "class ", "module "] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let rest = rest.strip_prefix("self.").unwrap_or(rest);
                let name = rest
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if name.len() >= 3 {
                    terms.push(name.to_lowercase());
                    for part in name.split('_') {
                        if part.len() >= 3 {
                            terms.push(part.to_lowercase());
                        }
                    }
                }
            }
        }
        // YARD doc comments
        if let Some(doc) = trimmed.strip_prefix("# ") {
            if !doc.starts_with('@') {
                extract_doc_words(doc, terms);
            }
        }
    }
}

fn extract_php_terms(content: &str, terms: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed
            .strip_prefix("public ")
            .or(trimmed.strip_prefix("function "))
        {
            let rest = rest
                .strip_prefix("static ")
                .unwrap_or(rest)
                .strip_prefix("function ")
                .unwrap_or(rest);
            let name = rest
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("");
            if name.len() >= 3 {
                terms.push(name.to_lowercase());
                for word in split_camel_case(name) {
                    if word.len() >= 3 {
                        terms.push(word.to_lowercase());
                    }
                }
            }
        }
        for prefix in ["class ", "interface ", "trait ", "enum "] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let name = rest
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if name.len() >= 3 {
                    terms.push(name.to_lowercase());
                    for word in split_camel_case(name) {
                        if word.len() >= 3 {
                            terms.push(word.to_lowercase());
                        }
                    }
                }
            }
        }
        // PHPDoc
        if let Some(doc) = trimmed.strip_prefix("* ").or(trimmed.strip_prefix("/** ")) {
            if !doc.starts_with('@') {
                extract_doc_words(doc, terms);
            }
        }
    }
}

fn extract_cpp_terms(content: &str, terms: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        // Class and struct declarations
        for prefix in ["class ", "struct ", "enum ", "namespace "] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let name = rest
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if name.len() >= 3 {
                    terms.push(name.to_lowercase());
                    for word in split_camel_case(name) {
                        if word.len() >= 3 {
                            terms.push(word.to_lowercase());
                        }
                    }
                }
            }
        }
        // Doxygen comments
        if let Some(doc) = trimmed
            .strip_prefix("/// ")
            .or(trimmed.strip_prefix("//! "))
            .or(trimmed.strip_prefix("* "))
        {
            if !doc.starts_with('@') && !doc.starts_with('\\') {
                extract_doc_words(doc, terms);
            }
        }
    }
}

/// Shared helper: extract meaningful words from a doc comment line.
fn extract_doc_words(doc: &str, terms: &mut Vec<String>) {
    for word in doc.split_whitespace() {
        let clean = word
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();
        if clean.len() >= 4 {
            terms.push(clean);
        }
    }
}

fn split_camel_case(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for c in s.chars() {
        if c.is_uppercase() && !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
        current.push(c);
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Extract searchable terms from a page file and its route.
fn extract_page_terms(page_file: &str, route: &str, site_dir: &Path) -> Vec<String> {
    let mut terms = Vec::new();

    // Route segments
    for segment in route.split('/') {
        if !segment.is_empty() && segment.len() >= 3 {
            terms.push(segment.to_lowercase());
            // Also split hyphens (model-switching -> model, switching)
            for part in segment.split('-') {
                if part.len() >= 3 {
                    terms.push(part.to_lowercase());
                }
            }
        }
    }

    // Read page content
    let full_path = site_dir.join(page_file);
    if let Ok(content) = std::fs::read_to_string(&full_path) {
        extract_page_content_terms(&content, &mut terms);
    }

    terms.sort();
    terms.dedup();
    terms
}

fn extract_page_content_terms(content: &str, terms: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();

        // Look for literal source path references (e.g., src/commands/bail.rs)
        for word in trimmed.split_whitespace() {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '.');
            if clean.starts_with("src/") || clean.starts_with("docs/") {
                // Extract the file stem as a term
                if let Some(stem) = Path::new(clean).file_stem().and_then(|s| s.to_str()) {
                    if stem != "mod" && stem.len() >= 3 {
                        terms.push(stem.to_lowercase());
                    }
                }
            }
        }

        // Extract text from HTML-like content: headings, dt, code
        if let Some(text) = extract_tag_text(trimmed, "h2")
            .or_else(|| extract_tag_text(trimmed, "h3"))
            .or_else(|| extract_tag_text(trimmed, "dt"))
        {
            for word in text.split_whitespace() {
                let clean = word
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase();
                if clean.len() >= 3 {
                    terms.push(clean);
                }
            }
        }

        // Inline code references: `bail`, `grant`, `trace`
        let mut rest = trimmed;
        while let Some(start) = rest.find('`') {
            rest = &rest[start + 1..];
            if let Some(end) = rest.find('`') {
                let code = &rest[..end];
                let clean = code
                    .trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                    .to_lowercase();
                if clean.len() >= 3 && !clean.contains(' ') {
                    terms.push(clean);
                }
                rest = &rest[end + 1..];
            } else {
                break;
            }
        }
    }
}

fn extract_tag_text(line: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    if let Some(start_idx) = line.find(&open) {
        let after_open = &line[start_idx + open.len()..];
        // Skip attributes until >
        if let Some(gt) = after_open.find('>') {
            let content_start = &after_open[gt + 1..];
            if let Some(end_idx) = content_start.find(&close) {
                return Some(content_start[..end_idx].to_string());
            }
        }
    }
    None
}

/// Score how well a source file matches a page.
/// Returns (score, reasons).
fn score_match(
    source_path: &str,
    source_terms: &[String],
    page_terms: &[String],
) -> (f64, Vec<String>) {
    let mut score = 0.0;
    let mut reasons = Vec::new();

    let page_term_set: HashSet<&str> = page_terms.iter().map(std::string::String::as_str).collect();

    // Tier 1: literal path reference in page content (strongest signal)
    let stem = Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if page_term_set.contains(stem) && !stem.is_empty() {
        score += 0.6;
        reasons.push(format!("page mentions \"{stem}\""));
    }

    // Tier 2: parent directory match (e.g., source in squasher/, page about compression)
    // This is captured by term overlap below.

    // Tier 3: term overlap
    if !source_terms.is_empty() {
        let matched: Vec<&String> = source_terms
            .iter()
            .filter(|t| page_term_set.contains(t.as_str()))
            .collect();
        let overlap = matched.len() as f64 / source_terms.len() as f64;
        if overlap > 0.0 {
            score += overlap * 0.4;
            if matched.len() <= 5 {
                let matched_strs: Vec<&str> = matched.iter().map(|s| s.as_str()).collect();
                reasons.push(format!("shared terms: {}", matched_strs.join(", ")));
            } else {
                reasons.push(format!("{} shared terms", matched.len()));
            }
        }
    }

    (score, reasons)
}

pub fn suggest_mappings(
    manifest: &Manifest,
    source_repo: &Path,
    site_dir: &Path,
    source_files: &[String],
    min_confidence: f64,
) -> SuggestReport {
    // Build page term index
    let mut page_terms: HashMap<String, Vec<String>> = HashMap::new();
    for page in &manifest.pages {
        if let Some(file) = &page.file {
            let terms = extract_page_terms(file, &page.route, site_dir);
            page_terms.insert(page.route.clone(), terms);
        }
    }

    // Collect already-mapped source files
    let already_mapped: HashSet<&str> = manifest
        .pages
        .iter()
        .flat_map(|p| p.sources.iter().map(|s| s.path.as_str()))
        .collect();

    let mut suggestions = Vec::new();
    let mut no_match = Vec::new();

    for source_file in source_files {
        // Skip already-mapped files
        if already_mapped.contains(source_file.as_str()) {
            continue;
        }

        let source_terms = extract_source_terms(source_file, source_repo);

        // Score against all pages
        let mut best_matches: Vec<(String, f64, Vec<String>)> = Vec::new();
        for (route, p_terms) in &page_terms {
            let (score, reasons) = score_match(source_file, &source_terms, p_terms);
            if score >= min_confidence {
                best_matches.push((route.clone(), score, reasons));
            }
        }

        best_matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        if best_matches.is_empty() {
            no_match.push(source_file.clone());
        } else {
            // Take top match (could return top N in future)
            let (route, confidence, reasons) = best_matches.remove(0);
            suggestions.push(Suggestion {
                source_path: source_file.clone(),
                route,
                confidence,
                reasons,
            });
        }
    }

    // Sort suggestions by confidence descending
    suggestions.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    SuggestReport {
        suggestions,
        no_match,
    }
}
