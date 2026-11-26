use creaper::config::Config;
use creaper::runner::check_source;
use std::path::PathBuf;

fn config_from(toml_str: &str) -> Config {
    Config::parse(toml_str, PathBuf::from(".")).unwrap()
}

fn default_config() -> Config {
    Config::default()
}

const CLEAN_CASES: &[(&str, &str)] = &[
    (
        "a.py",
        concat!(
            "#!/usr/bin/env python3\n",
            "# -*- coding: utf-8 -*-\n",
            "import os  # noqa: F401\n",
            "X = os.sep  # type: ignore[attr-defined]  # noqa: E501\n",
            "Y = \"# not a comment\"  # pragma: no cover\n",
            "# fmt: off\n",
            "# isort: skip_file\n",
            "Z = os.sep  # ty: ignore[unresolved-attribute, invalid-assignment]\n",
        ),
    ),
    ("b.pyi", "X: int  # type: ignore\n"),
    ("g.py", "\n# coding: utf-8\nX = 1\n"),
    (
        "pep723.py",
        concat!(
            "# /// script\n",
            "# requires-python = \">=3.14\"\n",
            "# dependencies = [\"httpx\"]\n",
            "# ///\n",
            "import httpx\n",
        ),
    ),
    (
        "c.go",
        concat!(
            "//go:build linux\n\n",
            "package c\n\n",
            "// +kubebuilder:validation:Minimum=0\n",
            "// +optional\n",
            "var S = \"// not a comment\" //nolint:gochecknoglobals\n",
            "var R = `/* raw // not */`\n",
            "var N = new(0.5)\n",
            "var B = new(true)\n",
        ),
    ),
    (
        "d.sh",
        concat!(
            "#!/usr/bin/env bash\n",
            "# recovery-teardown-contract: idempotent-v1\n",
            "# shellcheck disable=SC2086\n",
            "echo \"#not\" $x\n",
            "cat <<EOF\n",
            "# heredoc\n",
            "EOF\n",
        ),
    ),
    ("run", "#!/bin/sh\necho a#b\n"),
    (
        "script.nu",
        concat!(
            "#!/usr/bin/env nu\n",
            "let a = \"# not a comment\"\n",
            "let b = r#'# also not'#\n",
            "print $a $b\n",
        ),
    ),
    ("nu_script", "#!/usr/bin/env nu\nprint \"# ok\"\n"),
    (
        "e.yaml",
        concat!(
            "a: '# not'\n",
            "b: |\n",
            "  # block scalar\n",
            "c: x#y\n",
            "d: 1 # yamllint disable-line rule:line-length\n",
        ),
    ),
    ("f.toml", "a = '''\n# not\n'''\nb = \"#\"\n"),
    (
        "main.tf",
        concat!(
            "resource \"null_resource\" \"x\" {\n",
            "  triggers = {\n",
            "    a = \"# not\"\n",
            "    b = \"// not\"\n",
            "  }\n",
            "}\n",
        ),
    ),
    (
        "terragrunt.hcl",
        concat!(
            "locals {\n",
            "  s = <<-EOF\n",
            "    # heredoc not a comment\n",
            "  EOF\n",
            "}\n",
        ),
    ),
    (
        "page.php",
        concat!(
            "<?php\n",
            "$a = '# not';\n",
            "$b = <<<EOT\n",
            "// not\n",
            "EOT;\n",
            "#[Attribute]\n",
            "class A {}\n",
        ),
    ),
    (
        "view.php",
        "<p>Don't # stop</p>\n<?php echo '?>'; ?>\n<p>// not</p>\n<?= 1 ?>\n",
    ),
    ("tool", "\u{feff}#!/usr/bin/env php\n<?php\necho 1;\n"),
    (
        "halt.php",
        "<?php\necho 1;\n__HALT_COMPILER();\n// data # data\n",
    ),
    (
        "q.sql",
        "SELECT '-- not', $$/* also not */$$ FROM t WHERE x = 'it''s -- ok';\n",
    ),
    (
        "clickhouse_fragment.sql",
        "FROM default.app_version_info AS avi\nPREWHERE avi.dt >= {today:Date} - 3\nWHERE avi.server_id != ''\n",
    ),
    (
        "Dockerfile",
        "# syntax=docker/dockerfile:1\nFROM alpine\nRUN echo \"# not\"\n",
    ),
    ("app.Dockerfile", "FROM alpine\nRUN echo ok\n"),
    (
        "Makefile",
        "all:\n\t# shellcheck disable=SC2086\n\techo $$HOME $(VAR)\n",
    ),
    ("rules.mk", "all:\n\techo a\n"),
    ("run.sh.template", "#!/bin/sh\necho {name}\n"),
    (
        "php.ini.template.php",
        "memory_limit = 384M\nerror_log = \"a;b\"\n",
    ),
    ("app.ini", "[a]\nb = 1\n"),
    ("unit.service", "[Service]\nExecStart=/bin/sh -c 'a ; b'\n"),
    ("tool.cfg", "[x]\nurl = http://x/?a=1 ;b\n"),
    (".env.local", "A=it's\nB='a # b'\nC=x#y\nD=\"x # y\"\n"),
    (".env.example", "A='# not'\n"),
    (".gitignore", "*.pyc\n\\#literal\n"),
    (
        "tpl.yaml.j2",
        "a: {{ \"{# not #}\" }}\n{% raw %}{# not #}{% endraw %}\n",
    ),
    (
        "lib.rs",
        "#[allow(dead_code)]\npub fn f() -> &'static str { r#\"// not /* not */\"# }\n",
    ),
    (
        "app.js",
        "#!/usr/bin/env node\nconst a = `// ${1}`;\nconst re = /http:\\/\\//;\n// eslint-disable-next-line no-console\nconsole.log(a, re);\n",
    ),
    (
        "app.ts",
        "/// <reference types=\"node\" />\n// @ts-expect-error\nconst x: number = \"1\";\nexport { x };\n",
    ),
    (
        "app.tsx",
        "export const App = () => <div>{'// not'}</div>;\n",
    ),
];

