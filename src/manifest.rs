use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub source_repo: SourceRepo,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_patterns: Vec<String>,
    pub pages: Vec<Page>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceRepo {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    #[serde(default = "default_branch")]
    pub default_branch: String,
}

fn default_branch() -> String {
    "main".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Page {
    pub route: String,
    pub file: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub related: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<VerifiedAt>,
    #[serde(default)]
    pub status: Status,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concepts: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Source {
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifiedAt {
    pub sha: String,
    pub timestamp: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Current,
    Stale,
    Outdated,
    #[default]
    Unverified,
    Missing,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Current => write!(f, "current"),
            Status::Stale => write!(f, "stale"),
            Status::Outdated => write!(f, "outdated"),
            Status::Unverified => write!(f, "unverified"),
            Status::Missing => write!(f, "missing"),
        }
    }
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let manifest: Manifest = serde_json::from_str(&content)?;
        if manifest.version != 1 {
            return Err(format!("unsupported manifest version: {}", manifest.version).into());
        }
        Ok(manifest)
    }

    pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content + "\n")?;
        Ok(())
    }

    pub fn resolve_source_repo(
        &self,
        manifest_dir: &Path,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let repo_path = manifest_dir.join(&self.source_repo.path);
        let repo_path = repo_path.canonicalize().map_err(|_| {
            format!(
                "source repo not found at '{}' (resolved from manifest dir '{}'). \
                 Clone it or update source_repo.path in the manifest.",
                self.source_repo.path,
                manifest_dir.display()
            )
        })?;
        let git_dir = repo_path.join(".git");
        if !git_dir.exists() {
            return Err(format!(
                "'{}' exists but is not a git repository (no .git directory)",
                repo_path.display()
            )
            .into());
        }
        Ok(repo_path)
    }

    pub fn find_page(&self, route: &str) -> Option<usize> {
        self.pages.iter().position(|p| p.route == route)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> Manifest {
        Manifest {
            version: 1,
            source_repo: SourceRepo {
                path: "../source".to_string(),
                remote: None,
                default_branch: "main".to_string(),
            },
            exclude_patterns: vec![],
            pages: vec![
                Page {
                    route: "/docs/auth".to_string(),
                    file: Some("src/pages/docs/auth.astro".to_string()),
                    title: "Authentication".to_string(),
                    tags: vec!["reference".to_string()],
                    sources: vec![Source {
                        path: "src/auth.rs".to_string(),
                        sections: vec![],
                    }],
                    related: vec!["/docs/permissions".to_string()],
                    verified_at: Some(VerifiedAt {
                        sha: "abc1234".to_string(),
                        timestamp: "2025-01-01T00:00:00Z".to_string(),
                    }),
                    status: Status::Current,
                    concepts: vec![],
                },
                Page {
                    route: "/docs/api".to_string(),
                    file: None,
                    title: "API Reference".to_string(),
                    tags: vec![],
                    sources: vec![],
                    related: vec![],
                    verified_at: None,
                    status: Status::Missing,
                    concepts: vec![],
                },
            ],
        }
    }

    #[test]
    fn serde_roundtrip() {
        let manifest = sample_manifest();
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let parsed: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.pages.len(), 2);
        assert_eq!(parsed.pages[0].route, "/docs/auth");
        assert_eq!(parsed.pages[0].status, Status::Current);
        assert_eq!(parsed.pages[1].status, Status::Missing);
    }

    #[test]
    fn status_display() {
        assert_eq!(Status::Current.to_string(), "current");
        assert_eq!(Status::Stale.to_string(), "stale");
        assert_eq!(Status::Outdated.to_string(), "outdated");
        assert_eq!(Status::Unverified.to_string(), "unverified");
        assert_eq!(Status::Missing.to_string(), "missing");
    }

    #[test]
    fn status_default_is_unverified() {
        assert_eq!(Status::default(), Status::Unverified);
    }

    #[test]
    fn status_serde_snake_case() {
        let json = serde_json::to_string(&Status::Current).unwrap();
        assert_eq!(json, "\"current\"");
        let parsed: Status = serde_json::from_str("\"stale\"").unwrap();
        assert_eq!(parsed, Status::Stale);
    }

    #[test]
    fn find_page_existing() {
        let manifest = sample_manifest();
        assert_eq!(manifest.find_page("/docs/auth"), Some(0));
        assert_eq!(manifest.find_page("/docs/api"), Some(1));
    }

    #[test]
    fn find_page_missing() {
        let manifest = sample_manifest();
        assert_eq!(manifest.find_page("/nonexistent"), None);
    }

    #[test]
    fn load_rejects_bad_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        std::fs::write(
            &path,
            r#"{"version": 99, "source_repo": {"path": "."}, "pages": []}"#,
        )
        .unwrap();
        let result = Manifest::load(&path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("unsupported manifest version"));
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        let manifest = sample_manifest();
        manifest.save(&path).unwrap();
        let loaded = Manifest::load(&path).unwrap();
        assert_eq!(loaded.pages.len(), 2);
        assert_eq!(loaded.pages[0].title, "Authentication");
        assert_eq!(loaded.source_repo.default_branch, "main");
    }

    #[test]
    fn default_branch_defaults_to_main() {
        let json = r#"{"version": 1, "source_repo": {"path": "."}, "pages": []}"#;
        let manifest: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.source_repo.default_branch, "main");
    }

    #[test]
    fn exclude_patterns_omitted_when_empty() {
        let manifest = sample_manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        assert!(!json.contains("exclude_patterns"));
    }

    #[test]
    fn sections_omitted_when_empty() {
        let source = Source {
            path: "src/lib.rs".to_string(),
            sections: vec![],
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(!json.contains("sections"));
    }
}
