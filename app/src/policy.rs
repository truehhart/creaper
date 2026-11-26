use crate::config::{AllowLayer, Config, normalize_rel_path};
use crate::language::Language;
use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommentKind {
    Line,
    Block,
    Doc,
    Docstring,
    StringStatement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllowCategory {
    Shebang,
    Encoding,
    ScriptMetadata,
    CompilerDirectives,
    LintDirectives,
    DocComments,
    Docstrings,
    StringStatements,
    Todos,
    LicenseHeaders,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub line: usize,
    pub column: usize,
    pub text: String,
    pub kind: CommentKind,
}

#[derive(Debug, Clone, Copy)]
pub struct CategoryDescriptor {
    pub name: &'static str,
    pub default_enabled: bool,
    pub description: &'static str,
}

pub const CATEGORIES: &[CategoryDescriptor] = &[
    CategoryDescriptor {
        name: "shebang",
        default_enabled: true,
        description: "Line 1 #! interpreter directive in script languages",
    },
    CategoryDescriptor {
        name: "encoding",
        default_enabled: true,
        description: "PEP 263 Python coding cookie on line 1 or 2",
    },
    CategoryDescriptor {
        name: "script_metadata",
        default_enabled: true,
        description: "PEP 723 # /// script TOML metadata block in Python",
    },
    CategoryDescriptor {
        name: "compiler_directives",
        default_enabled: true,
        description: "Compiler, runtime, or build-tool directives (//go:*, Dockerfile # syntax/escape, TS /// <reference>)",
    },
    CategoryDescriptor {
        name: "lint_directives",
        default_enabled: true,
        description: "Bare linter/formatter/typechecker suppressions with zero trailing prose",
    },
    CategoryDescriptor {
        name: "doc_comments",
        default_enabled: false,
        description: "Structured doc comments (///, //!, /** */, /*! */)",
    },
    CategoryDescriptor {
        name: "docstrings",
        default_enabled: false,
        description: "Leading module, class, and function docstrings in Python",
    },
    CategoryDescriptor {
        name: "string_statements",
        default_enabled: false,
        description: "Standalone bare string, bytes, or f-string expression statements in Python",
    },
    CategoryDescriptor {
        name: "todos",
        default_enabled: false,
        description: "TODO/FIXME comments",
    },
    CategoryDescriptor {
        name: "license_headers",
        default_enabled: false,
        description: "SPDX-License-Identifier and SPDX-FileCopyrightText headers in the top 5 lines",
    },
];

#[derive(Debug, Clone)]
pub struct EffectivePolicy {
    enabled: HashSet<AllowCategory>,
    custom_patterns: Vec<Regex>,
}

impl EffectivePolicy {
    pub fn resolve(config: &Config, language: Language, rel_path: &str) -> Result<Self, String> {
        let mut policy = Self {
            enabled: HashSet::from([
                AllowCategory::Shebang,
                AllowCategory::Encoding,
                AllowCategory::ScriptMetadata,
                AllowCategory::CompilerDirectives,
                AllowCategory::LintDirectives,
            ]),
            custom_patterns: Vec::new(),
        };

        policy.apply_layer(&config.allow)?;

        if language.inherits_shell()
            && let Some(shell_cfg) = config.languages.get(&Language::Shell)
        {
            policy.apply_layer(&shell_cfg.allow)?;
        }

        if let Some(parent) = language.parent_config_language()
            && let Some(parent_cfg) = config.languages.get(&parent)
        {
            policy.apply_layer(&parent_cfg.allow)?;
            policy.apply_markers(parent, &parent_cfg.markers)?;
        }

        if let Some(lang_cfg) = config.languages.get(&language) {
            policy.apply_layer(&lang_cfg.allow)?;
            policy.apply_markers(language, &lang_cfg.markers)?;
        }

        let normalized = normalize_rel_path(rel_path);
        for item in &config.overrides {
            if item.matchers.iter().any(|m| m.is_match(&normalized)) {
                policy.apply_layer(&item.allow)?;
            }
        }

        Ok(policy)
    }

    pub fn has(&self, category: AllowCategory) -> bool {
        self.enabled.contains(&category)
    }

    fn set(&mut self, category: AllowCategory, value: Option<bool>) {
        match value {
            Some(true) => {
                self.enabled.insert(category);
            }
            Some(false) => {
                self.enabled.remove(&category);
            }
            None => {}
        }
    }

    fn apply_layer(&mut self, layer: &AllowLayer) -> Result<(), String> {
        self.set(AllowCategory::Shebang, layer.shebang);
        self.set(AllowCategory::Encoding, layer.encoding);
        self.set(AllowCategory::ScriptMetadata, layer.script_metadata);
        self.set(AllowCategory::CompilerDirectives, layer.compiler_directives);
        self.set(AllowCategory::LintDirectives, layer.lint_directives);
        self.set(AllowCategory::DocComments, layer.doc_comments);
        self.set(AllowCategory::Docstrings, layer.docstrings);
        self.set(AllowCategory::StringStatements, layer.string_statements);
        self.set(AllowCategory::Todos, layer.todos);
        self.set(AllowCategory::LicenseHeaders, layer.license_headers);
        for raw in &layer.patterns {
            let compiled = compile_full_match(raw)?;
            self.custom_patterns.push(compiled);
        }
        Ok(())
    }

    fn apply_markers(&mut self, language: Language, markers: &[String]) -> Result<(), String> {
        if markers.is_empty() {
            return Ok(());
        }
        let escaped = markers
            .iter()
            .map(|m| regex::escape(m))
            .collect::<Vec<_>>()
            .join("|");
        let pattern = match language {
            Language::Go => format!(r"// \+(?:{escaped})(?:[=:].*)?"),
            _ => format!(r"(?://|#)\s*\+(?:{escaped})(?:[=:].*)?"),
        };
        self.custom_patterns.push(compile_full_match(&pattern)?);
        Ok(())
    }

    pub fn is_allowed(
        &self,
        language: Language,
        comment: &Comment,
        first_line: &str,
        metadata_lines: &HashSet<usize>,
    ) -> bool {
        let text = comment.text.as_str();

        if comment.kind == CommentKind::Docstring {
            return self.has(AllowCategory::Docstrings) || self.matches_custom(text);
        }
        if comment.kind == CommentKind::StringStatement {
            return self.has(AllowCategory::StringStatements) || self.matches_custom(text);
        }

        if self.has(AllowCategory::ScriptMetadata) && metadata_lines.contains(&comment.line) {
            return true;
        }

        if self.has(AllowCategory::Shebang)
            && comment.line == 1
            && text.starts_with("#!")
            && language.supports_shebang()
        {
            return true;
        }

        if self.has(AllowCategory::Encoding)
            && language == Language::Python
            && (comment.line == 1
                || (comment.line == 2
                    && (first_line.trim().is_empty() || first_line.trim_start().starts_with('#'))))
            && PYTHON_CODING.is_match(text)
        {
            return true;
        }

        if self.has(AllowCategory::DocComments) && comment.kind == CommentKind::Doc {
            return true;
        }

        if self.has(AllowCategory::CompilerDirectives) && matches_compiler_directive(language, text)
        {
            return true;
        }

        if self.has(AllowCategory::LintDirectives) && matches_lint_directive(language, text) {
            return true;
        }

        if self.has(AllowCategory::Todos) && TODO_DIRECTIVE.is_match(text) {
            return true;
        }

        if self.has(AllowCategory::LicenseHeaders)
            && comment.line <= 5
            && SPDX_HEADER.is_match(text)
        {
            return true;
        }

        self.matches_custom(text)
    }

    fn matches_custom(&self, text: &str) -> bool {
        self.custom_patterns.iter().any(|re| re.is_match(text))
    }
}

pub fn compile_full_match(pattern: &str) -> Result<Regex, String> {
    Regex::new(&format!(r"\A(?:{pattern})\z"))
        .map_err(|err| format!("invalid allow regex '{pattern}': {err}"))
}

static PYTHON_CODING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A#\s*(?:-\*-\s*)?coding[:=]\s*[-\w.]+(?:\s*-\*-)?\s*\z").unwrap()
});

