# CLAUDE.md

- Use mise for all tools and tasks. Never call tools directly.
- mise for monorepos is configured. Tasks can be ran with `mise run '//<part>:<task>'` or `//:<task>` for root tasks.
- Always `mise run //:validate` before handing over to the operator.
- Always pin versions to their immutable identifier (e.g. sha).
- Use nushell for writing repository scripts. Use what's convenient for tool calls.
- Make sure the pre-commit hooks are ran and pass before you commit.
- Each part of this monorepo has its own `CLAUDE.md`. Read it before you work in that directory.
- Use Conventional Commits.
- This is a personal pet-project repository. Unless explicitly requested, completely ignore backwards-compatibility and commit straight to main.
