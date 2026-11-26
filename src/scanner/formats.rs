use crate::policy::{Comment, CommentKind};
use crate::scanner::scan_embedded_shell;
use regex::Regex;
use std::sync::LazyLock;

const PLACEHOLDER: &str = "creaper_placeholder";

struct Cursor<'a> {
    source: &'a str,
    bytes: &'a [u8],
    index: usize,
    line: usize,
    col: usize,
}

impl<'a> Cursor<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            index: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }

    fn peek_next(&self) -> Option<u8> {
        self.bytes.get(self.index + 1).copied()
    }

    fn bump(&mut self) {
        if self.bytes[self.index] == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        self.index += 1;
    }

    fn advance_to(&mut self, target: usize) {
        while self.index < target {
            self.bump();
        }
    }
}

pub fn scan_go(source: &str, fail_on_parse_error: bool) -> Result<Vec<Comment>, String> {
    let mut cur = Cursor::new(source);
    let mut comments = Vec::new();

    while let Some(b) = cur.peek() {
        match b {
            b'"' | b'\'' => skip_go_escaped_literal(&mut cur, b, fail_on_parse_error)?,
            b'`' => skip_go_raw_string(&mut cur, fail_on_parse_error)?,
            b'/' if cur.peek_next() == Some(b'/') => comments.push(read_line_comment(&mut cur)),
            b'/' if cur.peek_next() == Some(b'*') => {
                comments.push(read_block_comment(
                    &mut cur,
                    b'*',
                    b'/',
                    "Go",
                    fail_on_parse_error,
                )?);
            }
            _ => cur.bump(),
        }
    }

    Ok(comments)
}

fn skip_go_escaped_literal(
    cur: &mut Cursor<'_>,
    quote: u8,
    fail_on_parse_error: bool,
) -> Result<(), String> {
    let start_line = cur.line;
    let start_col = cur.col;
    cur.bump();
    while let Some(c) = cur.peek() {
        if c == b'\n' {
            if fail_on_parse_error {
                return Err(format!(
                    "unterminated Go literal at line {start_line}, column {start_col}"
                ));
            }
            return Ok(());
        }
        if c == b'\\' && cur.index + 1 < cur.bytes.len() {
            cur.bump();
            cur.bump();
            continue;
        }
        cur.bump();
        if c == quote {
            return Ok(());
        }
    }
    if fail_on_parse_error {
        return Err(format!(
            "unterminated Go literal at line {start_line}, column {start_col}"
        ));
    }
    Ok(())
}

fn skip_go_raw_string(cur: &mut Cursor<'_>, fail_on_parse_error: bool) -> Result<(), String> {
    let start_line = cur.line;
    let start_col = cur.col;
    cur.bump();
    while let Some(c) = cur.peek() {
        cur.bump();
        if c == b'`' {
            return Ok(());
        }
    }
    if fail_on_parse_error {
        return Err(format!(
            "unterminated Go raw string at line {start_line}, column {start_col}"
        ));
    }
    Ok(())
}

pub fn scan_sql(source: &str, fail_on_parse_error: bool) -> Result<Vec<Comment>, String> {
    let mut cur = Cursor::new(source);
    let mut comments = Vec::new();

    while let Some(b) = cur.peek() {
        if matches!(b, b'\'' | b'"' | b'`') {
            skip_sql_quoted(&mut cur, b);
            continue;
        }
        if b == b'$'
            && let Some(close_end) = match_dollar_quote(source, cur.index)
        {
            cur.advance_to(close_end);
            continue;
        }
        if b == b'-' && cur.peek_next() == Some(b'-') {
            comments.push(read_line_comment(&mut cur));
            continue;
        }
        if b == b'/' && cur.peek_next() == Some(b'*') {
            comments.push(read_block_comment(
                &mut cur,
                b'*',
                b'/',
                "SQL",
                fail_on_parse_error,
            )?);
            continue;
        }
        cur.bump();
    }

    Ok(comments)
}

fn skip_sql_quoted(cur: &mut Cursor<'_>, quote: u8) {
    cur.bump();
    while let Some(c) = cur.peek() {
        if c == b'\\' && quote != b'`' && cur.index + 1 < cur.bytes.len() {
            cur.bump();
            cur.bump();
        } else if c == quote {
            cur.bump();
            if cur.peek() == Some(quote) {
                cur.bump();
            } else {
                break;
            }
        } else {
            cur.bump();
        }
    }
}

