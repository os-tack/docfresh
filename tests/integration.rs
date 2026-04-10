use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn setup_fixture() -> (TempDir, TempDir) {
    let source_dir = TempDir::new().unwrap();
    let site_dir = TempDir::new().unwrap();

    // Initialize source repo
    git(source_dir.path(), &["init"]);
    git(
        source_dir.path(),
        &["config", "user.email", "test@test.com"],
    );
    git(source_dir.path(), &["config", "user.name", "Test"]);

    // Create source files
    fs::create_dir_all(source_dir.path().join("src/commands")).unwrap();
    fs::create_dir_all(source_dir.path().join("docs/spec")).unwrap();
    fs::write(
        source_dir.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        source_dir.path().join("src/commands/auth.rs"),
        "pub fn authenticate(token: &str) -> bool {\n    true\n}\n",
    )
    .unwrap();
    fs::write(
        source_dir.path().join("src/commands/bail.rs"),
        "pub fn pack() {}\npub fn unpack() {}\n",
    )
    .unwrap();
    fs::write(
        source_dir.path().join("docs/spec/auth.md"),
        "# Authentication\n\nHow auth works.\n",
    )
    .unwrap();

    git(source_dir.path(), &["add", "-A"]);
    git(source_dir.path(), &["commit", "-m", "initial source"]);

    // Create site files
    fs::create_dir_all(site_dir.path().join("docs")).unwrap();
    fs::write(
        site_dir.path().join("docs/auth.md"),
        "---\ntitle: Authentication\n---\n\n# Auth\n\nSee `src/commands/auth.rs`.\n",
    )
    .unwrap();
    fs::write(
        site_dir.path().join("docs/getting-started.md"),
        "---\ntitle: Getting Started\n---\n\n# Getting Started\n",
    )
    .unwrap();

    (source_dir, site_dir)
}

fn git(dir: &std::path::Path, args: &[&str]) {
    std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git command failed");
}

fn docfresh() -> Command {
    Command::cargo_bin("docfresh").unwrap()
}

fn manifest_path(site: &TempDir) -> String {
    site.path()
        .join("site-manifest.json")
        .to_str()
        .unwrap()
        .to_string()
}

#[test]
fn init_creates_manifest_with_pages() {
    let (source_dir, site_dir) = setup_fixture();
    let mp = manifest_path(&site_dir);

    docfresh()
        .current_dir(site_dir.path())
        .args([
            "-m",
            &mp,
            "init",
            "--source-repo",
            source_dir.path().to_str().unwrap(),
            "--site",
            "markdown",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 2 pages"));

    let content = fs::read_to_string(&mp).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(manifest["version"], 1);
    let pages = manifest["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 2);

    // Check title extraction from YAML front matter
    let auth_page = pages.iter().find(|p| p["route"] == "/auth").unwrap();
    assert_eq!(auth_page["title"], "Authentication");
}

#[test]
fn verify_all_marks_current() {
    let (source_dir, site_dir) = setup_fixture();
    let mp = manifest_path(&site_dir);

    docfresh()
        .current_dir(site_dir.path())
        .args([
            "-m",
            &mp,
            "init",
            "--source-repo",
            source_dir.path().to_str().unwrap(),
            "--site",
            "markdown",
        ])
        .assert()
        .success();

    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "verify", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Verified 2 pages"));

    let content = fs::read_to_string(&mp).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&content).unwrap();
    for page in manifest["pages"].as_array().unwrap() {
        assert_eq!(page["status"], "current");
        assert!(page["verified_at"]["sha"].is_string());
    }
}

#[test]
fn audit_clean_after_verify() {
    let (source_dir, site_dir) = setup_fixture();
    let mp = manifest_path(&site_dir);

    docfresh()
        .current_dir(site_dir.path())
        .args([
            "-m",
            &mp,
            "init",
            "--source-repo",
            source_dir.path().to_str().unwrap(),
            "--site",
            "markdown",
        ])
        .assert()
        .success();
    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "verify", "--all"])
        .assert()
        .success();

    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "audit"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 current, 0 stale"));
}

#[test]
fn audit_detects_stale_page() {
    let (source_dir, site_dir) = setup_fixture();
    let mp = manifest_path(&site_dir);

    // Init, map, verify
    docfresh()
        .current_dir(site_dir.path())
        .args([
            "-m",
            &mp,
            "init",
            "--source-repo",
            source_dir.path().to_str().unwrap(),
            "--site",
            "markdown",
        ])
        .assert()
        .success();
    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "map", "/auth", "src/commands/auth.rs"])
        .assert()
        .success();
    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "verify", "--all"])
        .assert()
        .success();

    // Modify source and commit
    fs::write(
        source_dir.path().join("src/commands/auth.rs"),
        "pub fn authenticate(token: &str) -> bool { token.len() > 0 }\npub fn refresh() {}\n",
    )
    .unwrap();
    git(source_dir.path(), &["add", "-A"]);
    git(source_dir.path(), &["commit", "-m", "add refresh"]);

    // Audit detects staleness
    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "audit"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("STALE"))
        .stdout(predicate::str::contains("/auth"));
}

#[test]
fn audit_json_output() {
    let (source_dir, site_dir) = setup_fixture();
    let mp = manifest_path(&site_dir);

    docfresh()
        .current_dir(site_dir.path())
        .args([
            "-m",
            &mp,
            "init",
            "--source-repo",
            source_dir.path().to_str().unwrap(),
            "--site",
            "markdown",
        ])
        .assert()
        .success();
    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "verify", "--all"])
        .assert()
        .success();

    let output = docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "audit", "--json"])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed["results"].is_array());
}

