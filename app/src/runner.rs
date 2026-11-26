use crate::config::{Config, normalize_rel_path};
use crate::language::{self, Language};
use crate::policy::{AllowCategory, CommentKind, EffectivePolicy, python_script_metadata_lines};
use crate::scanner;
use ignore::WalkBuilder;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub language: Language,
    pub kind: CommentKind,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunReport {
    pub findings: Vec<Finding>,
    pub errors: Vec<String>,
    pub files_checked: usize,
}

#[derive(Debug)]
struct FileResult {
    checked: bool,
    findings: Vec<Finding>,
    error: Option<String>,
}

pub fn run(paths: &[PathBuf], config: &Config, tracked_only: bool) -> Result<RunReport, String> {
    let targets = discover_files(paths, config, tracked_only)?;

    let results: Vec<FileResult> = targets
        .par_iter()
        .map(|(abs_path, rel_path)| check_file(abs_path, rel_path, config))
        .collect();

    let mut findings = Vec::new();
    let mut errors = Vec::new();
    let mut files_checked = 0usize;

    for res in results {
        if res.checked {
            files_checked += 1;
        }
        findings.extend(res.findings);
        if let Some(err) = res.error {
            errors.push(err);
        }
    }

    findings.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.column.cmp(&b.column))
    });
    errors.sort();

    Ok(RunReport {
        findings,
        errors,
        files_checked,
    })
}

pub fn check_source(
    rel_path: &str,
    content: &str,
    config: &Config,
) -> Result<Vec<Finding>, String> {
    if config.is_exempt(rel_path) {
        return Ok(Vec::new());
    }

    let Some(language) = language::detect(Path::new(rel_path), content)
        .map_err(|err| format!("{rel_path}: {err}"))?
    else {
        return Ok(Vec::new());
    };

    if !config.is_language_enabled(language) {
        return Ok(Vec::new());
    }

    let policy = EffectivePolicy::resolve(config, language, rel_path)?;
    let comments = scanner::scan(language, content, config.fail_on_parse_error)
        .map_err(|err| format!("{rel_path}: {err}"))?;

    let first_line = content
        .split('\n')
        .next()
        .unwrap_or("")
        .trim_end_matches('\r');
    let metadata_lines =
        if language == Language::Python && policy.has(AllowCategory::ScriptMetadata) {
            python_script_metadata_lines(content, scanner::toml_has_comments)
        } else {
            HashSet::new()
        };

    let mut findings = Vec::new();
    for comment in comments {
        if !policy.is_allowed(language, &comment, first_line, &metadata_lines) {
            findings.push(Finding {
                path: normalize_rel_path(rel_path),
                line: comment.line,
                column: comment.column,
                language,
                kind: comment.kind,
                text: comment.text,
            });
        }
    }

    Ok(findings)
}

fn check_file(abs_path: &Path, rel_path: &str, config: &Config) -> FileResult {
    if config.is_exempt(rel_path) {
        return FileResult {
            checked: false,
            findings: Vec::new(),
            error: None,
        };
    }

    let Ok(meta) = fs::symlink_metadata(abs_path) else {
        return FileResult {
            checked: false,
            findings: Vec::new(),
            error: Some(format!("{rel_path}: cannot stat file")),
        };
    };
    if meta.file_type().is_symlink() || !meta.is_file() {
        return FileResult {
            checked: false,
            findings: Vec::new(),
            error: None,
        };
    }

    let bytes = match fs::read(abs_path) {
        Ok(b) => b,
        Err(err) => {
            return FileResult {
                checked: false,
                findings: Vec::new(),
                error: Some(format!("{rel_path}: {err}")),
            };
        }
    };

    if bytes.contains(&0) {
        return FileResult {
            checked: false,
            findings: Vec::new(),
            error: None,
        };
    }

    let content = String::from_utf8_lossy(&bytes);
    let detected = match language::detect(Path::new(rel_path), &content) {
        Ok(lang) => lang,
        Err(err) => {
            return FileResult {
                checked: false,
                findings: Vec::new(),
                error: Some(format!("{rel_path}: {err}")),
            };
        }
    };

    let Some(lang) = detected else {
        return FileResult {
            checked: false,
            findings: Vec::new(),
            error: None,
        };
    };

    if !config.is_language_enabled(lang) {
        return FileResult {
            checked: false,
            findings: Vec::new(),
            error: None,
        };
    }

    match check_source(rel_path, &content, config) {
        Ok(findings) => FileResult {
            checked: true,
            findings,
            error: None,
        },
        Err(err) => FileResult {
            checked: true,
            findings: Vec::new(),
            error: Some(err),
        },
    }
}

