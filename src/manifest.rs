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
