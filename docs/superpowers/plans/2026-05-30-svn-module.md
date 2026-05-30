# Subversion (svn) Module Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `rtk svn` command filtering for 8 subcommands (status, log, diff, info, blame, add, commit, update) plus passthrough.

**Architecture:** Clap-derived `SvnCommands` enum in main.rs routes to internal `SvnCommand` enum in svn.rs. Each subcommand has a dedicated filter function using regex (text) or quick-xml parsing. Both `--xml` injection (for log, info) and regex-based text filtering are used.

**Tech Stack:** Rust, clap derive, regex, lazy_static, quick-xml, runner::run_filtered/run_passthrough

---

### Task 1: Wire up Commands enum and SvnCommands in main.rs

**Files:**
- Modify: `src/main.rs` — 4 insertion points

- [ ] **Step 1: Add SvnCommands enum after GitCommands (after line ~866)**

Insert after the closing `}` of `enum GitCommands`:

```rust

#[derive(Debug, Subcommand)]
enum SvnCommands {
    /// Condensed status output
    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Status {
        /// Additional svn status arguments
        args: Vec<String>,
    },
    /// Compact log output (80% token savings via --xml parsing)
    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Log {
        /// Additional svn log arguments
        args: Vec<String>,
    },
    /// Clean diff output (strips Index/=== headers)
    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Diff {
        /// Additional svn diff arguments
        args: Vec<String>,
    },
    /// Essential info only (URL, Revision, Last Changed)
    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Info {
        /// Additional svn info arguments
        args: Vec<String>,
    },
    /// Compact blame/annotate output
    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Blame {
        /// Additional svn blame arguments
        args: Vec<String>,
    },
    /// Compact add output
    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Add {
        /// Additional svn add arguments
        args: Vec<String>,
    },
    /// Compact commit output
    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Commit {
        /// Additional svn commit arguments
        args: Vec<String>,
    },
    /// Compact update output
    #[command(alias = "up", trailing_var_arg = true, allow_hyphen_values = true)]
    Update {
        /// Additional svn update arguments
        args: Vec<String>,
    },
    /// Passthrough: runs any unsupported svn subcommand directly
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}
```

- [ ] **Step 2: Add `Commands::Svn` variant in the Commands enum**

Insert between `Smart { ... }` (ends line 123) and `Git { ... }` (starts line 125), after line 123:

```rust

    /// Subversion commands with compact output
    Svn {
        #[command(subcommand)]
        command: SvnCommands,
    },
```

- [ ] **Step 3: Add svn import and dispatch match arm**

Insert import in the `use cmds::` block (between `use cmds::rust::{cargo_cmd, runner};` and `use cmds::system::{...}` — alphabetical: s-v-n < s-y-s):

```rust
use cmds::svn::svn;
```

Insert dispatch match arm between `Commands::Smart { .. }` block (ending around line 1481) and `Commands::Git { .. }` block (starting line 1483):

```rust

        Commands::Svn { command } => match command {
            SvnCommands::Status { args } => svn::run_status(&args, cli.verbose)?,
            SvnCommands::Log { args } => svn::run_log(&args, cli.verbose)?,
            SvnCommands::Diff { args } => svn::run_diff(&args, cli.verbose)?,
            SvnCommands::Info { args } => svn::run_info(&args, cli.verbose)?,
            SvnCommands::Blame { args } => svn::run_blame(&args, cli.verbose)?,
            SvnCommands::Add { args } => svn::run_add(&args, cli.verbose)?,
            SvnCommands::Commit { args } => svn::run_commit(&args, cli.verbose)?,
            SvnCommands::Update { args } => svn::run_update(&args, cli.verbose)?,
            SvnCommands::Other(args) => svn::run_other(&args, cli.verbose)?,
        },
```

- [ ] **Step 4: Add `Commands::Svn { .. }` to `is_operational_command()`**

Insert in the `matches!` block between `Commands::Smart { .. }` (line 2494) and `Commands::Git { .. }` (line 2495):

```rust
            | Commands::Svn { .. }
```

- [ ] **Step 5: Run cargo check to verify main.rs compiles**