fn discover_files(
    paths: &[PathBuf],
    config: &Config,
    tracked_only: bool,
) -> Result<Vec<(PathBuf, String)>, String> {
    let inputs: Vec<PathBuf> = if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths.to_vec()
    };

    let root = fs::canonicalize(&config.root).unwrap_or_else(|_| config.root.clone());
    let mut seen = HashSet::new();
    let mut files = Vec::new();

    for input in &inputs {
        let meta = fs::symlink_metadata(input)
            .map_err(|err| format!("cannot access {}: {err}", input.display()))?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_file() {
            let abs = fs::canonicalize(input).unwrap_or_else(|_| input.clone());
            if seen.insert(abs.clone()) {
                let rel = relative_display(&abs, &root, input);
                files.push((abs, rel));
            }
        } else if meta.is_dir() {
            let dir_files = if tracked_only {
                discover_git_tracked(input)?
            } else {
                discover_walk(input)?
            };
            for file in dir_files {
                let abs = fs::canonicalize(&file).unwrap_or(file.clone());
                if seen.insert(abs.clone()) {
                    let rel = relative_display(&abs, &root, &file);
                    files.push((abs, rel));
                }
            }
        }
    }

    Ok(files)
}

fn discover_git_tracked(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(dir)
        .output()
        .map_err(|err| format!("failed to run git ls-files in {}: {err}", dir.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "git ls-files failed in {}: {}",
            dir.display(),
            stderr.trim()
        ));
    }

    let mut files = Vec::new();
    for raw in output.stdout.split(|&b| b == 0) {
        if raw.is_empty() {
            continue;
        }
        let path_str = String::from_utf8_lossy(raw);
        let full = dir.join(path_str.as_ref());
        if let Ok(meta) = fs::symlink_metadata(&full)
            && !meta.file_type().is_symlink()
            && meta.is_file()
        {
            files.push(full);
        }
    }
    Ok(files)
}

fn discover_walk(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let walker = WalkBuilder::new(dir)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .follow_links(false)
        .filter_entry(|entry| {
            let is_git_dir =
                entry.file_name() == ".git" && entry.file_type().is_some_and(|ft| ft.is_dir());
            !is_git_dir
        })
        .build();

    let mut files = Vec::new();
    for entry in walker {
        let entry = entry.map_err(|err| format!("walk error: {err}"))?;
        if entry.file_type().is_some_and(|ft| ft.is_file()) {
            files.push(entry.into_path());
        }
    }
    Ok(files)
}

fn relative_display(abs: &Path, root: &Path, fallback: &Path) -> String {
    if let Ok(rel) = abs.strip_prefix(root) {
        return normalize_rel_path(&rel.to_string_lossy());
    }
    if let Some(rel) = diff_paths(abs, root) {
        return normalize_rel_path(&rel.to_string_lossy());
    }
    normalize_rel_path(&fallback.to_string_lossy())
}

fn diff_paths(path: &Path, base: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let path_comps: Vec<Component<'_>> = path.components().collect();
    let base_comps: Vec<Component<'_>> = base.components().collect();
    if path_comps.first() != base_comps.first() {
        return None;
    }
    let common = path_comps
        .iter()
        .zip(base_comps.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut out = PathBuf::new();
    for _ in common..base_comps.len() {
        out.push("..");
    }
    for comp in &path_comps[common..] {
        out.push(comp.as_os_str());
    }
    Some(out)
}
