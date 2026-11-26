use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Language {
    Python,
    Go,
    Shell,
    Nushell,
    Yaml,
    Toml,
    Hcl,
    Php,
    Sql,
    Dockerfile,
    Make,
    ShellTemplate,
    Ini,
    IniLines,
    Env,
    Ignore,
    Jinja2,
    Rust,
    Javascript,
    Typescript,
    Tsx,
}

impl Language {
    pub const ALL: &'static [Self] = &[
        Self::Python,
        Self::Go,
        Self::Shell,
        Self::Nushell,
        Self::Yaml,
        Self::Toml,
        Self::Hcl,
        Self::Php,
        Self::Sql,
        Self::Dockerfile,
        Self::Make,
        Self::ShellTemplate,
        Self::Ini,
        Self::IniLines,
        Self::Env,
        Self::Ignore,
        Self::Jinja2,
        Self::Rust,
        Self::Javascript,
        Self::Typescript,
        Self::Tsx,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Go => "go",
            Self::Shell => "shell",
            Self::Nushell => "nushell",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Hcl => "hcl",
            Self::Php => "php",
            Self::Sql => "sql",
            Self::Dockerfile => "dockerfile",
            Self::Make => "make",
            Self::ShellTemplate => "shell-template",
            Self::Ini => "ini",
            Self::IniLines => "ini-lines",
            Self::Env => "env",
            Self::Ignore => "ignore",
            Self::Jinja2 => "jinja2",
            Self::Rust => "rust",
            Self::Javascript => "javascript",
            Self::Typescript => "typescript",
            Self::Tsx => "tsx",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "python" | "py" => Some(Self::Python),
            "go" | "golang" => Some(Self::Go),
            "shell" | "sh" | "bash" => Some(Self::Shell),
            "nushell" | "nu" => Some(Self::Nushell),
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "hcl" | "terraform" | "tf" => Some(Self::Hcl),
            "php" => Some(Self::Php),
            "sql" => Some(Self::Sql),
            "dockerfile" | "docker" => Some(Self::Dockerfile),
            "make" | "makefile" => Some(Self::Make),
            "shell-template" => Some(Self::ShellTemplate),
            "ini" => Some(Self::Ini),
            "ini-lines" => Some(Self::IniLines),
            "env" => Some(Self::Env),
            "ignore" | "gitignore" => Some(Self::Ignore),
            "jinja2" | "jinja" | "j2" => Some(Self::Jinja2),
            "rust" | "rs" => Some(Self::Rust),
            "javascript" | "js" => Some(Self::Javascript),
            "typescript" | "ts" => Some(Self::Typescript),
            "tsx" => Some(Self::Tsx),
            _ => None,
        }
    }

    pub const fn supports_shebang(self) -> bool {
        matches!(
            self,
            Self::Python
                | Self::Shell
                | Self::ShellTemplate
                | Self::Nushell
                | Self::Php
                | Self::Javascript
                | Self::Typescript
        )
    }

    pub const fn inherits_shell(self) -> bool {
        matches!(self, Self::Dockerfile | Self::Make | Self::ShellTemplate)
    }

    pub const fn parent_config_language(self) -> Option<Self> {
        match self {
            Self::IniLines => Some(Self::Ini),
            Self::Tsx => Some(Self::Typescript),
            _ => None,
        }
    }

    pub const fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Python => &["py", "pyi"],
            Self::Go => &["go"],
            Self::Shell => &["sh", "bash", "bats", "ksh", "zsh"],
            Self::Nushell => &["nu"],
            Self::Yaml => &["yaml", "yml"],
            Self::Toml => &["toml"],
            Self::Hcl => &["tf", "tfvars", "hcl"],
            Self::Php => &["php", "phtml"],
            Self::Sql => &["sql"],
            Self::Dockerfile => &["dockerfile"],
            Self::Make => &["mk"],
            Self::ShellTemplate => &["sh.template"],
            Self::Ini => &["ini"],
            Self::IniLines => &["cfg", "service", "timer", "repo"],
            Self::Env => &["env"],
            Self::Ignore => &[],
            Self::Jinja2 => &["j2", "jinja", "jinja2"],
            Self::Rust => &["rs"],
            Self::Javascript => &["js", "mjs", "cjs", "jsx"],
            Self::Typescript => &["ts", "mts", "cts"],
            Self::Tsx => &["tsx"],
        }
    }

    pub const fn filenames(self) -> &'static [&'static str] {
        match self {
            Self::Dockerfile => &["Dockerfile", "Dockerfile.*", "*.Dockerfile"],
            Self::Make => &["Makefile", "makefile", "GNUmakefile"],
            Self::Env => &[".env", ".env.*", "*env.example"],
            Self::Ignore => &[".gitignore", ".dockerignore"],
            _ => &[],
        }
    }
}