#[test]
fn map_adds_source_and_is_idempotent() {
    let (source_dir, site_dir) = setup_fixture();
    let mp = manifest_path(&site_dir);

    docfresh()
        .current_dir(site_dir.path())
        .args([
            "-m",
            &mp,
            "init",
            "--source-repo",
            source_dir.path().to_str().unwrap(),
            "--site",
            "markdown",
        ])
        .assert()
        .success();

    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "map", "/auth", "src/commands/auth.rs"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Mapped"));

    // Second map — idempotent
    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "map", "/auth", "src/commands/auth.rs"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already mapped"));

    // Verify source was added
    let content = fs::read_to_string(&mp).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&content).unwrap();
    let auth = manifest["pages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["route"] == "/auth")
        .unwrap();
    assert_eq!(auth["sources"].as_array().unwrap().len(), 1);
}

#[test]
fn status_table_output() {
    let (source_dir, site_dir) = setup_fixture();
    let mp = manifest_path(&site_dir);

    docfresh()
        .current_dir(site_dir.path())
        .args([
            "-m",
            &mp,
            "init",
            "--source-repo",
            source_dir.path().to_str().unwrap(),
            "--site",
            "markdown",
        ])
        .assert()
        .success();

    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ROUTE"))
        .stdout(predicate::str::contains("/auth"))
        .stdout(predicate::str::contains("unverified"));
}

#[test]
fn status_json_output() {
    let (source_dir, site_dir) = setup_fixture();
    let mp = manifest_path(&site_dir);

    docfresh()
        .current_dir(site_dir.path())
        .args([
            "-m",
            &mp,
            "init",
            "--source-repo",
            source_dir.path().to_str().unwrap(),
            "--site",
            "markdown",
        ])
        .assert()
        .success();

    let output = docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "status", "--json"])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 2);
}

#[test]
fn diff_shows_changes() {
    let (source_dir, site_dir) = setup_fixture();
    let mp = manifest_path(&site_dir);

    docfresh()
        .current_dir(site_dir.path())
        .args([
            "-m",
            &mp,
            "init",
            "--source-repo",
            source_dir.path().to_str().unwrap(),
            "--site",
            "markdown",
        ])
        .assert()
        .success();
    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "map", "/auth", "src/commands/auth.rs"])
        .assert()
        .success();
    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "verify", "--all"])
        .assert()
        .success();

    // Modify and commit
    fs::write(
        source_dir.path().join("src/commands/auth.rs"),
        "pub fn authenticate() -> bool { false }\n",
    )
    .unwrap();
    git(source_dir.path(), &["add", "-A"]);
    git(source_dir.path(), &["commit", "-m", "break auth"]);

    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "diff", "/auth"])
        .assert()
        .success()
        .stdout(predicate::str::contains("src/commands/auth.rs"))
        .stdout(predicate::str::contains("break auth"));
}

#[test]
fn list_presets_shows_frameworks() {
    docfresh()
        .args(["init", "--list-presets"])
        .assert()
        .success()
        .stdout(predicate::str::contains("astro"))
        .stdout(predicate::str::contains("hugo"))
        .stdout(predicate::str::contains("nextjs-app"))
        .stdout(predicate::str::contains("docusaurus"))
        .stdout(predicate::str::contains("rust"))
        .stdout(predicate::str::contains("python"))
        .stdout(predicate::str::contains("typescript"));
}

#[test]
fn coverage_shows_undocumented() {
    let (source_dir, site_dir) = setup_fixture();
    let mp = manifest_path(&site_dir);

    docfresh()
        .current_dir(site_dir.path())
        .args([
            "-m",
            &mp,
            "init",
            "--source-repo",
            source_dir.path().to_str().unwrap(),
            "--site",
            "markdown",
        ])
        .assert()
        .success();

    // Map only auth, leave bail unmapped
    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "map", "/auth", "src/commands/auth.rs"])
        .assert()
        .success();

    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "coverage"])
        .assert()
        .success()
        .stdout(predicate::str::contains("UNDOCUMENTED"))
        .stdout(predicate::str::contains("src/commands/bail.rs"));
}

#[test]
fn verify_single_page() {
    let (source_dir, site_dir) = setup_fixture();
    let mp = manifest_path(&site_dir);

    docfresh()
        .current_dir(site_dir.path())
        .args([
            "-m",
            &mp,
            "init",
            "--source-repo",
            source_dir.path().to_str().unwrap(),
            "--site",
            "markdown",
        ])
        .assert()
        .success();

    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "verify", "/auth"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Verified /auth"));

    // Only auth should be current, getting-started still unverified
    let content = fs::read_to_string(&mp).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&content).unwrap();
    let pages = manifest["pages"].as_array().unwrap();
    let auth = pages.iter().find(|p| p["route"] == "/auth").unwrap();
    let gs = pages
        .iter()
        .find(|p| p["route"] == "/getting-started")
        .unwrap();
    assert_eq!(auth["status"], "current");
    assert_eq!(gs["status"], "unverified");
}

#[test]
fn init_rejects_existing_manifest() {
    let (_source_dir, site_dir) = setup_fixture();
    let mp = manifest_path(&site_dir);
    fs::write(&mp, "{}").unwrap();

    docfresh()
        .current_dir(site_dir.path())
        .args(["-m", &mp, "init"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("already exists"));
}