static PYTHON_LINT: LazyLock<Regex> = LazyLock::new(|| {
    let codes = r"[\w-]+(?:\s*,\s*[\w-]+)*";
    let directive = format!(
        r"(?:noqa(?::\s*[A-Z]+[0-9]+(?:\s*,\s*[A-Z]+[0-9]+)*)?|type:\s*ignore(?:\[{codes}\])?|ty:\s*ignore(?:\[{codes}\])?|pragma:\s*no\s+(?:cover|branch)|fmt:\s*(?:off|on|skip)|isort:\s*(?:skip_file|skip|off|on))"
    );
    Regex::new(&format!(r"\A#\s*{directive}(?:\s*#\s*{directive})*\s*\z")).unwrap()
});

static GO_COMPILER: LazyLock<Regex> = LazyLock::new(|| {
    let directives = [
        "build",
        "debug",
        "embed",
        "generate",
        "linkname",
        "nocheckptr",
        "noescape",
        "noinline",
        "norace",
        "nosplit",
        "uintptrescapes",
        "uintptrkeepalive",
        "wasmexport",
        "wasmimport",
    ]
    .join("|");
    Regex::new(&format!(r"\A//go:(?:{directives})\b.*\z")).unwrap()
});

static GO_LINT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A//nolint(?::[\w,-]+)?\z").unwrap());

static SHELL_LINT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A#\s*shellcheck(?:\s+(?:disable|enable|external-sources|shell|source|source-path)=\S+)+\s*\z")
        .unwrap()
});