const REFUSED_CASES: &[(&str, &str)] = &[
    ("a.py", "X = 1  # why\n"),
    ("a.py", "X = 1  # ~keep bypass attempt\n"),
    ("a.py", "def f():\n    \"\"\"Doc.\"\"\"\n"),
    ("b.pyi", "\"\"\"Module doc.\"\"\"\n"),
    ("b.pyi", "X: int\n\"\"\"Attr doc.\"\"\"\n"),
    ("a.py", "def f():\n    x = 1\n    \"mid string\"\n"),
    ("a.py", "f\"\"\"{1} explains\"\"\"\n"),
    ("a.py", "b\"bytes explanation\"\n"),
    ("a.py", "# this coding: utf-8 is required\n"),
    ("a.py", "X = 1\n# coding: utf-8\n"),
    ("a.py", "X = 1\nY = 2\n# coding: utf-8\n"),
    ("a.py", "X = 1\n#!/usr/bin/env python3\n"),
    ("a.py", "import os  # noqa: F401 kept for re-export\n"),
    ("a.py", "# ty: ignore because why\n"),
    ("a.py", "# pyright: basic\n"),
    ("a.py", "# /// script\n# why = 1\n# ///\n"),
    (
        "a.py",
        "# /// script\n# requires-python = \">=3.14\"\n# # inner comment\n# dependencies = [\"x\"]\n# ///\n",
    ),
    ("c.go", "package c\n\n// C is c.\nvar C = 1\n"),
    ("c.go", "package c\n\n/* block */\n"),
    ("c.go", "package c\n\n//go:explain why\n"),
    ("c.go", "package c\n\n// +kubebuilder:skip\n"),
    (
        "c.go",
        "package c\n\nvar C = 1 //nolint:gochecknoglobals // shared\n",
    ),
    ("d.sh", "echo a # why\n"),
    ("d.sh", "# shellcheck disable=SC2086 because hack\n"),
    ("run", "#!/usr/bin/env bash\n# why\n"),
    ("script.nu", "def main [] {\n  # why\n  print hi\n}\n"),
    ("nu_script", "#!/usr/bin/env nu\n# why\n"),
    ("e.yaml", "a: 1 # why\n"),
    ("e.yaml", "a: 1 # yamllint disable-line because hack\n"),
    ("f.toml", "# why\na = 1\n"),
    ("f.toml", "#! why\na = 1\n"),
    ("main.tf", "resource \"a\" \"b\" {}\n# why\n"),
    ("main.tf", "resource \"a\" \"b\" {}\n// why\n"),
    ("main.tf", "resource \"a\" \"b\" {}\n/* why */\n"),
    ("terragrunt.hcl", "# why\nlocals {}\n"),
    ("page.php", "<?php\n# why\n"),
    ("page.php", "<?php\n// why\n"),
    ("page.php", "<?php\n/* why */\n"),
    ("page.php", "<?php\n/** doc */\n"),
    ("view.php", "<p>It's</p>\n<?php\n// why\n"),
    ("tool", "#!/usr/bin/env php\n<?php\n# why\n"),
    ("halt.php", "<?php\n// why\n__halt_compiler();\n"),
    ("q.sql", "SELECT 1; -- why\n"),
    ("q.sql", "SELECT 1; /* why */\n"),
    ("Dockerfile", "FROM alpine\n# why\n"),
    ("Dockerfile", "FROM alpine\nRUN echo a # why\n"),
    ("Dockerfile", "FROM alpine\nRUN <<EOF\n# why\nEOF\n"),
    (
        "Dockerfile",
        "FROM alpine\nRUN echo a \\\n  # why\n  && echo b\n",
    ),
    ("app.Dockerfile", "FROM alpine\n# why\n"),
    ("Makefile", "all: # why\n\techo a\n"),
    ("Makefile", "all:\n\techo a # why\n"),
    ("rules.mk", "# why\n"),
    ("run.sh.template", "#!/bin/sh\n# why {name}\n"),
    ("app.ini", "; why\n[a]\n"),
    ("app.ini", "[a]\nb = 1 ; why\n"),
    ("php.ini.template.php", "memory_limit = 384M;why\n"),
    ("unit.service", "[Service]\n; why\n"),
    ("tool.cfg", "# why\n[x]\n"),
    (".env.local", "A=it's\n# why\n"),
    (".env.local", "A='x' # why\n"),
    (".env.quoted", "A=\"x\"#why\n"),
    (".env.example", "A=1 # why\n"),
    (".gitignore", "# why\n*.pyc\n"),
    (".dockerignore", "# why\ntarget/\n"),
    ("tpl.j2", "{# why #}\nhello\n"),
    ("tpl.yaml.j2", "{#- why -#}\na: 1\n"),
    ("lib.rs", "// why\npub fn f() {}\n"),
    ("lib.rs", "/* why */\npub fn f() {}\n"),
    ("lib.rs", "/// doc comment\npub fn f() {}\n"),
    ("lib.rs", "//! crate doc\npub fn f() {}\n"),
    ("lib.rs", "/** block doc */\npub fn f() {}\n"),
    ("app.js", "// why\nconst x = 1;\n"),
    (
        "app.js",
        "// eslint-disable-next-line no-console because hack\nconsole.log(1);\n",
    ),
    ("app.ts", "/** jsdoc */\nexport const x = 1;\n"),
    (
        "app.tsx",
        "export const A = () => <div>{/* jsx comment */}</div>;\n",
    ),
];

