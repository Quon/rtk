# Subversion (svn)

> Part of [`src/cmds/`](../README.md) — see also [docs/contributing/TECHNICAL.md](../../../docs/contributing/TECHNICAL.md)

## Specifics

- **SvnCommands** uses a clap sub-enum (like GoCommands/DotnetCommands) with 8 named variants + `Other` catch-all for passthrough
- `--xml` mode injected for `svn log` and `svn info` to get locale-independent structured output; parsed with string-based XML extraction (svn's XML format is stable enough that `quick-xml` isn't needed)
- `svn diff` adds `--no-diff-deleted` to suppress deleted file diffs (common noise)
- User-provided `--xml` is stripped when rtk already adds it (avoids duplicate flag errors)
- All `run_*` functions use `runner::run_filtered` with `tee` label for recovery

## Cross-command

- Standalone module — no cross-ecosystem dependencies
- `run_other()` delegates to `runner::run_passthrough("svn", args)` for any unhandled subcommand

## Real-world quirks handled

- svn XML often splits attributes across lines (`<logentry\n   revision="3">`) — parser handles this
- `svn blame` shows `-` for uncommitted lines (not a number) — regex accounts for this
- `svn update` may output "At revision N." instead of "Updated to revision N." — regex handles both forms
- Certificate errors cause svn to exit with partial XML — filter falls back to raw output
