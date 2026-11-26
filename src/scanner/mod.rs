mod formats;
mod treesitter;

use crate::language::Language;
use crate::policy::Comment;

pub fn scan(
    language: Language,
    source: &str,
    fail_on_parse_error: bool,
) -> Result<Vec<Comment>, String> {
    match language {
        Language::Python
        | Language::Shell
        | Language::Nushell
        | Language::Yaml
        | Language::Toml
        | Language::Hcl
        | Language::Php
        | Language::Rust
        | Language::Javascript
        | Language::Typescript
        | Language::Tsx => treesitter::scan(language, source, fail_on_parse_error),
        Language::Go => formats::scan_go(source, fail_on_parse_error),
        Language::Sql => formats::scan_sql(source, fail_on_parse_error),
        Language::Dockerfile => formats::scan_dockerfile(source, fail_on_parse_error),
        Language::Make => formats::scan_make(source, fail_on_parse_error),
        Language::ShellTemplate => formats::scan_shell_template(source, fail_on_parse_error),
        Language::Ini => Ok(formats::scan_ini(source)),
        Language::IniLines => Ok(formats::scan_ini_lines(source)),
        Language::Env => Ok(formats::scan_env(source)),
        Language::Ignore => Ok(formats::scan_ignore(source)),
        Language::Jinja2 => formats::scan_jinja2(source, fail_on_parse_error),
    }
}

pub fn toml_has_comments(source: &str) -> bool {
    treesitter::scan(Language::Toml, source, false).is_ok_and(|comments| !comments.is_empty())
}

pub(crate) fn scan_embedded_shell(
    source: &str,
    fail_on_parse_error: bool,
) -> Result<Vec<Comment>, String> {
    treesitter::scan(Language::Shell, source, fail_on_parse_error)
}
