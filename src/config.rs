use serde::Deserialize;
use std::path::Path;

const CONFIG_FILENAME: &str = ".docfresh.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub source: SourceConfig,
    pub ci: CiConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SourceConfig {
    /// Additional scan patterns beyond the language preset defaults.
    pub scan: Vec<String>,
    /// Glob patterns for source files to exclude from coverage and suggest.
    pub exclude: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct CiConfig {
    /// Maximum number of stale pages before audit fails. 0 = any stale page fails.
    pub max_stale: usize,
    /// Minimum documentation coverage percentage. 0 = disabled.
    pub min_coverage: usize,
    /// Fail if any source file is unmapped (not in a page's sources and not excluded).
    pub fail_on_unmapped: bool,
    /// Output format for the CI report: "text", "markdown", "json".
    pub format: String,
}

impl Default for CiConfig {
    fn default() -> Self {
        Self {
            max_stale: 0,
            min_coverage: 0,
            fail_on_unmapped: false,
            format: "markdown".to_string(),
        }
    }
}

impl Config {
    /// Load config from `.docfresh.toml` in the given directory, or return defaults.
    pub fn load(dir: &Path) -> Self {
        let path = dir.join(CONFIG_FILENAME);
        if !path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("warning: failed to parse {CONFIG_FILENAME}: {e}");
                    Self::default()
                }
            },
            Err(e) => {
                eprintln!("warning: failed to read {CONFIG_FILENAME}: {e}");
                Self::default()
            }
        }
    }

    /// Merge scan patterns: config extras on top of preset defaults.
    pub fn scan_patterns<'a>(&'a self, defaults: &[&'a str]) -> Vec<&'a str> {
        let mut patterns: Vec<&str> = defaults.to_vec();
        for p in &self.source.scan {
            patterns.push(p.as_str());
        }
        patterns
    }

    /// Combined exclude patterns from config and manifest.
    pub fn exclude_patterns(&self, manifest_excludes: &[String]) -> Vec<String> {
        let mut patterns = self.source.exclude.clone();
        for p in manifest_excludes {
            if !patterns.contains(p) {
                patterns.push(p.clone());
            }
        }
        patterns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = Config::default();
        assert_eq!(config.ci.max_stale, 0);
        assert_eq!(config.ci.min_coverage, 0);
        assert!(!config.ci.fail_on_unmapped);
        assert_eq!(config.ci.format, "markdown");
        assert!(config.source.scan.is_empty());
        assert!(config.source.exclude.is_empty());
    }

    #[test]
    fn parse_full_config() {
        let toml = r#"
[source]
scan = ["tests/**/*.rs", "benches/**/*.rs"]
exclude = ["src/**/mod.rs", "src/internal/**"]

[ci]
max_stale = 3
min_coverage = 25
fail_on_unmapped = true
format = "json"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.source.scan.len(), 2);
        assert_eq!(config.source.exclude.len(), 2);
        assert_eq!(config.ci.max_stale, 3);
        assert_eq!(config.ci.min_coverage, 25);
        assert!(config.ci.fail_on_unmapped);
        assert_eq!(config.ci.format, "json");
    }

    #[test]
    fn parse_partial_config() {
        let toml = r#"
[ci]
fail_on_unmapped = true
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert!(config.ci.fail_on_unmapped);
        // Defaults preserved
        assert_eq!(config.ci.max_stale, 0);
        assert!(config.source.scan.is_empty());
    }

    #[test]
    fn merge_scan_patterns() {
        let config: Config = toml::from_str(
            r#"
[source]
scan = ["tests/**/*.rs"]
"#,
        )
        .unwrap();
        let defaults = vec!["src/**/*.rs", "docs/**/*.md"];
        let merged = config.scan_patterns(&defaults);
        assert_eq!(merged, vec!["src/**/*.rs", "docs/**/*.md", "tests/**/*.rs"]);
    }

    #[test]
    fn merge_exclude_patterns() {
        let config: Config = toml::from_str(
            r#"
[source]
exclude = ["src/**/mod.rs"]
"#,
        )
        .unwrap();
        let manifest_excludes = vec!["docs/archive/*".to_string(), "src/**/mod.rs".to_string()];
        let merged = config.exclude_patterns(&manifest_excludes);
        // Deduplicates
        assert_eq!(merged.len(), 2);
        assert!(merged.contains(&"src/**/mod.rs".to_string()));
        assert!(merged.contains(&"docs/archive/*".to_string()));
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load(dir.path());
        assert_eq!(config.ci.max_stale, 0);
    }

    #[test]
    fn load_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".docfresh.toml"), "[ci]\nmax_stale = 5\n").unwrap();
        let config = Config::load(dir.path());
        assert_eq!(config.ci.max_stale, 5);
    }

    #[test]
    fn load_invalid_toml_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".docfresh.toml"), "not valid {{toml").unwrap();
        let config = Config::load(dir.path());
        assert_eq!(config.ci.max_stale, 0);
    }
}