fn read_line_comment(cur: &mut Cursor<'_>) -> Comment {
    let start = cur.index;
    let line = cur.line;
    let column = cur.col;
    while let Some(b) = cur.peek() {
        if b == b'\n' {
            break;
        }
        cur.bump();
    }
    Comment {
        line,
        column,
        text: cur.source[start..cur.index]
            .trim_end_matches('\r')
            .to_string(),
        kind: CommentKind::Line,
    }
}

fn read_block_comment(
    cur: &mut Cursor<'_>,
    close_first: u8,
    close_second: u8,
    label: &str,
    fail_on_parse_error: bool,
) -> Result<Comment, String> {
    let start = cur.index;
    let line = cur.line;
    let column = cur.col;
    cur.bump();
    cur.bump();
    let mut closed = false;
    while cur.index + 1 < cur.bytes.len() {
        if cur.peek() == Some(close_first) && cur.peek_next() == Some(close_second) {
            cur.bump();
            cur.bump();
            closed = true;
            break;
        }
        cur.bump();
    }
    if !closed {
        if fail_on_parse_error {
            return Err(format!(
                "unterminated {label} block comment at line {line}, column {column}"
            ));
        }
        cur.advance_to(cur.bytes.len());
    }
    Ok(Comment {
        line,
        column,
        text: cur.source[start..cur.index].to_string(),
        kind: CommentKind::Block,
    })
}

fn match_dollar_quote(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut j = start + 1;
    while j < bytes.len() && bytes[j] != b'$' {
        let c = bytes[j];
        if j == start + 1 {
            if !(c.is_ascii_alphabetic() || c == b'_') {
                return None;
            }
        } else if !(c.is_ascii_alphanumeric() || c == b'_') {
            return None;
        }
        j += 1;
    }
    if j >= bytes.len() || bytes[j] != b'$' {
        return None;
    }
    let tag = &source[start..=j];
    let rest = &source[j + 1..];
    let rel_close = rest.find(tag)?;
    Some(j + 1 + rel_close + tag.len())
}

static DOCKER_ESCAPE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A#\s*escape=(\S+)\s*\z").unwrap());

static DOCKER_DIRECTIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A#\s*(?:syntax|escape)=\S+\s*\z").unwrap());

static DOCKER_RUN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)\A\s*RUN\s+(.*)\z").unwrap());

static DOCKER_HEREDOC_CMD: LazyLock<Regex> = LazyLock::new(|| {
    let shell = r"(?:(?:/usr/bin/|/bin/)?(?:ba|da|a)?sh(?:\s+-\S+)*)";
    Regex::new(&format!(
        r#"\A(?:--\S+\s+)*(?:{shell}\s+)?<<-?\s*['"]?\w+['"]?(?:\s+{shell})?\s*\z"#
    ))
    .unwrap()
});

static HEREDOC_RE: LazyLock<Regex> = LazyLock::new(|| {
    let quoted = r#"'(?:\\.|''|[^'\\])*'|"(?:\\.|""|[^"\\])*"|`(?:\\.|``|[^`\\])*`"#;
    Regex::new(&format!(
        r#"{quoted}|<<-?\s*['"]?(?P<label>[A-Za-z_]\w*)['"]?"#
    ))
    .unwrap()
});

pub fn scan_dockerfile(source: &str, fail_on_parse_error: bool) -> Result<Vec<Comment>, String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut comments = Vec::new();
    let mut escape_char = '\\';
    let mut seen_instruction = false;
    let mut idx = 0;

    while idx < lines.len() {
        let raw_line = lines[idx];
        let trimmed = raw_line.trim_start();

        if trimmed.is_empty() {
            idx += 1;
            continue;
        }

        if trimmed.starts_with('#') {
            if !seen_instruction && DOCKER_DIRECTIVE.is_match(trimmed.trim_end()) {
                if let Some(cap) = DOCKER_ESCAPE.captures(trimmed.trim_end())
                    && let Some(ch) = cap[1].chars().next()
                    && cap[1].chars().count() == 1
                {
                    escape_char = ch;
                }
            } else {
                seen_instruction = true;
            }
            comments.push(trimmed_line_comment(raw_line, idx + 1));
            idx += 1;
            continue;
        }

        seen_instruction = true;
        let start = idx;
        let header_joined =
            collect_docker_instruction(&lines, &mut idx, escape_char, &mut comments);
        let terminator = find_heredoc_label(&header_joined);
        let (body_start, body_end) = consume_docker_heredoc(
            &lines,
            &mut idx,
            terminator.as_deref(),
            start,
            fail_on_parse_error,
        )?;

        if let Some(cap) = DOCKER_RUN.captures(&header_joined) {
            let cmd = &cap[1];
            if !cmd.trim_start().starts_with('[') {
                let span = DockerRunSpan {
                    start,
                    body_start,
                    body_end,
                    end: idx,
                    has_heredoc: terminator.is_some(),
                    escape_char,
                };
                let (base_line, script) = build_docker_run_script(cmd, &lines, span);
                for mut c in scan_embedded_shell(&script, fail_on_parse_error)? {
                    c.line += base_line;
                    comments.push(c);
                }
            }
        }
    }

    Ok(comments)
}