pub fn detect(path: &Path, content: &str) -> Result<Option<Language>, String> {
    if content.as_bytes().contains(&0) {
        return Ok(None);
    }

    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let lower_name = name.to_ascii_lowercase();

    if lower_name.ends_with(".sh.template") {
        return Ok(Some(Language::ShellTemplate));
    }

    if let Some(lang) = detect_by_exact_suffix(name) {
        return Ok(Some(lang));
    }

    let shebang_lang = if content.starts_with("#!") {
        shebang_language(content)?
    } else {
        None
    };

    if shebang_lang == Some(Language::Python) {
        return Ok(Some(Language::Python));
    }

    if is_php_source(content) {
        return Ok(Some(Language::Php));
    }

    if lower_name.contains(".ini.template.") || lower_name.ends_with(".ini.template") {
        return Ok(Some(Language::Ini));
    }

    let effective_name = name.strip_suffix(".template").unwrap_or(name);
    if let Some(lang) = detect_by_filename(name) {
        return Ok(Some(lang));
    }

    if let Some(lang) = detect_by_extension(effective_name) {
        return Ok(Some(lang));
    }

    Ok(shebang_lang)
}

fn detect_by_exact_suffix(name: &str) -> Option<Language> {
    let ext = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "py" | "pyi" => Some(Language::Python),
        "go" => Some(Language::Go),
        "sh" | "bash" | "bats" | "ksh" | "zsh" => Some(Language::Shell),
        "nu" => Some(Language::Nushell),
        "yml" | "yaml" => Some(Language::Yaml),
        "toml" => Some(Language::Toml),
        "tf" | "tfvars" | "hcl" => Some(Language::Hcl),
        "rs" => Some(Language::Rust),
        "js" | "mjs" | "cjs" | "jsx" => Some(Language::Javascript),
        "ts" | "mts" | "cts" => Some(Language::Typescript),
        "tsx" => Some(Language::Tsx),
        "j2" | "jinja" | "jinja2" => Some(Language::Jinja2),
        _ => None,
    }
}

fn detect_by_filename(name: &str) -> Option<Language> {
    match name {
        "Makefile" | "makefile" | "GNUmakefile" => Some(Language::Make),
        "Dockerfile" => Some(Language::Dockerfile),
        ".env" => Some(Language::Env),
        ".gitignore" | ".dockerignore" => Some(Language::Ignore),
        _ if name.starts_with("Dockerfile.") || name.ends_with(".Dockerfile") => {
            Some(Language::Dockerfile)
        }
        _ if name.starts_with(".env.")
            || Path::new(name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("env"))
            || name.ends_with("env.example") =>
        {
            Some(Language::Env)
        }
        _ => None,
    }
}

fn detect_by_extension(name: &str) -> Option<Language> {
    let ext = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "php" | "phtml" => Some(Language::Php),
        "sql" => Some(Language::Sql),
        "ini" => Some(Language::Ini),
        "cfg" | "service" | "timer" | "repo" => Some(Language::IniLines),
        "mk" => Some(Language::Make),
        "dockerfile" => Some(Language::Dockerfile),
        _ => None,
    }
}

fn is_php_source(content: &str) -> bool {
    let body = content.strip_prefix('\u{feff}').unwrap_or(content);
    let body = if body.starts_with("#!") {
        body.split_once('\n').map_or("", |(_, rest)| rest)
    } else {
        body
    };
    let trimmed = body.trim_start();
    trimmed.len() >= 5 && trimmed[..5].eq_ignore_ascii_case("<?php")
}

const ENV_VALUE_OPTIONS: &[&str] = &["-u", "--unset", "-C", "--chdir", "-a", "--argv0"];

const UV_VALUE_OPTIONS: &[&str] = &[
    "--extra",
    "--no-extra",
    "--group",
    "--no-group",
    "--only-group",
    "--no-editable-package",
    "--env-file",
    "--with",
    "--with-editable",
    "--with-requirements",
    "--package",
    "--python-platform",
    "--index",
    "--default-index",
    "--index-url",
    "--extra-index-url",
    "--find-links",
    "--index-strategy",
    "--keyring-provider",
    "--upgrade-package",
    "--upgrade-group",
    "--resolution",
    "--prerelease",
    "--prerelease-package",
    "--fork-strategy",
    "--exclude-newer",
    "--exclude-newer-package",
    "--no-sources-package",
    "--reinstall-package",
    "--link-mode",
    "--config-setting",
    "--config-settings-package",
    "--no-build-isolation-package",
    "--no-build-package",
    "--no-binary-package",
    "--cache-dir",
    "--refresh-package",
    "--python",
    "--color",
    "--allow-insecure-host",
    "--directory",
    "--project",
    "--config-file",
    "-w",
    "-i",
    "-f",
    "-P",
    "-C",
    "-p",
];

