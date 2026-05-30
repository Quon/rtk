# Subversion (svn) Module Design

## Overview

Add a `svn` command filter module to RTK, following the git module architecture pattern. The module routes 8 named subcommands (`status`, `log`, `diff`, `info`, `blame`, `add`, `commit`, `update`) plus catch-all passthrough for others. Subversion's `--xml` output is used for complex subcommands (`log`, `info`) to ensure stable parsing across svn versions; simpler subcommands use regex-based text filtering.

## File Structure

```
src/cmds/svn/
  mod.rs       -- automod::dir!(pub "src/cmds/svn");
  svn.rs       -- SvnCommand enum + run() + filter functions
```

## Design Decisions

### Clap enum routing (Git mode, not Gh mode)

Using a clap-derived `SvnCommands` enum with named variants for each handled subcommand and an `Other(Vec<OsString>)` catch-all. This gives automatic `--help` generation per subcommand and clear dispatch.

### XML parsing for complex output

`svn log` and `svn info` use `--xml` mode + `quick-xml` parsing (already a dependency). This avoids brittleness from locale-dependent or version-dependent text format changes.

### Text regex for simpler output

`svn status`, `svn diff`, `svn blame`, `svn add`, `svn commit`, `svn update` use `lazy_static!` regex + line filtering. These commands have stable, simple text output that doesn't justify XML parsing overhead.

## Routing

### Changes to `src/main.rs`

**Commands enum — add Svn variant:**

```rust
Commands::Svn {
    #[arg(long)]
    non_interactive: bool,
    #[arg(long)]
    trust_server_cert: bool,
    #[arg(long)]
    username: Option<String>,
    #[arg(long)]
    password: Option<String>,
    command: SvnCommands,
}
```

**SvnCommands enum:**

```rust
#[derive(Debug, Clone, clap::Parser)]
enum SvnCommands {
    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Status { args: Vec<String> },
    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Log { args: Vec<String> },
    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Diff { args: Vec<String> },
    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Info { args: Vec<String> },
    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Blame { args: Vec<String> },
    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Add { args: Vec<String> },
    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Commit { args: Vec<String> },
    #[command(alias = "up", trailing_var_arg = true, allow_hyphen_values = true)]
    Update { args: Vec<String> },
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}
```

**Dispatch match arm:**

```rust
Commands::Svn { command, .. } => {
    svn::run(command, cli.verbose)?
}
```

### Changes to `src/cmds/mod.rs`

Insert `pub mod svn;` (alphabetically before `system`: s-v-n < s-y-s).

### Changes to `is_operational_command()`

Add `Commands::Svn { .. }` to the operational commands list.

## Filter Behavior per Subcommand

### svn status (regex, ~60% savings)

Strip extra whitespace, keep only status column + path. Only lines matching status patterns (`[M A D ? ! C ~ X ]` etc.) are kept.

```
Input:   M       src/main.rs
Output:  M  src/main.rs
```

Key regexes:
- `STATUS_LINE_RE` — match a valid status line
- Strip prefix space, normalize inter-column whitespace to 2 spaces

### svn log (xml parse, ~80% savings)

Use `svn log --xml`, parse `<logentry revision="...">` nodes via `quick-xml`. Output one line per entry:

```
r12345 user 2024-01-15 Fix bug in parser
r12344 user 2024-01-14 Added new feature
```

Filter content:
- revision number
- author
- date (YYYY-MM-DD)
- first line of log message only

### svn diff (regex, ~70% savings)

Similar to git diff filtering. Strip Index/=== header noise, keep filenames and actual diff hunks.

```
Input:  Index: src/main.rs
        ===================================================================
        --- src/main.rs    (revision 12345)
        +++ src/main.rs    (working copy)
        @@ -1,3 +1,4 @@
         line1
        +new line

Output: --- src/main.rs
        +++ src/main.rs
        @@ -1,3 +1,4 @@
         line1
        +new line
```

### svn info (xml parse, ~80% savings)