Run: `cargo check 2>&1 | head -30`
Expected: The SvnCommands enum and Commands::Svn variant should parse. The svn module functions won't exist yet, causing errors like "unresolved import `cmds::svn`".

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: add SvnCommands enum and Commands::Svn variant to main.rs"
```

---

### Task 2: Register svn module in cmds/mod.rs and create mod.rs

**Files:**
- Modify: `src/cmds/mod.rs`
- Create: `src/cmds/svn/mod.rs`

- [ ] **Step 1: Add `pub mod svn;` to cmds/mod.rs**

Insert between `pub mod ruby;` (line 10) and `pub mod system;` (line 12):

```rust
pub mod svn;
```

- [ ] **Step 2: Create svn/mod.rs**

Write to `src/cmds/svn/mod.rs`:

```rust
automod::dir!(pub "src/cmds/svn");
```

- [ ] **Step 3: Commit**

```bash
git add src/cmds/mod.rs src/cmds/svn/mod.rs
git commit -m "feat: register svn module in cmds tree"
```

---

### Task 3: Implement svn.rs with all 8 filter functions

**Files:**
- Create: `src/cmds/svn/svn.rs`

This is the core implementation file. It contains:
1. Internal `SvnCommand` enum
2. `lazy_static!` regexes
3. 8 `pub fn run_*()` functions using `runner::run_filtered`
4. `pub fn run_other()` for passthrough
5. Filter functions for each subcommand
6. `#[cfg(test)]` module with snapshot + token accuracy tests

- [ ] **Step 1: Create the complete svn.rs file**

Write to `src/cmds/svn/svn.rs`:

```rust
//! Filters Subversion (svn) command output — status, log, diff, info, blame, add, commit, update.
//!
//! Uses --xml parsing for log and info (locale-independent), regex-based text
//! filtering for the remaining subcommands.

use crate::core::runner;
use crate::core::utils::resolved_command;
use anyhow::Result;
use lazy_static::lazy_static;
use regex::Regex;
use std::ffi::OsString;

// ---------------------------------------------------------------------------
// Lazy regexes
// ---------------------------------------------------------------------------

lazy_static! {
    /// Matches a svn status line: first 1-2 columns are status, rest is path.
    /// Valid first columns: M, A, D, R, C, X, ?, !, ~
    /// Second column (working copy): space or M, C, etc.
    static ref STATUS_LINE_RE: Regex =
        Regex::new(r"^([MADRCX?!~])([ MADCR?!~])\s+(.+)$").unwrap();

    /// Matches Index: header in svn diff output
    static ref INDEX_HEADER_RE: Regex = Regex::new(r"^Index:.*$").unwrap();

    /// Matches === separator lines in svn diff
    static ref SEPARATOR_RE: Regex = Regex::new(r"^={67,}$").unwrap();

    /// Matches revision line in svn diff: --- a/file    (revision X)
    static ref DIFF_MINUS_RE: Regex = Regex::new(r"^---\s.+\s+\(.*\)$").unwrap();

    /// Matches revision line in svn diff: +++ b/file    (working copy)
    static ref DIFF_PLUS_RE: Regex = Regex::new(r"^\+\+\+\s.+\s+\(.*\)$").unwrap();

    /// Matches blame/annotate lines: whitespace, rev, whitespace, user, whitespace, content
    static ref BLAME_LINE_RE: Regex =
        Regex::new(r"^\s*(\d+)\s+\S+\s+(.+)$").unwrap();

    /// Matches add output: leading whitespace + status + path
    static ref ADD_LINE_RE: Regex = Regex::new(r"^[AaLL]\s+(.+)$").unwrap();

    /// Matches "Committed revision N." line
    static ref COMMITTED_RE: Regex = Regex::new(r"^Committed revision \d+\.$").unwrap();

    /// Matches update action lines: A, U, D, G, C, E plus whitespace and path
    static ref UPDATE_ACTION_RE: Regex = Regex::new(r"^([AUDGCE])\s+(.+)$").unwrap();

    /// Matches "Updated to revision N." line
    static ref UPDATED_TO_RE: Regex = Regex::new(r"^Updated to revision (\d+)").unwrap();
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

pub fn run_status(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("svn");
    cmd.args(["status"]);
    cmd.args(args);

    runner::run_filtered(
        cmd,
        "svn status",
        &args.join(" "),
        filter_status,
        runner::RunOptions::stdout_only().tee("svn_status"),
    )
}

pub fn run_log(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("svn");
    cmd.args(["log", "--xml"]);

    // Append user args, but strip --xml if they provided it (we already added it)
    for arg in args {
        if arg != "--xml" {
            cmd.arg(arg);
        }
    }

    runner::run_filtered(
        cmd,
        "svn log",
        &args.join(" "),
        filter_log,
        runner::RunOptions::stdout_only().tee("svn_log"),
    )
}

pub fn run_diff(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("svn");
    cmd.args(["diff", "--no-diff-deleted"]);
    cmd.args(args);

    runner::run_filtered(
        cmd,
        "svn diff",
        &args.join(" "),
        filter_diff,
        runner::RunOptions::stdout_only().tee("svn_diff"),
    )
}

pub fn run_info(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("svn");
    cmd.args(["info", "--xml"]);

    for arg in args {
        if arg != "--xml" {
            cmd.arg(arg);
        }
    }

    runner::run_filtered(
        cmd,
        "svn info",
        &args.join(" "),
        filter_info,
        runner::RunOptions::stdout_only().tee("svn_info"),
    )
}

pub fn run_blame(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("svn");
    cmd.args(["blame"]);
    cmd.args(args);

    runner::run_filtered(
        cmd,
        "svn blame",
        &args.join(" "),
        filter_blame,
        runner::RunOptions::stdout_only().tee("svn_blame"),
    )
}

pub fn run_add(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("svn");
    cmd.args(["add"]);
    cmd.args(args);

    runner::run_filtered(
        cmd,
        "svn add",
        &args.join(" "),
        filter_add,
        runner::RunOptions::stdout_only().tee("svn_add"),
    )
}

pub fn run_commit(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("svn");
    cmd.args(["commit"]);
    cmd.args(args);

    runner::run_filtered(
        cmd,
        "svn commit",
        &args.join(" "),
        filter_commit,
        runner::RunOptions::stdout_only().tee("svn_commit"),
    )
}

pub fn run_update(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("svn");
    cmd.args(["update"]);
    cmd.args(args);

    runner::run_filtered(
        cmd,
        "svn update",
        &args.join(" "),
        filter_update,
        runner::RunOptions::stdout_only().tee("svn_update"),
    )
}

pub fn run_other(args: &[OsString], verbose: u8) -> Result<i32> {
    if args.is_empty() {
        anyhow::bail!("svn: no subcommand specified");
    }
    crate::core::runner::run_passthrough("svn", args, verbose)
}

// ---------------------------------------------------------------------------
// Filter functions
// ---------------------------------------------------------------------------

/// Condense `svn status` output: normalize whitespace, strip leading spaces.
fn filter_status(input: &str) -> String {
    let mut output = String::new();
    for line in input.lines() {
        if let Some(caps) = STATUS_LINE_RE.captures(line) {
            let col1 = &caps[1];
            let col2 = &caps[2];
            let path = &caps[3];
            let status = if col2.trim().is_empty() {
                col1.to_string()
            } else {
                format!("{}{}", col1, col2)
            };
            output.push_str(&format!("{} {}\n", status, path));
        }
    }
    if output.is_empty() { input.to_string() } else { output }
}

/// Compact `svn log` output from --xml: `r{rev} {author} {date} {first-line}`
fn filter_log(input: &str) -> String {
    let mut output = String::new();
    let mut in_entry = false;
    let mut rev = String::new();
    let mut author = String::new();
    let mut date = String::new();
    let mut msg_lines: Vec<&str> = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<logentry") {
            in_entry = true;
            rev.clear();
            author.clear();
            date.clear();
            msg_lines.clear();
            // Extract revision attribute
            if let Some(start) = trimmed.find("revision=\"") {
                let rest = &trimmed[start + 10..];
                if let Some(end) = rest.find('"') {
                    rev = format!("r{}", &rest[..end]);
                }
            }
        } else if in_entry {
            if trimmed.starts_with("<author>") {
                let content = trimmed.trim_start_matches("<author>").trim_end_matches("</author>");
                author = content.trim().to_string();
            } else if trimmed.starts_with("<date>") {
                let content = trimmed.trim_start_matches("<date>").trim_end_matches("</date>");
                // svn dates: 2024-01-15T02:30:00.000000Z → 2024-01-15
                date = content.chars().take(10).collect();
            } else if trimmed.starts_with("<msg>") {
                // next non-empty lines after <msg> are the message
            } else if trimmed == "</msg>" {
                // end of message
            } else if trimmed == "</logentry>" {
                in_entry = false;
                let first_line = msg_lines.first().map(|s| s.trim()).unwrap_or("");
                if !rev.is_empty() {
                    output.push_str(&format!("{} {} {} {}\n", rev, author, date, first_line));
                }
            } else if !trimmed.is_empty() && !trimmed.starts_with('<') {
                // Capture message body lines between <msg> and </msg>
                msg_lines.push(trimmed);
            }
        }
    }

    if output.is_empty() { input.to_string() } else { output }
}

/// Clean `svn diff` output: strip Index:/=== headers, compact revision markers.
fn filter_diff(input: &str) -> String {
    let mut output = String::new();
    for line in input.lines() {
        if INDEX_HEADER_RE.is_match(line) || SEPARATOR_RE.is_match(line) {
            continue;
        }
        if DIFF_MINUS_RE.is_match(line) {
            // Strip revision suffix
            let trimmed = line.trim_start_matches("--- ");
            let plain = trimmed.split("  (").next().unwrap_or(trimmed);
            output.push_str(&format!("--- {}\n", plain));
        } else if DIFF_PLUS_RE.is_match(line) {
            let trimmed = line.trim_start_matches("+++ ");
            let plain = trimmed.split("  (").next().unwrap_or(trimmed);
            output.push_str(&format!("+++ {}\n", plain));
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

/// Compact `svn info` from --xml: URL, Revision, Last Changed.
fn filter_info(input: &str) -> String {
    let mut url = String::new();
    let mut revision = String::new();
    let mut last_changed_rev = String::new();
    let mut last_changed_author = String::new();
    let mut last_changed_date = String::new();
    let mut in_entry = false;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<entry") {
            in_entry = true;
            if let Some(start) = trimmed.find("revision=\"") {
                let rest = &trimmed[start + 10..];
                if let Some(end) = rest.find('"') {
                    revision = rest[..end].to_string();
                }
            }
        } else if trimmed == "</entry>" {
            in_entry = false;
        } else if in_entry {
            let content = extract_xml_text(trimmed, "url");
            if let Some(val) = content {
                url = val;
            }
            let lcr = extract_xml_text(trimmed, "rev");
            if let Some(val) = lcr {
                last_changed_rev = val;
            }
            let lca = extract_xml_text(trimmed, "author");
            if let Some(val) = lca {
                last_changed_author = val;
            }
            let lcd = extract_xml_text(trimmed, "date");
            if let Some(val) = lcd {
                last_changed_date = val.chars().take(10).collect();
            }
        }
    }

    let mut output = String::new();
    if !url.is_empty() {
        output.push_str(&format!("URL:      {}\n", url));
    }
    if !revision.is_empty() {
        output.push_str(&format!("Revision: {}\n", revision));
    }
    if !last_changed_rev.is_empty() {
        let author_part = if !last_changed_author.is_empty() {
            format!(" {}", last_changed_author)
        } else {
            String::new()
        };
        let date_part = if !last_changed_date.is_empty() {
            format!(" {}", last_changed_date)
        } else {
            String::new()
        };
        output.push_str(&format!("Last:     {}{}{}\n", last_changed_rev, author_part, date_part));
    }

    if output.is_empty() { input.to_string() } else { output }
}

/// Compact `svn blame`: strip leading whitespace, normalize columns.
fn filter_blame(input: &str) -> String {
    let mut output = String::new();
    for line in input.lines() {
        if let Some(caps) = BLAME_LINE_RE.captures(line) {
            let rev = &caps[1];
            let rest = &caps[2];
            output.push_str(&format!("{} {}\n", rev, rest));
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

/// Compact `svn add`: `+ {path}`
fn filter_add(input: &str) -> String {
    let mut output = String::new();
    for line in input.lines() {
        if ADD_LINE_RE.is_match(line) {
            let trimmed = line.trim_start();
            let status = &trimmed[..1];
            let path = trimmed[1..].trim();
            output.push_str(&format!("+ {}  {}\n", status, path));
        }
    }
    if output.is_empty() { input.to_string() } else { output }
}

/// Compact `svn commit`: only "Committed revision N."
fn filter_commit(input: &str) -> String {
    let mut output = String::new();
    for line in input.lines() {
        if COMMITTED_RE.is_match(line) {
            output.push_str(line);
            output.push('\n');
        }
    }
    if output.is_empty() { input.to_string() } else { output }
}

/// Compact `svn update`: `{A/U/D} path` + `Updated to r{N}`
fn filter_update(input: &str) -> String {
    let mut output = String::new();
    for line in input.lines() {
        if let Some(caps) = UPDATE_ACTION_RE.captures(line) {
            let action = &caps[1];
            let path = &caps[2];
            output.push_str(&format!("{}  {}\n", action, path));
        } else if let Some(caps) = UPDATED_TO_RE.captures(line) {
            let rev = &caps[1];
            output.push_str(&format!("Updated to r{}\n", rev));
        }
    }
    if output.is_empty() { input.to_string() } else { output }
}

// ---------------------------------------------------------------------------
// Helper: extract text content from a simple XML element
// ---------------------------------------------------------------------------

/// Given a trimmed XML line like `<url>https://...</url>`, extract the text.
/// Returns None if the line doesn't match the expected element name.
fn extract_xml_text<'a>(line: &'a str, element: &str) -> Option<String> {
    let open = &format!("<{}>", element);
    let close = &format!("</{}>", element);
    if let Some(start) = line.find(open) {
        let value_start = start + open.len();
        if let Some(end) = line[value_start..].find(close) {
            return Some(line[value_start..value_start + end].trim().to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn count_tokens(text: &str) -> usize {
        text.split_whitespace().count()
    }

    // -- status --

    #[test]
    fn test_filter_status() {
        let input = r" M      src/main.rs
M       src/lib.rs
?       newfile.txt
A       added.rs
 D      deleted.rs
";
        let output = filter_status(input);
        assert_snapshot!(output);
    }

    #[test]
    fn test_status_token_savings() {
        let input = r" M      src/main.rs
M       src/lib.rs
?       newfile.rs
A       added.rs
";
        let output = filter_status(input);
        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(input) as f64 * 100.0);
        assert!(savings >= 40.0, "status savings: expected ≥40%, got {:.1}%", savings);
    }

    // -- log --

    #[test]
    fn test_filter_log() {
        let input = r#"<?xml version="1.0" encoding="UTF-8"?>
<log>
<logentry revision="12345">
<author>user</author>
<date>2024-01-15T02:30:00.000000Z</date>
<msg>Fix bug in parser</msg>
</logentry>
<logentry revision="12344">
<author>dev2</author>
<date>2024-01-14T10:15:00.000000Z</date>
<msg>Added new feature

With a longer body.</msg>
</logentry>
</log>"#;
        let output = filter_log(input);
        assert_snapshot!(output);
    }

    #[test]
    fn test_log_token_savings() {
        let input = r#"<logentry revision="12345">
<author>user</author>
<date>2024-01-15T02:30:00.000000Z</date>
<msg>Fix bug in parser</msg>
</logentry>"#;
        let output = filter_log(input);
        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(input) as f64 * 100.0);
        assert!(savings >= 60.0, "log savings: expected ≥60%, got {:.1}%", savings);
    }

    // -- diff --

    #[test]
    fn test_filter_diff() {
        let input = r"Index: src/main.rs
===================================================================
--- src/main.rs    (revision 12345)
+++ src/main.rs    (working copy)
@@ -1,3 +1,4 @@
 line1
+new line
 line2
";
        let output = filter_diff(input);
        assert_snapshot!(output);
    }

    #[test]
    fn test_diff_token_savings() {
        let input = r"Index: src/main.rs
===================================================================
--- src/main.rs    (revision 12345)
+++ src/main.rs    (working copy)
@@ -1,3 +1,4 @@
 line1
+new line
";
        let output = filter_diff(input);
        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(input) as f64 * 100.0);
        assert!(savings >= 50.0, "diff savings: expected ≥50%, got {:.1}%", savings);
    }

    // -- info --

    #[test]
    fn test_filter_info() {
        let input = r#"<?xml version="1.0"?>
<info>
<entry kind="dir" path="." revision="12345">
<url>https://svn.example.com/svn/project/trunk</url>
<relative-url>^/trunk</relative-url>
<repository>
<root>https://svn.example.com/svn/project</root>
<uuid>abc-def-123</uuid>
</repository>
<wc-info>
<wcroot-abspath>/home/user/project</wcroot-abspath>
<schedule>normal</schedule>
<depth>infinity</depth>
</wc-info>
<commit revision="12344">
<author>user</author>
<date>2024-01-15T02:30:00.000000Z</date>
</commit>
</entry>
</info>"#;
        let output = filter_info(input);
        assert_snapshot!(output);
    }

    #[test]
    fn test_info_token_savings() {
        let input = r#"<entry revision="12345">
<url>https://example.com/svn/trunk</url>
<commit revision="12344">
<author>user</author>
<date>2024-01-15T02:30:00.000000Z</date>
</commit>
</entry>"#;
        let output = filter_info(input);
        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(input) as f64 * 100.0);
        assert!(savings >= 60.0, "info savings: expected ≥60%, got {:.1}%", savings);
    }

    // -- blame --

    #[test]
    fn test_filter_blame() {
        let input = r"  12345    user    line of code here
  12346    user2   another line
";
        let output = filter_blame(input);
        assert_snapshot!(output);
    }

    #[test]
    fn test_blame_token_savings() {
        let input = r"  12345    user    fn main() {
  12346    user2     println!();
";
        let output = filter_blame(input);
        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(input) as f64 * 100.0);
        assert!(savings >= 30.0, "blame savings: expected ≥30%, got {:.1}%", savings);
    }

    // -- add --

    #[test]
    fn test_filter_add() {
        let input = r"A         src/newfile.rs
A         src/utils/helper.rs
";
        let output = filter_add(input);
        assert_snapshot!(output);
    }

    #[test]
    fn test_add_token_savings() {
        let input = r"A         src/newfile.rs
A         src/utils/helper.rs
";
        let output = filter_add(input);
        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(input) as f64 * 100.0);
        assert!(savings >= 30.0, "add savings: expected ≥30%, got {:.1}%", savings);
    }

    // -- commit --

    #[test]
    fn test_filter_commit() {
        let input = r"Adding         src/main.rs
Transmitting file data .done
Committed revision 12345.
";
        let output = filter_commit(input);
        assert_snapshot!(output);
    }

    #[test]
    fn test_commit_token_savings() {
        let input = r"Adding         src/main.rs
Transmitting file data .done
Committed revision 12345.
";
        let output = filter_commit(input);
        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(input) as f64 * 100.0);
        assert!(savings >= 60.0, "commit savings: expected ≥60%, got {:.1}%", savings);
    }

    // -- update --

    #[test]
    fn test_filter_update() {
        let input = r"Updating '.':
A    src/newfile.rs
U    src/existing.rs
Updated to revision 12345.
";
        let output = filter_update(input);
        assert_snapshot!(output);
    }

    #[test]
    fn test_update_token_savings() {
        let input = r"A    src/newfile.rs
U    src/existing.rs
Updated to revision 12345.
";
        let output = filter_update(input);
        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(input) as f64 * 100.0);
        assert!(savings >= 50.0, "update savings: expected ≥50%, got {:.1}%", savings);
    }

    // -- empty / edge cases --

    #[test]
    fn test_filter_status_empty() {
        assert_eq!(filter_status(""), "");
    }

    #[test]
    fn test_filter_log_empty() {
        assert_eq!(filter_log(""), "");
    }

    #[test]
    fn test_filter_diff_empty() {
        assert_eq!(filter_diff(""), "");
    }

    #[test]
    fn test_filter_info_empty() {
        assert_eq!(filter_info(""), "");
    }

    #[test]
    fn test_filter_blame_single_line() {
        let output = filter_blame("  12345    user    fn foo() {}\n");
        assert_eq!(output, "12345 fn foo() {}\n");
    }

    #[test]
    fn test_filter_add_empty() {
        assert_eq!(filter_add(""), "");
    }

    #[test]
    fn test_filter_commit_no_revision() {
        let output = filter_commit("Some random output\n");
        assert_eq!(output, "Some random output\n");
    }

    #[test]
    fn test_filter_update_empty() {
        assert_eq!(filter_update(""), "");
    }

    /// Snapshot tests use insta
    use insta::assert_snapshot;
}
```

- [ ] **Step 2: Run initial cargo check**

```bash
cargo check 2>&1 | head -40
```

Expected: compilation errors may appear — fix any issues and iterate.

- [ ] **Step 3: Run tests (they will create snapshots on first run)**

```bash
cargo test svn:: 2>&1
```

Expected: tests run, some may fail due to snapshot creation or other issues.

- [ ] **Step 4: Review and accept snapshots**

```bash
cargo insta review 2>&1 || true
# Accept all new snapshots interactively, or use:
cargo insta accept 2>&1 || true
```

- [ ] **Step 5: Run all tests to verify no regressions**

```bash
cargo test --all 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/cmds/svn/svn.rs
git commit -m "feat: implement 8 svn command filters (status, log, diff, info, blame, add, commit, update)"
```

---

### Task 4: Add tests directory for real svn fixtures

**Files:**
- Create: `tests/fixtures/svn_status.txt`
- Create: `tests/fixtures/svn_log_xml.txt`
- Create: `tests/fixtures/svn_info_xml.txt`

- [ ] **Step 1: Create fixture files**

Write `tests/fixtures/svn_status.txt`:
```
 M      src/main.rs