const UV_PYTHON_MODES: &[&str] = &["-m", "--module", "-s", "--script", "--gui-script"];

fn shebang_language(content: &str) -> Result<Option<Language>, String> {
    let line = content
        .strip_prefix("#!")
        .unwrap_or("")
        .split('\n')
        .next()
        .unwrap_or("")
        .trim_end_matches('\r');
    let mut words = split_words(line)?;
    while !words.is_empty() {
        let head = words.remove(0);
        let name = head.rsplit('/').next().unwrap_or(&head);
        if name == "env" {
            let has_split = words.iter().any(|w| {
                w.starts_with("-S")
                    || w.split_once('=')
                        .is_some_and(|(k, _)| k == "--split-string")
            });
            if has_split && (line.contains('\\') || line.contains("${")) {
                return Err(
                    "cannot scan env split-string escapes or variable expansions".to_string(),
                );
            }
            words = env_command(words)?;
        } else if name == "uv" {
            words = uv_command(words);
        } else {
            return Ok(interpreter_language(name));
        }
    }
    Ok(None)
}

fn split_words(input: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_word = false;
    let mut quote: Option<char> = None;

    while let Some(ch) = chars.next() {
        match quote {
            Some('\'') => {
                if ch == '\'' {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            Some('"') => {
                if ch == '"' {
                    quote = None;
                } else if ch == '\\' {
                    let Some(escaped) = chars.next() else {
                        return Err("unterminated escape in shebang".to_string());
                    };
                    current.push(escaped);
                } else {
                    current.push(ch);
                }
            }
            None => match ch {
                c if c.is_ascii_whitespace() => {
                    if in_word {
                        words.push(std::mem::take(&mut current));
                        in_word = false;
                    }
                }
                '\'' | '"' => {
                    in_word = true;
                    quote = Some(ch);
                }
                '\\' => {
                    in_word = true;
                    let Some(escaped) = chars.next() else {
                        return Err("unterminated escape in shebang".to_string());
                    };
                    current.push(escaped);
                }
                _ => {
                    in_word = true;
                    current.push(ch);
                }
            },
            Some(_) => unreachable!(),
        }
    }

    if quote.is_some() {
        return Err("unterminated quote in shebang".to_string());
    }
    if in_word {
        words.push(current);
    }
    Ok(words)
}

fn env_command(mut words: Vec<String>) -> Result<Vec<String>, String> {
    while !words.is_empty() {
        let word = words.remove(0);
        if let Some(rest) = word.strip_prefix("-S").filter(|r| !r.is_empty()) {
            let mut expanded = split_words(rest)?;
            expanded.extend(words);
            words = expanded;
        } else if let Some(rest) = word.strip_prefix("--split-string=") {
            let mut expanded = split_words(rest)?;
            expanded.extend(words);
            words = expanded;
        } else if word == "--" {
            return Ok(words);
        } else if !skip_option(&word, &mut words, ENV_VALUE_OPTIONS) && !is_env_assignment(&word) {
            let mut result = vec![word];
            result.extend(words);
            return Ok(result);
        }
    }
    Ok(words)
}

fn uv_command(mut words: Vec<String>) -> Vec<String> {
    while !words.is_empty() {
        let word = words.remove(0);
        if UV_PYTHON_MODES.contains(&word.as_str()) {
            return vec!["python".to_string()];
        }
        if word == "--" {
            return words;
        }
        if word == "run" || skip_option(&word, &mut words, UV_VALUE_OPTIONS) {
            continue;
        }
        let mut result = vec![word];
        result.extend(words);
        return result;
    }
    vec!["python".to_string()]
}

fn skip_option(word: &str, rest: &mut Vec<String>, value_options: &[&str]) -> bool {
    if value_options.contains(&word) && !rest.is_empty() {
        rest.remove(0);
    }
    word.starts_with('-')
}

fn is_env_assignment(word: &str) -> bool {
    let Some((key, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn interpreter_language(name: &str) -> Option<Language> {
    let stripped = name.strip_prefix("platform-").unwrap_or(name);
    if is_versioned_prefix(stripped, "python") || is_versioned_prefix(stripped, "pypy") {
        return Some(Language::Python);
    }
    if matches!(
        name,
        "sh" | "bash" | "dash" | "ksh" | "mksh" | "ash" | "zsh"
    ) {
        return Some(Language::Shell);
    }
    if name == "nu" {
        return Some(Language::Nushell);
    }
    if is_versioned_prefix(name, "php") {
        return Some(Language::Php);
    }
    if matches!(name, "node" | "nodejs" | "deno" | "bun") {
        return Some(Language::Javascript);
    }
    None
}

fn is_versioned_prefix(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit() || c == '.'))
}