fn trimmed_line_comment(raw_line: &str, line: usize) -> Comment {
    let trimmed = raw_line.trim_start();
    Comment {
        line,
        column: raw_line.len() - trimmed.len() + 1,
        text: trimmed.trim_end().to_string(),
        kind: CommentKind::Line,
    }
}

fn collect_docker_instruction(
    lines: &[&str],
    idx: &mut usize,
    escape_char: char,
    comments: &mut Vec<Comment>,
) -> String {
    let mut header_lines = vec![lines[*idx].to_string()];
    *idx += 1;
    while ends_with_escape(header_lines.last().map_or("", String::as_str), escape_char)
        && *idx < lines.len()
    {
        let next = lines[*idx];
        if next.trim_start().starts_with('#') {
            comments.push(trimmed_line_comment(next, *idx + 1));
            header_lines.push(String::new());
        } else {
            header_lines.push(next.to_string());
        }
        *idx += 1;
    }
    header_lines.join("\n")
}

fn consume_docker_heredoc(
    lines: &[&str],
    idx: &mut usize,
    terminator: Option<&str>,
    start: usize,
    fail_on_parse_error: bool,
) -> Result<(usize, usize), String> {
    let body_start = *idx;
    let Some(label) = terminator else {
        return Ok((body_start, body_start));
    };
    while *idx < lines.len() && lines[*idx].trim() != label {
        *idx += 1;
    }
    if *idx == lines.len() {
        if fail_on_parse_error {
            return Err(format!(
                "unterminated Docker heredoc '{label}' at line {}",
                start + 1
            ));
        }
        return Ok((body_start, *idx));
    }
    let body_end = *idx;
    *idx += 1;
    Ok((body_start, body_end))
}

#[derive(Clone, Copy)]
struct DockerRunSpan {
    start: usize,
    body_start: usize,
    body_end: usize,
    end: usize,
    has_heredoc: bool,
    escape_char: char,
}

fn build_docker_run_script(cmd: &str, lines: &[&str], span: DockerRunSpan) -> (usize, String) {
    if span.has_heredoc && DOCKER_HEREDOC_CMD.is_match(cmd) {
        let body = lines[span.body_start..span.body_end].join("\n");
        return (span.body_start, format!("{body}\n"));
    }
    let cleaned_cmd = strip_run_flags(cmd);
    let normalized = normalize_docker_escapes(cleaned_cmd, span.escape_char);
    let mut full = format!("{normalized}\n");
    if span.body_end > span.body_start {
        full.push_str(&lines[span.body_start..span.end].join("\n"));
        full.push('\n');
    }
    (span.start, full)
}

fn ends_with_escape(line: &str, escape_char: char) -> bool {
    let trimmed = line.trim_end_matches([' ', '\t', '\r']);
    let mut count = 0;
    for ch in trimmed.chars().rev() {
        if ch == escape_char {
            count += 1;
        } else {
            break;
        }
    }
    count % 2 == 1
}

