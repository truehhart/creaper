use clap::Parser;
use creaper::cli::{Cli, ListKind, OutputFormat};
use creaper::config::Config;
use creaper::language::Language;
use creaper::policy::CATEGORIES;
use creaper::runner;
use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const WHY: &str = "no comment or docstring justifies a poor implementation or a workaround: code stands on its names and structure, and code that needs an excuse needs a better design";

fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Some(kind) = cli.list {
        match kind {
            ListKind::Languages => print_languages(),
            ListKind::Categories => print_categories(),
        }
        return ExitCode::SUCCESS;
    }

    let config = match resolve_config(&cli) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("creaper: {err}");
            return ExitCode::from(2);
        }
    };

    let report = match runner::run(&cli.paths, &config, cli.tracked) {
        Ok(rep) => rep,
        Err(err) => {
            eprintln!("creaper: {err}");
            return ExitCode::from(2);
        }
    };

    match cli.format {
        OutputFormat::Json => match serde_json::to_string_pretty(&report) {
            Ok(json) => println!("{json}"),
            Err(err) => {
                eprintln!("creaper: failed to serialize json: {err}");
                return ExitCode::from(2);
            }
        },
        OutputFormat::Text => {
            for err in &report.errors {
                eprintln!("creaper: error: {err}");
            }
            for finding in &report.findings {
                let preview = truncate_preview(&finding.text, 120);
                eprintln!(
                    "{}:{}:{}: [{}] {preview}",
                    finding.path,
                    finding.line,
                    finding.column,
                    finding.language.as_str()
                );
            }
            if !report.findings.is_empty() {
                eprintln!("creaper: {WHY}");
            }
        }
    }

    if !report.errors.is_empty() {
        ExitCode::from(2)
    } else if !report.findings.is_empty() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn resolve_config(cli: &Cli) -> Result<Config, String> {
    if cli.no_config {
        let root = discovery_start(&cli.paths);
        return Ok(Config {
            root,
            ..Config::default()
        });
    }
    let target_root = discovery_start(&cli.paths);
    if let Some(ref path) = cli.config {
        let cfg_parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let canon_parent = std::fs::canonicalize(cfg_parent).ok();
        let canon_target = std::fs::canonicalize(&target_root).ok();
        let root = match (canon_parent, canon_target) {
            (Some(p), Some(t)) if t.starts_with(&p) => p,
            _ => target_root,
        };
        return Config::load_with_root(path, root);
    }
    Config::discover(&target_root)
}

fn discovery_start(paths: &[PathBuf]) -> PathBuf {
    if paths.len() == 1 {
        let single = &paths[0];
        if single.is_dir() {
            return single.clone();
        }
        if let Some(parent) = single.parent()
            && !parent.as_os_str().is_empty()
        {
            return parent.to_path_buf();
        }
    }
    env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf())
}

fn truncate_preview(text: &str, max_chars: usize) -> String {
    let first_line = text.lines().next().unwrap_or("");
    let char_count = first_line.chars().count();
    if char_count <= max_chars && !text.contains('\n') {
        return first_line.to_string();
    }
    let mut out: String = first_line.chars().take(max_chars).collect();
    if char_count > max_chars || text.contains('\n') {
        out.push_str("...");
    }
    out
}

fn print_languages() {
    for lang in Language::ALL {
        let mut patterns: Vec<String> =
            lang.extensions().iter().map(|e| format!("*.{e}")).collect();
        patterns.extend(lang.filenames().iter().map(|f| (*f).to_string()));
        println!("{:<16} {}", lang.as_str(), patterns.join(", "));
    }
}

fn print_categories() {
    for cat in CATEGORIES {
        let state = if cat.default_enabled { "on " } else { "off" };
        println!("{:<22} [default: {state}]  {}", cat.name, cat.description);
    }
}