static DOCKER_COMPILER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A#\s*(?:syntax|escape)=\S+\s*\z").unwrap());

static DOCKER_LINT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A#\s*hadolint\s+ignore=[\w,]+\s*\z").unwrap());

static YAML_LINT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\A#\s*yamllint\s+(?:disable-line|disable-file|disable|enable)(?:\s+rule:[\w-]+)*\s*\z",
    )
    .unwrap()
});

static HCL_LINT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A(?:#|//)\s*tflint-ignore:\s*[\w,-]+\s*\z").unwrap());

static PHP_LINT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\A//\s*(?:phpcs:(?:ignore|disable|enable)(?:\s+[\w.,-]+)*|@phpstan-ignore(?:-(?:next-)?line)?(?:\s+[\w.-]+)*)\s*\z",
    )
    .unwrap()
});

static JS_TS_COMPILER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\A///\s*<reference\s+(?:path|types|lib|no-default-lib)="[^"]+"\s*/>\s*\z"#)
        .unwrap()
});

static JS_TS_LINT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\A//\s*(?:eslint-disable(?:-next-line|-line)?(?:\s+[@\w/-]+(?:\s*,\s*[@\w/-]+)*)?|biome-ignore\s+[\w/-]+|@ts-(?:ignore|expect-error|nocheck|check)|prettier-ignore)\s*\z",
    )
    .unwrap()
});

static TODO_DIRECTIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A(?://|#|/\*|--|;|\{#-?)\s*(?:TODO|FIXME)(?:\([^)]+\))?:\s*\S.*\z").unwrap()
});

static SPDX_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\A(?://|#|/\*|--|;)\s*SPDX-(?:License-Identifier|FileCopyrightText):\s*[^\n*]+(?:\s*\*/)?\s*\z",
    )
    .unwrap()
});

fn matches_compiler_directive(language: Language, text: &str) -> bool {
    match language {
        Language::Go => GO_COMPILER.is_match(text),
        Language::Dockerfile => DOCKER_COMPILER.is_match(text),
        Language::Javascript | Language::Typescript | Language::Tsx => {
            JS_TS_COMPILER.is_match(text)
        }
        _ => false,
    }
}

fn matches_lint_directive(language: Language, text: &str) -> bool {
    match language {
        Language::Python => PYTHON_LINT.is_match(text),
        Language::Go => GO_LINT.is_match(text),
        Language::Shell | Language::Make | Language::ShellTemplate => SHELL_LINT.is_match(text),
        Language::Dockerfile => DOCKER_LINT.is_match(text) || SHELL_LINT.is_match(text),
        Language::Yaml => YAML_LINT.is_match(text),
        Language::Hcl => HCL_LINT.is_match(text),
        Language::Php => PHP_LINT.is_match(text),
        Language::Javascript | Language::Typescript | Language::Tsx => JS_TS_LINT.is_match(text),
        _ => false,
    }
}

static PEP723_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^# /// script\r?\n(?P<body>(?:#(?: [^\n]*)?\r?\n)*)# ///(?:\r?\n|\z)").unwrap()
});

pub fn python_script_metadata_lines(
    text: &str,
    toml_has_comments: fn(&str) -> bool,
) -> HashSet<usize> {
    let mut lines = HashSet::new();
    for cap in PEP723_BLOCK.captures_iter(text) {
        let Some(whole) = cap.get(0) else {
            continue;
        };
        let body = &cap["body"];
        let payload: String = body
            .lines()
            .map(|line| {
                line.strip_prefix("# ")
                    .unwrap_or_else(|| line.strip_prefix('#').unwrap_or(""))
            })
            .collect::<Vec<_>>()
            .join("\n");
        if is_valid_pep723_payload(&payload, toml_has_comments) {
            let start_line = text[..whole.start()]
                .bytes()
                .filter(|&b| b == b'\n')
                .count()
                + 1;
            let line_count = whole.as_str().lines().count();
            for line in start_line..start_line + line_count {
                lines.insert(line);
            }
        }
    }
    lines
}

fn is_valid_pep723_payload(payload: &str, toml_has_comments: fn(&str) -> bool) -> bool {
    if toml_has_comments(payload) {
        return false;
    }
    let Ok(table) = toml::from_str::<toml::Table>(payload) else {
        return false;
    };
    if table.is_empty() {
        return false;
    }
    let mut has_known_meta = false;
    for (key, value) in &table {
        match key.as_str() {
            "requires-python" => {
                if !value.is_str() {
                    return false;
                }
                has_known_meta = true;
            }
            "dependencies" => {
                let Some(items) = value.as_array() else {
                    return false;
                };
                if !items
                    .iter()
                    .all(|item| item.as_str().is_some_and(|s| !s.is_empty()))
                {
                    return false;
                }
                has_known_meta = true;
            }
            "tool" => {
                if !value.is_table() {
                    return false;
                }
            }
            _ => return false,
        }
    }
    has_known_meta
}