fn normalize_docker_escapes(cmd: &str, escape_char: char) -> String {
    if escape_char == '\\' {
        return cmd.to_string();
    }
    cmd.lines()
        .map(|line| {
            if let Some(prefix) = line.trim_end().strip_suffix(escape_char) {
                format!("{prefix}\\")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_run_flags(mut cmd: &str) -> &str {
    loop {
        let trimmed = cmd.trim_start_matches([' ', '\t']);
        if !trimmed.starts_with("--") {
            return trimmed;
        }
        let Some(end) = trimmed.find(|c: char| c.is_ascii_whitespace()) else {
            return "";
        };
        cmd = &trimmed[end..];
    }
}

fn find_heredoc_label(body: &str) -> Option<String> {
    for cap in HEREDOC_RE.captures_iter(body) {
        if let Some(label) = cap.name("label") {
            return Some(label.as_str().to_string());
        }
    }
    None
}

pub fn scan_make(source: &str, fail_on_parse_error: bool) -> Result<Vec<Comment>, String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut comments = Vec::new();
    let mut idx = 0;

    while idx < lines.len() {
        if lines[idx].starts_with('\t') {
            let start = idx;
            while idx < lines.len() && lines[idx].starts_with('\t') {
                idx += 1;
            }
            let recipe_raw: Vec<String> = lines[start..idx]
                .iter()
                .map(|line| {
                    line.strip_prefix('\t')
                        .unwrap_or(line)
                        .trim_start_matches(['@', '-', '+'])
                        .to_string()
                })
                .collect();
            let script = format!("{}\n", render_make_script(&recipe_raw.join("\n")));
            for mut c in scan_embedded_shell(&script, fail_on_parse_error)? {
                c.line += start;
                comments.push(c);
            }
            continue;
        }

        if let Some((col, text)) = find_make_comment(lines[idx]) {
            comments.push(Comment {
                line: idx + 1,
                column: col,
                text,
                kind: CommentKind::Line,
            });
        }
        idx += 1;
    }

    Ok(comments)
}

fn find_make_comment(line: &str) -> Option<(usize, String)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == b'#' {
            return Some((i + 1, line[i..].trim_end_matches('\r').to_string()));
        }
        i += 1;
    }
    None
}

fn render_make_script(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'$' {
                out.push('$');
                i += 2;
                continue;
            }
            if bytes[i + 1] == b'(' || bytes[i + 1] == b'{' {
                let open = bytes[i + 1];
                let close = if open == b'(' { b')' } else { b'}' };
                let mut depth = 1usize;
                let mut j = i + 2;
                while j < bytes.len() && depth > 0 {
                    if bytes[j] == open {
                        depth += 1;
                    } else if bytes[j] == close {
                        depth -= 1;
                    }
                    j += 1;
                }
                let expr = &text[i..j];
                if expr.contains('#') {
                    out.push_str(expr);
                } else {
                    out.push_str(PLACEHOLDER);
                    let newlines = expr.bytes().filter(|&b| b == b'\n').count();
                    for _ in 0..newlines {
                        out.push('\n');
                    }
                }
                i = j;
                continue;
            }
        }
        if let Some(ch) = text[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }

    out
}

pub fn scan_shell_template(
    source: &str,
    fail_on_parse_error: bool,
) -> Result<Vec<Comment>, String> {
    let rendered = render_shell_template(source)?;
    scan_embedded_shell(&rendered, fail_on_parse_error)
}

fn render_shell_template(source: &str) -> Result<String, String> {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'{' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                out.push('{');
                i += 2;
                continue;
            }
            let Some(rel_end) = source[i + 1..].find('}') else {
                return Err("unterminated shell template placeholder".to_string());
            };
            let field = &source[i + 1..i + 1 + rel_end];
            if !is_valid_ident(field) {
                return Err(format!("unsupported shell template field '{field}'"));
            }
            out.push_str(PLACEHOLDER);
            i += 2 + rel_end;
            continue;
        }
        if bytes[i] == b'}' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'}' {
                out.push('}');
                i += 2;
                continue;
            }
            return Err("unmatched '}' in shell template".to_string());
        }
        if let Some(ch) = source[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }

    Ok(out)
}

fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn scan_ini(source: &str) -> Vec<Comment> {
    let mut comments = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || trimmed.starts_with(';') {
            comments.push(trimmed_line_comment(line, idx + 1));
            continue;
        }
        if let Some((col, text)) = find_inline_semicolon(line) {
            comments.push(Comment {
                line: idx + 1,
                column: col,
                text,
                kind: CommentKind::Line,
            });
        }
    }
    comments
}