#[test]
fn clean_files_pass_across_all_languages() {
    let cfg = config_from(
        r##"
[languages.go]
markers = ["kubebuilder:validation:Minimum", "optional"]

[languages.shell.allow]
patterns = ["# recovery-teardown-contract: idempotent-v1"]
"##,
    );

    for (path, content) in CLEAN_CASES {
        let findings = check_source(path, content, &cfg)
            .unwrap_or_else(|err| panic!("unexpected error on {path}: {err}"));
        assert!(
            findings.is_empty(),
            "expected {path} to be clean, got: {findings:?}"
        );
    }
}

#[test]
fn refused_comments_and_bypasses_are_caught_across_all_languages() {
    let cfg = config_from(
        r#"
[languages.go]
markers = ["kubebuilder:validation:Minimum", "optional"]
"#,
    );

    for (path, content) in REFUSED_CASES {
        let findings = check_source(path, content, &cfg)
            .unwrap_or_else(|err| panic!("unexpected error on {path} ({content:?}): {err}"));
        assert!(
            !findings.is_empty(),
            "expected finding on {path} for {content:?}"
        );
    }
}

#[test]
fn shebang_uv_run_detects_python() {
    let cfg = default_config();
    let script = "#!/usr/bin/env -S uv run --script\n# why\nprint(1)\n";
    let findings = check_source("bin/tool", script, &cfg).unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].line, 2);
}

#[test]
fn parse_error_fails_closed_by_default() {
    let cfg = default_config();
    let err = check_source("broken.py", "def f(:\n    pass\n", &cfg).unwrap_err();
    assert!(err.contains("parse error"));

    let go_str_err =
        check_source("broken.go", "package p\nvar s = \"unterminated\n", &cfg).unwrap_err();
    assert!(go_str_err.contains("unterminated Go literal"));

    let go_blk_err = check_source("broken.go", "package p\n/* unterminated", &cfg).unwrap_err();
    assert!(go_blk_err.contains("unterminated Go block comment"));
}

#[test]
fn parse_error_can_be_relaxed_in_config() {
    let cfg = config_from("fail_on_parse_error = false\n");
    let findings = check_source("broken.py", "def f(:\n    # hidden\n    pass\n", &cfg).unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].line, 2);
}
