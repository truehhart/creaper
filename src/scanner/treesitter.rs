use crate::language::Language;
use crate::policy::{Comment, CommentKind};
use tree_sitter::{Language as TsLanguage, Node, Parser};

pub fn scan(
    language: Language,
    source: &str,
    fail_on_parse_error: bool,
) -> Result<Vec<Comment>, String> {
    let ts_lang = grammar_for(language);
    let mut parser = Parser::new();
    parser
        .set_language(&ts_lang)
        .map_err(|err| format!("failed to load grammar for {}: {err}", language.as_str()))?;

    let effective_source = preprocess_source(language, source);
    let tree = parser
        .parse(effective_source, None)
        .ok_or_else(|| format!("failed to parse {}", language.as_str()))?;

    let root = tree.root_node();
    if fail_on_parse_error && root.has_error() {
        let (line, col) = first_error_position(root);
        return Err(format!(
            "parse error in {} at line {line}, column {col}",
            language.as_str()
        ));
    }

    let mut comments = Vec::new();
    collect(language, root, effective_source, &mut comments);
    Ok(comments)
}

fn preprocess_source(language: Language, source: &str) -> &str {
    if language == Language::Php {
        let without_bom = source.strip_prefix('\u{feff}').unwrap_or(source);
        return strip_php_halt_compiler(without_bom);
    }
    source
}

fn strip_php_halt_compiler(source: &str) -> &str {
    let bytes = source.as_bytes();
    let needle = b"__halt_compiler";
    let mut i = 0;
    let mut in_single = false;
    let mut in_double = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_single {
            if b == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if b == b'\'' {
                in_single = false;
            }
            i += 1;
            continue;
        }
        if in_double {
            if b == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_double = false;
            }
            i += 1;
            continue;
        }
        if b == b'\'' {
            in_single = true;
            i += 1;
            continue;
        }
        if b == b'"' {
            in_double = true;
            i += 1;
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'#' && (i + 1 >= bytes.len() || bytes[i + 1] != b'[') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        if i + needle.len() <= bytes.len()
            && bytes[i..i + needle.len()].eq_ignore_ascii_case(needle)
        {
            let before_ok =
                i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
            let mut j = i + needle.len();
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if before_ok && j < bytes.len() && bytes[j] == b'(' {
                while j < bytes.len() && bytes[j] != b';' {
                    j += 1;
                }
                let end = if j < bytes.len() { j + 1 } else { bytes.len() };
                return &source[..end];
            }
        }
        i += 1;
    }
    source
}

fn grammar_for(language: Language) -> TsLanguage {
    match language {
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::Shell => tree_sitter_bash::LANGUAGE.into(),
        Language::Nushell => tree_sitter_nu::LANGUAGE.into(),
        Language::Yaml => tree_sitter_yaml::LANGUAGE.into(),
        Language::Toml => tree_sitter_toml_ng::LANGUAGE.into(),
        Language::Hcl => tree_sitter_hcl::LANGUAGE.into(),
        Language::Php => tree_sitter_php::LANGUAGE_PHP.into(),
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::Javascript => tree_sitter_javascript::LANGUAGE.into(),
        Language::Typescript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        _ => unreachable!("non-tree-sitter language routed to treesitter scanner"),
    }
}

fn first_error_position(node: Node) -> (usize, usize) {
    if node.is_error() || node.is_missing() {
        let pos = node.start_position();
        return (pos.row + 1, pos.column + 1);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.has_error() {
            return first_error_position(child);
        }
    }
    let pos = node.start_position();
    (pos.row + 1, pos.column + 1)
}

fn collect(language: Language, node: Node, source: &str, out: &mut Vec<Comment>) {
    if let Some(comment) = classify_node(language, node, source) {
        out.push(comment);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(language, child, source, out);
    }
}

