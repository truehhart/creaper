# creaper

Strict, AST-accurate multi-language comment and docstring linter powered by Rust and tree-sitter.

Built to keep LLM prose comments, explanatory docstrings, and inline bypasses out of codebases across every language in a repository, packaged as a single binary with zero runtime dependencies.

## Why creaper

- **Zero inline bypasses**: no `~keep` or magic inline escape comment that an agent can use to pass CI.
- **Full-match allow rules**: every built-in directive and custom regex pattern matches the entire comment (`\A...\z`), so `# noqa: F401 explaining my hack` and `//nolint:foo // reason` are refused.
- **Typed allow-list categories**: toggle categories independently globally, per language, or per file glob (`shebang`, `encoding`, `script_metadata`, `compiler_directives`, `lint_directives`, `doc_comments`, `docstrings`, `string_statements`, `todos`, `license_headers`, `markers`, `patterns`).
- **Real syntax awareness**: tree-sitter grammars plus targeted structural scanners distinguish real comments from string literals, heredocs, YAML block scalars, PHP/HTML boundaries, and Rust/PHP attributes; catches Python attribute docstrings and bare string/f-string statements; scans embedded shell inside Dockerfile `RUN` and Makefile recipes.
- **Fail-closed**: unknown config keys, broken globs, invalid regexes, and parse errors exit `2`.

## Install

### mise

```toml
[tools]
"github:truehhart/creaper" = "0.1.0"
```

### Cargo

```sh
cargo install --git https://github.com/truehhart/creaper.git
```

## Usage

```sh
creaper
creaper --tracked
creaper src/ tests/
creaper --config creaper.toml --format json .
creaper --list languages
creaper --list categories
```

Exit codes:

- `0`: clean
- `1`: forbidden comments or docstrings found
- `2`: configuration, IO, or parse error

## Supported languages

| Language | Extensions / Filenames / Detection |
| --- | --- |
| `python` | `*.py`, `*.pyi`, shebang (`python*`, `pypy*`, `uv run`) |
| `go` | `*.go` |
| `shell` | `*.sh`, `*.bash`, `*.bats`, `*.ksh`, `*.zsh`, shebang (`sh`, `bash`, `dash`, `ash`, `ksh`, `mksh`, `zsh`) |
| `nushell` | `*.nu`, shebang (`nu`) |
| `yaml` | `*.yaml`, `*.yml` |
| `toml` | `*.toml` |
| `hcl` | `*.tf`, `*.tfvars`, `*.hcl` |
| `php` | `*.php`, `*.phtml`, shebang (`php*`), `<?php` header |
| `sql` | `*.sql` |
| `dockerfile` | `Dockerfile`, `Dockerfile.*`, `*.Dockerfile`, `*.dockerfile` (including embedded `RUN` shell) |
| `make` | `Makefile`, `makefile`, `GNUmakefile`, `*.mk` (including embedded recipe shell) |
| `shell-template` | `*.sh.template` |
| `ini` | `*.ini`, `*.ini.template*` |
| `ini-lines` | `*.cfg`, `*.service`, `*.timer`, `*.repo` |
| `env` | `.env`, `.env.*`, `*.env`, `*env.example` |
| `ignore` | `.gitignore`, `.dockerignore` |
| `jinja2` | `*.j2`, `*.jinja`, `*.jinja2` |
| `rust` | `*.rs` |
| `javascript` | `*.js`, `*.mjs`, `*.cjs`, `*.jsx`, shebang (`node`, `deno`, `bun`) |
| `typescript` | `*.ts`, `*.mts`, `*.cts` |
| `tsx` | `*.tsx` |

## Configuration

`creaper` auto-discovers `creaper.toml` or `.creaper.toml` walking up to the git root, or accepts `--config FILE` / `--no-config`. Unknown keys are rejected.

### Allow-list categories

| Category | Default | Scope |
| --- | --- | --- |
| `shebang` | `true` | Line 1 `#!` in script languages |
| `encoding` | `true` | Python PEP 263 coding cookie on line 1 or 2 |
| `script_metadata` | `true` | Python PEP 723 `# /// script` TOML block (validated, zero inner comments) |
| `compiler_directives` | `true` | `//go:<known>`, Dockerfile `# syntax=` / `# escape=`, TS `/// <reference ... />` |
| `lint_directives` | `true` | Bare `noqa`, `type: ignore`, `ty: ignore`, `pragma: no cover/branch`, `fmt:`, `isort:`, `nolint`, `shellcheck`, `hadolint`, `yamllint`, `tflint-ignore`, `phpcs`, `@phpstan-ignore`, `eslint-disable`, `biome-ignore`, `@ts-*`, `prettier-ignore` |
| `doc_comments` | `false` | `///`, `//!`, `/** */`, `/*! */` |
| `docstrings` | `false` | Leading module, class, and function docstrings in Python |
| `string_statements` | `false` | Non-docstring bare string, bytes, or f-string expression statements in Python |
| `todos` | `false` | `TODO:` / `FIXME:` comments |
| `license_headers` | `false` | `SPDX-License-Identifier` / `SPDX-FileCopyrightText` in the top 5 lines |

### Example `creaper.toml`

```toml
generated_registry = "generated.toml"
exempt = ["vendor/", "ai_gens/", "legacy.py"]
fail_on_parse_error = true

[allow]
shebang = true
encoding = true
script_metadata = true
compiler_directives = true
lint_directives = true
doc_comments = false
docstrings = false
string_statements = false
todos = false
license_headers = false
patterns = []

[languages.go]
markers = ["kubebuilder:object:root", "kubebuilder:validation:Minimum", "optional"]

[languages.shell.allow]
patterns = ["# recovery-teardown-contract: idempotent-v1"]

[languages.yaml.allow]
lint_directives = false

[languages.sql]
enabled = false

[[overrides]]
files = [".pre-commit-config.yaml"]

[overrides.allow]
patterns = ['# frozen: v[0-9]+\.[0-9]+(?:\.[0-9]+)?']
```

## pre-commit

```yaml
repos:
  - repo: local
    hooks:
      - id: creaper
        name: no comments or docstrings
        entry: mise exec -- creaper
        language: system
```