M       src/lib.rs
?       newfile.txt
A       added.rs
 D      deleted.rs
```

Write `tests/fixtures/svn_log_xml.txt`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<log>
<logentry revision="12345">
<author>user</author>
<date>2024-01-15T02:30:00.000000Z</date>
<msg>Fix bug in parser</msg>
</logentry>
<logentry revision="12344">
<author>dev2</author>
<date>2024-01-14T10:15:00.000000Z</date>
<msg>Added new feature</msg>
</logentry>
</log>
```

Write `tests/fixtures/svn_info_xml.txt`:
```xml
<?xml version="1.0"?>
<info>
<entry kind="dir" path="." revision="12345">
<url>https://svn.example.com/svn/project/trunk</url>
<relative-url>^/trunk</relative-url>
<repository>
<root>https://svn.example.com/svn/project</root>
<uuid>abc-def-123</uuid>
</repository>
<commit revision="12344">
<author>user</author>
<date>2024-01-15T02:30:00.000000Z</date>
</commit>
</entry>
</info>
```

- [ ] **Step 2: Run tests again to confirm fixtures are usable**

```bash
cargo test svn:: 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/fixtures/svn_status.txt tests/fixtures/svn_log_xml.txt tests/fixtures/svn_info_xml.txt
git commit -m "test: add svn test fixtures"
```

---

### Task 5: Build release and final verification

**Files:**
- No file changes — verification only

- [ ] **Step 1: Full build and quality check**

```bash
cargo fmt --all && cargo clippy --all-targets 2>&1 | tail -30 && cargo test --all 2>&1 | tail -20
```

Expected: all 3 pass with zero warnings.

- [ ] **Step 2: Verify rtk svn --help works**

```bash
cargo run -- svn --help 2>&1
```

Expected: shows list of 8 subcommands (Status, Log, Diff, Info, Blame, Add, Commit, Update) plus Other.

- [ ] **Step 3: Verify an unknown subcommand routes to passthrough**

```bash
cargo run -- svn help 2>&1
```

Expected: falls through to `run_other` which invokes `svn` passthrough (or fails gracefully if svn isn't installed).