fn classify_node(language: Language, node: Node, source: &str) -> Option<Comment> {
    let kind = node.kind();
    let pos = node.start_position();
    let line = pos.row + 1;
    let column = pos.column + 1;

    match language {
        Language::Python => {
            if kind == "comment" {
                let text = node_text(node, source)?.trim_end_matches('\r').to_string();
                return Some(Comment {
                    line,
                    column,
                    text,
                    kind: CommentKind::Line,
                });
            }
            if kind == "expression_statement" {
                return classify_python_string_statement(node, source);
            }
            None
        }
        Language::Hcl => {
            if kind == "comment" {
                let text = node_text(node, source)?.trim_end_matches('\r').to_string();
                let comment_kind = if text.starts_with("/*") {
                    CommentKind::Block
                } else {
                    CommentKind::Line
                };
                return Some(Comment {
                    line,
                    column,
                    text,
                    kind: comment_kind,
                });
            }
            None
        }
        Language::Shell | Language::Nushell | Language::Yaml | Language::Toml => {
            if kind == "comment" {
                let text = node_text(node, source)?.trim_end_matches('\r').to_string();
                return Some(Comment {
                    line,
                    column,
                    text,
                    kind: CommentKind::Line,
                });
            }
            None
        }
        Language::Php => {
            if kind == "comment" {
                let text = node_text(node, source)?.trim_end_matches('\r').to_string();
                let comment_kind = classify_c_style_comment(&text);
                return Some(Comment {
                    line,
                    column,
                    text,
                    kind: comment_kind,
                });
            }
            None
        }
        Language::Rust => {
            if kind == "line_comment" {
                let text = node_text(node, source)?
                    .trim_end_matches('\r')
                    .trim_end_matches('\n')
                    .to_string();
                let is_doc = (text.starts_with("///") && !text.starts_with("////"))
                    || text.starts_with("//!");
                return Some(Comment {
                    line,
                    column,
                    text,
                    kind: if is_doc {
                        CommentKind::Doc
                    } else {
                        CommentKind::Line
                    },
                });
            }
            if kind == "block_comment" {
                let text = node_text(node, source)?.trim_end_matches('\r').to_string();
                let is_doc =
                    (text.starts_with("/**") && !text.starts_with("/***") && text != "/**/")
                        || text.starts_with("/*!");
                return Some(Comment {
                    line,
                    column,
                    text,
                    kind: if is_doc {
                        CommentKind::Doc
                    } else {
                        CommentKind::Block
                    },
                });
            }
            None
        }
        Language::Javascript | Language::Typescript | Language::Tsx => {
            if kind == "comment" || kind == "hash_bang_line" {
                let text = node_text(node, source)?.trim_end_matches('\r').to_string();
                let comment_kind = classify_c_style_comment(&text);
                return Some(Comment {
                    line,
                    column,
                    text,
                    kind: comment_kind,
                });
            }
            None
        }
        _ => None,
    }
}

fn classify_c_style_comment(text: &str) -> CommentKind {
    if text.starts_with("/**") && text != "/**/" && !text.starts_with("/***") {
        CommentKind::Doc
    } else if text.starts_with("/*") {
        CommentKind::Block
    } else {
        CommentKind::Line
    }
}

fn classify_python_string_statement(expr_stmt: Node, source: &str) -> Option<Comment> {
    let mut cursor = expr_stmt.walk();
    let named_children: Vec<Node> = expr_stmt
        .named_children(&mut cursor)
        .filter(|child| child.kind() != "comment")
        .collect();
    if named_children.len() != 1 {
        return None;
    }
    let value = named_children[0];
    if !matches!(value.kind(), "string" | "concatenated_string") {
        return None;
    }

    let text = node_text(value, source)?;
    let pos = value.start_position();
    let is_plain_str = is_plain_python_string(value, source);
    let is_docstring_pos = is_python_docstring_position(expr_stmt);
    let comment_kind = if is_plain_str && is_docstring_pos {
        CommentKind::Docstring
    } else {
        CommentKind::StringStatement
    };

    Some(Comment {
        line: pos.row + 1,
        column: pos.column + 1,
        text: text.to_string(),
        kind: comment_kind,
    })
}

fn is_plain_python_string(node: Node, source: &str) -> bool {
    if node.kind() == "concatenated_string" {
        let mut cursor = node.walk();
        return node
            .named_children(&mut cursor)
            .filter(|c| c.kind() != "comment")
            .all(|c| is_plain_python_string(c, source));
    }
    if node.kind() != "string" {
        return false;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "interpolation" {
            return false;
        }
        if child.kind() == "string_start"
            && let Some(start) = node_text(child, source)
        {
            let prefix = start.trim_end_matches(['\'', '"']);
            if prefix
                .bytes()
                .any(|b| matches!(b.to_ascii_lowercase(), b'b' | b'f' | b't'))
            {
                return false;
            }
        }
    }
    true
}

fn is_python_docstring_position(expr_stmt: Node) -> bool {
    let Some(parent) = expr_stmt.parent() else {
        return false;
    };
    let in_docstring_container = match parent.kind() {
        "module" => true,
        "block" => parent
            .parent()
            .is_some_and(|gp| matches!(gp.kind(), "function_definition" | "class_definition")),
        _ => false,
    };
    if !in_docstring_container {
        return false;
    }
    let mut cursor = parent.walk();
    for sibling in parent.named_children(&mut cursor) {
        if sibling.kind() == "comment" {
            continue;
        }
        return sibling.id() == expr_stmt.id();
    }
    false
}

fn node_text<'a>(node: Node, source: &'a str) -> Option<&'a str> {
    source.get(node.start_byte()..node.end_byte())
}
