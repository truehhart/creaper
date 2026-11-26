use creaper::config::Config;
use creaper::runner::check_source;
use std::path::PathBuf;
use tempfile::TempDir;

fn config_from(toml_str: &str) -> Config {
    Config::parse(toml_str, PathBuf::from(".")).unwrap()
}

fn assert_clean(path: &str, content: &str, cfg: &Config) {
    let findings = check_source(path, content, cfg).unwrap();
    assert_eq!(findings, Vec::new(), "expected {path} to be clean");
}

#[test]
fn toggling_shebang_category() {
    let cfg = config_from("[allow]\nshebang = false\n");
    let findings = check_source("a.py", "#!/usr/bin/env python3\nx = 1\n", &cfg).unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].line, 1);
}

#[test]
fn toggling_encoding_and_script_metadata() {
    let cfg = config_from("[allow]\nencoding = false\nscript_metadata = false\n");
    let enc = check_source("a.py", "# coding: utf-8\nx = 1\n", &cfg).unwrap();
    assert_eq!(enc.len(), 1);

    let meta = check_source(
        "b.py",
        "# /// script\n# requires-python = \">=3.14\"\n# dependencies = [\"httpx\"]\n# ///\n",
        &cfg,
    )
    .unwrap();
    assert_eq!(meta.len(), 4);
}

#[test]
fn toggling_compiler_and_lint_directives() {
    let no_compiler = config_from("[allow]\ncompiler_directives = false\n");
    assert_eq!(
        check_source("a.go", "//go:build linux\npackage a\n", &no_compiler)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        check_source(
            "Dockerfile",
            "# syntax=docker/dockerfile:1\nFROM alpine\n",
            &no_compiler
        )
        .unwrap()
        .len(),
        1
    );
    assert_eq!(
        check_source("a.ts", "/// <reference types=\"node\" />\n", &no_compiler)
            .unwrap()
            .len(),
        1
    );

    let no_lint = config_from("[allow]\nlint_directives = false\n");
    assert_eq!(
        check_source("a.py", "import os  # noqa: F401\n", &no_lint)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        check_source(
            "a.go",
            "package a\nvar x = 1 //nolint:gochecknoglobals\n",
            &no_lint
        )
        .unwrap()
        .len(),
        1
    );
    assert_eq!(
        check_source("a.sh", "# shellcheck disable=SC2086\necho $x\n", &no_lint)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn doc_comments_and_docstrings_are_granular() {
    let allow_doc_comments = config_from("[allow]\ndoc_comments = true\n");
    let rs = concat!(
        "//! crate doc\n",
        "/// fn doc\n",
        "/** block doc */\n",
        "// regular comment\n",
        "pub fn f() {}\n",
    );
    let rs_findings = check_source("lib.rs", rs, &allow_doc_comments).unwrap();
    assert_eq!(rs_findings.len(), 1);
    assert_eq!(rs_findings[0].line, 4);

    let allow_docstrings = config_from("[allow]\ndocstrings = true\n");
    let py = concat!(
        "\"\"\"Module doc.\"\"\"\n",
        "X: int = 1\n",
        "\"\"\"Attr string statement.\"\"\"\n",
        "def f():\n",
        "    \"\"\"Fn doc.\"\"\"\n",
        "    \"mid body string\"\n",
        "    return 1\n",
    );
    let py_findings = check_source("a.py", py, &allow_docstrings).unwrap();
    assert_eq!(py_findings.len(), 2);
    assert_eq!(py_findings[0].line, 3);
    assert_eq!(py_findings[1].line, 6);

    let allow_string_stmts = config_from("[allow]\ndocstrings = true\nstring_statements = true\n");
    assert_clean("a.py", py, &allow_string_stmts);
}

#[test]
fn todos_and_license_headers_categories() {
    let allow_todos = config_from("[allow]\ntodos = true\n");
    let src = "// TODO(WPT-42): remove legacy branch\n// regular note\n";
    let findings = check_source("a.rs", src, &allow_todos).unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].line, 2);

    let allow_license = config_from("[allow]\nlicense_headers = true\n");
    assert_clean(
        "a.py",
        "# SPDX-License-Identifier: MIT\nx = 1\n",
        &allow_license,
    );

    let too_late = "\n\n\n\n\n\n# SPDX-License-Identifier: MIT\nx = 1\n";
    assert_eq!(
        check_source("a.py", too_late, &allow_license)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn custom_patterns_are_always_full_match() {
    let cfg = config_from(
        r##"
[languages.python.allow]
patterns = ["# allowed-tag"]
"##,
    );
    assert_clean("a.py", "# allowed-tag\nx = 1\n", &cfg);
    assert_eq!(
        check_source("a.py", "# allowed-tag with extra LLM prose\nx = 1\n", &cfg)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn path_overrides_and_shell_inheritance() {
    let cfg = config_from(
        r##"
[languages.shell.allow]
patterns = ["# contract: v1"]

[[overrides]]
files = [".pre-commit-config.yaml"]

[overrides.allow]
patterns = ['# frozen: v[0-9]+\.[0-9]+(?:\.[0-9]+)?']
"##,
    );

    let pc = "repos:\n  - repo: x\n    rev: abc  # frozen: v6.0.0\n";
    assert_clean(".pre-commit-config.yaml", pc, &cfg);
    assert_eq!(
        check_source("other.yaml", "rev: abc  # frozen: v6.0.0\n", &cfg)
            .unwrap()
            .len(),
        1
    );

    let docker = "FROM alpine\nRUN echo ok\nRUN # contract: v1\n";
    assert_clean("Dockerfile", docker, &cfg);
}

#[test]
fn exempt_generated_registry_and_disabled_language() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("generated.toml"),
        "\"gen/api.go\" = \"gen\"\n",
    )
    .unwrap();
    let cfg = Config::parse(
        r#"
generated_registry = "generated.toml"
exempt = ["vendor/", "legacy.py", "**/*.auto.rs"]

[languages.sql]
enabled = false
"#,
        tmp.path().to_path_buf(),
    )
    .unwrap();

    assert_clean("gen/api.go", "package gen\n// generated\n", &cfg);
    assert_clean("vendor/a.py", "# vendor comment\n", &cfg);
    assert_clean("legacy.py", "# legacy comment\n", &cfg);
    assert_clean("src/foo.auto.rs", "// generated rust\n", &cfg);
    assert_clean("query.sql", "-- disabled sql\nSELECT 1;\n", &cfg);
}

#[test]
fn unknown_config_keys_and_languages_are_rejected() {
    assert!(Config::parse("unknown_key = true\n", PathBuf::from(".")).is_err());
    assert!(Config::parse("[allow]\nkeep_marker = true\n", PathBuf::from(".")).is_err());
    assert!(
        Config::parse(
            "[languages.brainfuck]\nenabled = true\n",
            PathBuf::from(".")
        )
        .is_err()
    );
}
