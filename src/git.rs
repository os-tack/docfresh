use std::path::Path;
use std::process::Command;

pub struct LogEntry {
    pub sha: String,
    pub message: String,
}

pub fn head_sha(repo_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(repo_path)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "git rev-parse failed in '{}': {}",
            repo_path.display(),
            stderr
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

#[allow(dead_code)]
pub fn head_sha_full(repo_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "git rev-parse failed in '{}': {}",
            repo_path.display(),
            stderr
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

#[allow(dead_code)]
pub fn files_changed(
    repo_path: &Path,
    since_sha: &str,
    paths: &[&str],
) -> Result<bool, Box<dyn std::error::Error>> {
    if paths.is_empty() {
        return Ok(false);
    }
    // First verify the SHA exists
    let check = Command::new("git")
        .args(["cat-file", "-t", since_sha])
        .current_dir(repo_path)
        .output()?;
    if !check.status.success() {
        return Err(format!(
            "SHA '{}' not found in repo '{}'. The page may need re-verification.",
            since_sha,
            repo_path.display()
        )
        .into());
    }

    let mut args = vec!["diff", "--quiet", since_sha, "HEAD", "--"];
    args.extend(paths);
    let status = Command::new("git")
        .args(&args)
        .current_dir(repo_path)
        .status()?;
    // exit 0 = no changes, exit 1 = changes exist
    Ok(!status.success())
}

pub fn file_log(
    repo_path: &Path,
    since_sha: &str,
    paths: &[&str],
) -> Result<Vec<LogEntry>, Box<dyn std::error::Error>> {
    if paths.is_empty() {
        return Ok(vec![]);
    }
    let range = format!("{since_sha}..HEAD");
    let mut args = vec!["log", "--oneline", &range, "--"];
    args.extend(paths);
    let output = Command::new("git")
        .args(&args)
        .current_dir(repo_path)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git log failed: {stderr}").into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    let entries = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let (sha, message) = line.split_once(' ').unwrap_or((line, ""));
            LogEntry {
                sha: sha.to_string(),
                message: message.to_string(),
            }
        })
        .collect();
    Ok(entries)
}

pub fn changed_files_between(
    repo_path: &Path,
    since_sha: &str,
    paths: &[&str],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    if paths.is_empty() {
        return Ok(vec![]);
    }
    let mut args = vec!["diff", "--name-only", since_sha, "HEAD", "--"];
    args.extend(paths);
    let output = Command::new("git")
        .args(&args)
        .current_dir(repo_path)
        .output()?;
    if !output.status.success() {
        return Ok(vec![]);
    }
    let stdout = String::from_utf8(output.stdout)?;
    Ok(stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}