Use `svn info --xml`, extract URL, Revision, Last Changed Rev/Author/Date:

```
URL:      https://svn.example.com/project/trunk
Revision: 12345
Last:     12344 user 2024-01-15
```

### svn blame (regex, ~50% savings)

Blame is already compact. Strip leading whitespace, normalize separator.

```
Input:    12345    user    line of code here
Output:  12345  user  line of code here
```

### svn add (regex, ~50% savings)

Strip leading whitespace, remove `(bin)` or other annotations, compact:

```
Input:  A         src/newfile.rs
Output: + src/newfile.rs
```

### svn commit (regex, ~70% savings)

Only show "Committed revision N." line:

```
Input:  Adding         src/main.rs
        Transmitting file data .done
        Committed revision 12345.

Output: Committed revision 12345.
```

### svn update (regex, ~70% savings)

Show file action lines + summary, compact prefix:

```
Input:  Updating '.':
        A    src/newfile.rs
        U    src/existing.rs
        Updated to revision 12345.

Output: A  src/newfile.rs
        U  src/existing.rs
        Updated to r12345
```

## Execution Pattern (all filtered commands)

Following the RTK filter pattern from `runner.rs`:

```rust
pub fn run(cmd: SvnCommands, verbose: u8) -> Result<i32> {
    match cmd {
        SvnCommands::Status { args } => run_status(&args, verbose),
        SvnCommands::Log { args } => run_log(&args, verbose),
        // ...
        SvnCommands::Other(args) => run_passthrough(&args, verbose),
    }
}

fn run_status(args: &[String], verbose: u8) -> Result<i32> {
    let timer = tracking::TimedExecution::start();
    let mut cmd = resolved_command("svn");
    cmd.args(["status"]);
    cmd.args(args);

    runner::run_filtered(cmd, "svn", "status", |stdout| {
        filter_status(stdout)
    }, RunOptions::stdout_only())?;

    Ok(())
}
```

For log/info (XML parsing):

```rust
fn run_log(args: &[String], verbose: u8) -> Result<i32> {
    let timer = tracking::TimedExecution::start();
    let mut cmd = resolved_command("svn");
    cmd.args(["log", "--xml"]);
    cmd.args(args);

    runner::run_filtered(cmd, "svn", "log", |stdout| {
        filter_log_xml(stdout)
    }, RunOptions::stdout_only())?;

    Ok(())
}
```

For passthrough:

```rust
fn run_passthrough(args: &[OsString], verbose: u8) -> Result<i32> {
    let mut svn_args: Vec<OsString> = Vec::new();
    svn_args.extend(args.iter().cloned());
    runner::run_passthrough("svn", &svn_args, verbose)
}
```

## Token Savings Expectations

| Subcommand | Method | Expected Savings |
|-----------|--------|-----------------|
| status | regex text | ~60% |
| log | xml parse | ~80% |
| diff | regex text | ~70% |
| info | xml parse | ~80% |
| blame | regex text | ~50% |
| add | regex text | ~50% |
| commit | regex text | ~70% |
| update | regex text | ~70% |

## Files to Modify

| File | Change |
|------|--------|
| `src/cmds/svn/mod.rs` | Create: `automod::dir!(pub "src/cmds/svn");` |
| `src/cmds/svn/svn.rs` | Create: SvnCommand enum + run() + all filter functions |
| `src/cmds/mod.rs` | Add: `pub mod svn;` |
| `src/main.rs` | Add: `Commands::Svn` variant in Commands enum + dispatch + `is_operational_command()` |
| `tests/fixtures/` | Create real svn command output fixtures |
| `.github/workflows/` | (Optional) Add svn install step if CI doesn't have it |

## Testing

- **Snapshot tests** per subcommand using real `svn` output fixtures
- **Token accuracy tests** verifying ≥50-80% savings per subcommand
- **Edge cases**: empty svn output, non-svn directory, xml parse failures (fallback to pass-through)
