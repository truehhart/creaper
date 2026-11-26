# CLAUDE.md

- Keep the task names in `mise.toml`. CI and the root tasks depend on them.
- Keep `validate` as the single entry point for every app check.
- Do not leave comments or docstrings in source files. `//app:lint` runs `creaper` on tracked repository files.
