# CLAUDE.md

- Always hard-pin actions to a full commit sha. Put the release tag in a trailing comment.
  - `uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0 # v7.0.0`
- Never leave comments in CI steps. Steps must be self-explanatory.
- Always name CI steps.
- Keep `if:` conditions and ternary chains multiline. Use a folded scalar (`>-`) and put one condition or one branch on each line.
- Give every job the minimum permissions it needs. Start from `contents: read`.
- Run `mise run '//.github:validate'` after you change a workflow. It runs actionlint and zizmor.
