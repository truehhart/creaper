use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_creaper")
}

#[test]
fn cli_exits_zero_one_and_two() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("clean.py"), "x = 1\n").unwrap();

    let clean = Command::new(bin()).arg(tmp.path()).output().unwrap();
    assert_eq!(clean.status.code(), Some(0));

    fs::write(tmp.path().join("dirty.py"), "x = 1  # why\n").unwrap();
    let dirty = Command::new(bin()).arg(tmp.path()).output().unwrap();
    assert_eq!(dirty.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&dirty.stderr);
    assert!(stderr.contains("dirty.py:1:8: [python] # why"));

    let json = Command::new(bin())
        .args(["--format", "json"])
        .arg(tmp.path())
        .output()
        .unwrap();
    assert_eq!(json.status.code(), Some(1));
    let parsed: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(parsed["findings"][0]["path"], "dirty.py");
    assert_eq!(parsed["findings"][0]["language"], "python");
    assert_eq!(parsed["findings"][0]["line"], 1);

    fs::write(tmp.path().join("creaper.toml"), "bad_key = 1\n").unwrap();
    let bad_cfg = Command::new(bin()).arg(tmp.path()).output().unwrap();
    assert_eq!(bad_cfg.status.code(), Some(2));

    let no_cfg = Command::new(bin())
        .arg("--no-config")
        .arg(tmp.path().join("clean.py"))
        .output()
        .unwrap();
    assert_eq!(no_cfg.status.code(), Some(0));
}

#[test]
fn cli_discovers_config_and_rejects_ambiguous_configs() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("creaper.toml"),
        "[allow]\ndocstrings = true\n",
    )
    .unwrap();
    fs::write(tmp.path().join("a.py"), "\"\"\"Ok.\"\"\"\n").unwrap();

    let ok = Command::new(bin()).arg(tmp.path()).output().unwrap();
    assert_eq!(ok.status.code(), Some(0));

    fs::write(
        tmp.path().join(".creaper.toml"),
        "[allow]\ndocstrings = false\n",
    )
    .unwrap();
    let ambiguous = Command::new(bin()).arg(tmp.path()).output().unwrap();
    assert_eq!(ambiguous.status.code(), Some(2));
}

#[test]
fn cli_list_flags_and_tracked_mode() {
    let langs = Command::new(bin())
        .args(["--list", "languages"])
        .output()
        .unwrap();
    assert_eq!(langs.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&langs.stdout);
    assert!(stdout.contains("python"));
    assert!(stdout.contains("nushell"));
    assert!(stdout.contains("hcl"));

    let cats = Command::new(bin())
        .args(["--list", "categories"])
        .output()
        .unwrap();
    assert_eq!(cats.status.code(), Some(0));
    let cat_out = String::from_utf8_lossy(&cats.stdout);
    assert!(cat_out.contains("lint_directives"));
    assert!(cat_out.contains("docstrings"));

    let tmp = TempDir::new().unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(tmp.path())
            .status()
            .unwrap()
            .success()
    );
    fs::write(tmp.path().join("tracked.py"), "x = 1\n").unwrap();
    fs::write(tmp.path().join("untracked.py"), "# untracked comment\n").unwrap();
    assert!(
        Command::new("git")
            .args(["add", "tracked.py"])
            .current_dir(tmp.path())
            .status()
            .unwrap()
            .success()
    );

    let tracked = Command::new(bin())
        .arg("--tracked")
        .arg(tmp.path())
        .output()
        .unwrap();
    assert_eq!(tracked.status.code(), Some(0));

    let walked = Command::new(bin()).arg(tmp.path()).output().unwrap();
    assert_eq!(walked.status.code(), Some(1));
}

#[test]
fn cli_external_config_relativizes_exempt_paths() {
    let cfg_dir = TempDir::new().unwrap();
    let repo_dir = TempDir::new().unwrap();
    let cfg_path = cfg_dir.path().join("external.toml");
    fs::write(&cfg_path, "exempt = [\"ignored/\"]\n").unwrap();
    fs::create_dir_all(repo_dir.path().join("ignored")).unwrap();
    fs::write(repo_dir.path().join("ignored/a.py"), "# allowed here\n").unwrap();
    fs::write(repo_dir.path().join("clean.py"), "x = 1\n").unwrap();

    let out = Command::new(bin())
        .arg("--config")
        .arg(&cfg_path)
        .arg(repo_dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
}
