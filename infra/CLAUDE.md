# CLAUDE.md

- All sensitive information must be encrypted with `sops`.
- Keep to the following folder structure in every module: [main.tf, versions.tf, variables.tf, outputs.tf]. If a file would be empty, omit it.
- Modules should only define resources and outputs that are needed today.
- Strongly prefer using for_each instead of count, but don't over-engineer to comply.
