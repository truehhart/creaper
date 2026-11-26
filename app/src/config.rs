use crate::language::Language;
use globset::{GlobBuilder, GlobMatcher};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_FILENAMES: &[&str] = &["creaper.toml", ".creaper.toml"];

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawConfig {
    pub generated_registry: Option<PathBuf>,
    #[serde(default)]
    pub exempt: Vec<String>,
    pub fail_on_parse_error: Option<bool>,
    #[serde(default)]
    pub allow: AllowLayer,
    #[serde(default)]
    pub languages: BTreeMap<String, LanguageConfig>,
    #[serde(default)]
    pub overrides: Vec<PathOverride>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllowLayer {
    pub shebang: Option<bool>,
    pub encoding: Option<bool>,
    pub script_metadata: Option<bool>,
    pub compiler_directives: Option<bool>,
    pub lint_directives: Option<bool>,
    pub doc_comments: Option<bool>,
    pub docstrings: Option<bool>,
    pub string_statements: Option<bool>,
    pub todos: Option<bool>,
    pub license_headers: Option<bool>,
    #[serde(default)]
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageConfig {
    pub enabled: Option<bool>,
    #[serde(default)]
    pub markers: Vec<String>,
    #[serde(default)]
    pub allow: AllowLayer,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathOverride {
    pub files: Vec<String>,
    #[serde(default)]
    pub allow: AllowLayer,
}

#[derive(Debug, Clone)]
pub struct CompiledOverride {
    pub matchers: Vec<GlobMatcher>,
    pub allow: AllowLayer,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub root: PathBuf,
    pub fail_on_parse_error: bool,
    pub exempt: Vec<ExemptMatcher>,
    pub generated: HashSet<String>,
    pub allow: AllowLayer,
    pub languages: BTreeMap<Language, LanguageConfig>,
    pub overrides: Vec<CompiledOverride>,
}

#[derive(Debug, Clone)]
pub enum ExemptMatcher {
    DirectoryPrefix(String),
    Exact(String),
    Glob(GlobMatcher),
}

impl ExemptMatcher {
    pub fn compile(pattern: &str) -> Result<Self, String> {
        let normalized = normalize_rel_path(pattern);
        if normalized.ends_with('/') {
            return Ok(Self::DirectoryPrefix(normalized));
        }
        if !normalized.contains(['*', '?', '[', '{']) {
            return Ok(Self::Exact(normalized));
        }
        let matcher = compile_glob(&normalized)?;
        Ok(Self::Glob(matcher))
    }

    pub fn is_match(&self, rel_path: &str) -> bool {
        match self {
            Self::DirectoryPrefix(prefix) => rel_path.starts_with(prefix),
            Self::Exact(exact) => rel_path == exact,
            Self::Glob(glob) => glob.is_match(rel_path),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            fail_on_parse_error: true,
            exempt: Vec::new(),
            generated: HashSet::new(),
            allow: AllowLayer::default(),
            languages: BTreeMap::new(),
            overrides: Vec::new(),
        }
    }
}

impl Config {
    pub fn discover(start_dir: &Path) -> Result<Self, String> {
        let mut current = start_dir.to_path_buf();
        loop {
            let mut found = Vec::new();
            for name in CONFIG_FILENAMES {
                let candidate = current.join(name);
                if candidate.is_file() {
                    found.push(candidate);
                }
            }
            if found.len() > 1 {
                return Err(format!(
                    "multiple config files found in {}: {}",
                    current.display(),
                    found
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if let Some(path) = found.pop() {
                return Self::load(&path);
            }
            if current.join(".git").exists() {
                break;
            }
            let Some(parent) = current.parent() else {
                break;
            };
            current = parent.to_path_buf();
        }
        Ok(Self {
            root: start_dir.to_path_buf(),
            ..Self::default()
        })
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let root = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Self::load_with_root(path, root)
    }

    pub fn load_with_root(path: &Path, root: PathBuf) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|err| format!("failed to read config {}: {err}", path.display()))?;
        Self::parse(&content, root)
    }

    pub fn parse(content: &str, root: PathBuf) -> Result<Self, String> {
        let raw: RawConfig =
            toml::from_str(content).map_err(|err| format!("invalid config: {err}"))?;

        let mut exempt = Vec::with_capacity(raw.exempt.len());
        for entry in &raw.exempt {
            exempt.push(ExemptMatcher::compile(entry)?);
        }

        let generated = match &raw.generated_registry {
            Some(rel) => load_generated_registry(&root.join(rel))?,
            None => HashSet::new(),
        };

        let mut languages = BTreeMap::new();
        for (name, cfg) in raw.languages {
            let Some(language) = Language::from_name(&name) else {
                return Err(format!("unknown language in config: {name}"));
            };
            if languages.insert(language, cfg).is_some() {
                return Err(format!(
                    "duplicate language config for {}",
                    language.as_str()
                ));
            }
        }

        let mut overrides = Vec::with_capacity(raw.overrides.len());
        for item in raw.overrides {
            if item.files.is_empty() {
                return Err("override entry must specify at least one file glob".to_string());
            }
            let mut matchers = Vec::with_capacity(item.files.len());
            for pattern in &item.files {
                let normalized = normalize_rel_path(pattern);
                matchers.push(compile_glob(&normalized)?);
            }
            overrides.push(CompiledOverride {
                matchers,
                allow: item.allow,
            });
        }

        Ok(Self {
            root,
            fail_on_parse_error: raw.fail_on_parse_error.unwrap_or(true),
            exempt,
            generated,
            allow: raw.allow,
            languages,
            overrides,
        })
    }

    pub fn is_exempt(&self, rel_path: &str) -> bool {
        let normalized = normalize_rel_path(rel_path);
        if self.generated.contains(&normalized) {
            return true;
        }
        self.exempt
            .iter()
            .any(|matcher| matcher.is_match(&normalized))
    }

    pub fn is_language_enabled(&self, language: Language) -> bool {
        if let Some(cfg) = self.languages.get(&language)
            && let Some(enabled) = cfg.enabled
        {
            return enabled;
        }
        if let Some(parent) = language.parent_config_language()
            && let Some(cfg) = self.languages.get(&parent)
            && let Some(enabled) = cfg.enabled
        {
            return enabled;
        }
        true
    }
}

fn load_generated_registry(path: &Path) -> Result<HashSet<String>, String> {
    let content = fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read generated_registry {}: {err}",
            path.display()
        )
    })?;
    let table: toml::Table = toml::from_str(&content)
        .map_err(|err| format!("invalid generated_registry {}: {err}", path.display()))?;
    Ok(table
        .into_iter()
        .map(|(key, _)| normalize_rel_path(&key))
        .collect())
}

fn compile_glob(pattern: &str) -> Result<GlobMatcher, String> {
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|g| g.compile_matcher())
        .map_err(|err| format!("invalid glob pattern '{pattern}': {err}"))
}

pub fn normalize_rel_path(path: &str) -> String {
    let replaced = path.replace('\\', "/");
    replaced.strip_prefix("./").unwrap_or(&replaced).to_string()
}