fn find_inline_semicolon(line: &str) -> Option<(usize, String)> {
    let bytes = line.as_bytes();
    let mut in_double = false;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'"' {
            in_double = !in_double;
        } else if b == b';' && !in_double {
            return Some((i + 1, line[i..].trim_end_matches('\r').to_string()));
        }
    }
    None
}

pub fn scan_ini_lines(source: &str) -> Vec<Comment> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            trimmed.starts_with('#') || trimmed.starts_with(';')
        })
        .map(|(idx, line)| trimmed_line_comment(line, idx + 1))
        .collect()
}

pub fn scan_env(source: &str) -> Vec<Comment> {
    source
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            find_env_comment(line).map(|(col, text)| Comment {
                line: idx + 1,
                column: col,
                text,
                kind: CommentKind::Line,
            })
        })
        .collect()
}

fn find_env_comment(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return Some((
            line.len() - trimmed.len() + 1,
            trimmed.trim_end_matches('\r').to_string(),
        ));
    }
    let eq_pos = line.find('=')?;
    if line[..eq_pos].contains('#') {
        let hash = line.find('#')?;
        return Some((hash + 1, line[hash..].trim_end_matches('\r').to_string()));
    }
    let bytes = line.as_bytes();
    let mut i = eq_pos + 1;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
        i += 1;
    }
    if i < bytes.len() && (bytes[i] == b'\'' || bytes[i] == b'"') {
        let quote = bytes[i];
        i += 1;
        let mut closed = false;
        while i < bytes.len() {
            if quote == b'"' && bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if bytes[i] == quote {
                i += 1;
                closed = true;
                break;
            }
            i += 1;
        }
        if closed {
            while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'#' {
                return Some((i + 1, line[i..].trim_end_matches('\r').to_string()));
            }
            return None;
        }
    }
    let mut j = eq_pos + 1;
    while j < bytes.len() {
        if bytes[j] == b'#' && (j == 0 || matches!(bytes[j - 1], b' ' | b'\t')) {
            return Some((j + 1, line[j..].trim_end_matches('\r').to_string()));
        }
        j += 1;
    }
    None
}

pub fn scan_ignore(source: &str) -> Vec<Comment> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with('#'))
        .map(|(idx, line)| trimmed_line_comment(line, idx + 1))
        .collect()
}

static JINJA_RAW_OPEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A\{%-?\s*raw\s*-?%\}").unwrap());

static JINJA_RAW_CLOSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{%-?\s*endraw\s*-?%\}").unwrap());

pub fn scan_jinja2(source: &str, fail_on_parse_error: bool) -> Result<Vec<Comment>, String> {
    let mut cur = Cursor::new(source);
    let mut comments = Vec::new();

    while let Some(b) = cur.peek() {
        if b == b'{'
            && let Some(next) = cur.peek_next()
        {
            if next == b'#' {
                comments.push(read_block_comment(
                    &mut cur,
                    b'#',
                    b'}',
                    "Jinja2",
                    fail_on_parse_error,
                )?);
                continue;
            }
            if next == b'%'
                && let Some(raw_open) = JINJA_RAW_OPEN.find(&source[cur.index..])
            {
                let after_open = cur.index + raw_open.end();
                cur.advance_to(after_open);
                if let Some(raw_close) = JINJA_RAW_CLOSE.find(&source[after_open..]) {
                    cur.advance_to(after_open + raw_close.end());
                    continue;
                }
                if fail_on_parse_error {
                    return Err(format!(
                        "unterminated Jinja2 raw block at line {}",
                        cur.line
                    ));
                }
                break;
            }
            if next == b'{' || next == b'%' {
                let close_first = if next == b'{' { b'}' } else { b'%' };
                skip_jinja_tag(&mut cur, close_first);
                continue;
            }
        }
        cur.bump();
    }

    Ok(comments)
}

fn skip_jinja_tag(cur: &mut Cursor<'_>, close_first: u8) {
    cur.bump();
    cur.bump();
    let mut quote: Option<u8> = None;
    while let Some(b) = cur.peek() {
        if let Some(q) = quote {
            if b == b'\\' && cur.index + 1 < cur.bytes.len() {
                cur.bump();
                cur.bump();
                continue;
            }
            if b == q {
                quote = None;
            }
        } else if b == b'\'' || b == b'"' {
            quote = Some(b);
        } else if b == close_first && cur.peek_next() == Some(b'}') {
            cur.bump();
            cur.bump();
            break;
        }
        cur.bump();
    }
}
