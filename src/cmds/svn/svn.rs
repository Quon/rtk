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
    /// First column (repository status): space, M, A, D, R, C, X, ?, !, ~
    /// Second column (working copy status): space, M, C, D, R, X, ?, !, ~
    /// Leading space in column 1 means "no repository modification".
    static ref STATUS_LINE_RE: Regex =
        Regex::new(r"^([ MADRCX?!~])([ MADCRX?!~])\s+(.+)$").unwrap();

    /// Matches Index: header in svn diff output
    static ref INDEX_HEADER_RE: Regex = Regex::new(r"^Index:.*$").unwrap();

    /// Matches === separator lines in svn diff
    static ref SEPARATOR_RE: Regex = Regex::new(r"^={67,}$").unwrap();

    /// Matches revision line in svn diff: --- a/file    (revision X)
    static ref DIFF_MINUS_RE: Regex = Regex::new(r"^---\s.+\s+\(.*\)$").unwrap();

    /// Matches revision line in svn diff: +++ b/file    (working copy)
    static ref DIFF_PLUS_RE: Regex = Regex::new(r"^\+\+\+\s.+\s+\(.*\)$").unwrap();

    /// Matches blame/annotate lines: whitespace, rev/dash, whitespace, user/dash, whitespace, content
    static ref BLAME_LINE_RE: Regex =
        Regex::new(r"^\s*(\d+|-)\s+(\S+|-)\s+(.+)$").unwrap();

    /// Matches add output: leading whitespace + status + path
    static ref ADD_LINE_RE: Regex = Regex::new(r"^[AaLL]\s+(.+)$").unwrap();

    /// Matches "Committed revision N." line
    static ref COMMITTED_RE: Regex = Regex::new(r"^Committed revision \d+\.$").unwrap();

    /// Matches update action lines: A, U, D, G, C, E plus whitespace and path
    static ref UPDATE_ACTION_RE: Regex = Regex::new(r"^([AUDGCE])\s+(.+)$").unwrap();

    /// Matches "At revision N." or "Updated to revision N." line
    static ref UPDATED_TO_RE: Regex = Regex::new(r"(?:Updated to|At) revision (\d+)").unwrap();

    /// Matches msg close tag for stripping
    static ref MSG_CLOSE_RE: Regex = Regex::new(r"</msg>\s*$").unwrap();
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

pub fn run_status(args: &[String], _verbose: u8) -> Result<i32> {
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

pub fn run_log(args: &[String], _verbose: u8) -> Result<i32> {
    // If user explicitly requested --xml, they want raw XML output — passthrough
    let has_xml = args.iter().any(|a| a == "--xml");
    if has_xml {
        let os_args: Vec<OsString> = std::iter::once("log".into())
            .chain(args.iter().map(|s| OsString::from(s.as_str())))
            .collect();
        return crate::core::runner::run_passthrough("svn", &os_args, _verbose);
    }

    let mut cmd = resolved_command("svn");
    cmd.args(["log", "--xml"]);
    cmd.args(args);

    runner::run_filtered(
        cmd,
        "svn log",
        &args.join(" "),
        filter_log,
        runner::RunOptions::stdout_only().tee("svn_log"),
    )
}

pub fn run_diff(args: &[String], _verbose: u8) -> Result<i32> {
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

pub fn run_info(args: &[String], _verbose: u8) -> Result<i32> {
    // If user explicitly requested --xml, they want raw XML output — passthrough
    let has_xml = args.iter().any(|a| a == "--xml");
    if has_xml {
        let os_args: Vec<OsString> = std::iter::once("info".into())
            .chain(args.iter().map(|s| OsString::from(s.as_str())))
            .collect();
        return crate::core::runner::run_passthrough("svn", &os_args, _verbose);
    }

    let mut cmd = resolved_command("svn");
    cmd.args(["info", "--xml"]);
    cmd.args(args);

    runner::run_filtered(
        cmd,
        "svn info",
        &args.join(" "),
        filter_info,
        runner::RunOptions::stdout_only().tee("svn_info"),
    )
}

pub fn run_blame(args: &[String], _verbose: u8) -> Result<i32> {
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

pub fn run_add(args: &[String], _verbose: u8) -> Result<i32> {
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

pub fn run_commit(args: &[String], _verbose: u8) -> Result<i32> {
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

pub fn run_update(args: &[String], _verbose: u8) -> Result<i32> {
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

pub fn run_other(args: &[OsString], _verbose: u8) -> Result<i32> {
    if args.is_empty() {
        anyhow::bail!("svn: no subcommand specified");
    }
    crate::core::runner::run_passthrough("svn", args, _verbose)
}

// ---------------------------------------------------------------------------
// Filter functions
// ---------------------------------------------------------------------------

/// Condense `svn status` output: normalize whitespace, strip leading spaces.
/// Lines without a recognized status column are silently dropped.
fn filter_status(input: &str) -> String {
    let mut output = String::new();
    for line in input.lines() {
        if let Some(caps) = STATUS_LINE_RE.captures(line) {
            let first = caps[1].trim();
            let second = caps[2].trim();
            let path = &caps[3];
            let status = if first.is_empty() {
                second.to_string()
            } else if second.is_empty() {
                first.to_string()
            } else {
                format!("{}{}", first, second)
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
    let mut msg_lines: Vec<String> = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<logentry") {
            in_entry = true;
            rev.clear();
            author.clear();
            date.clear();
            msg_lines.clear();
            // Extract revision attribute (may be on this line or next)
            if let Some(start) = trimmed.find("revision=\"") {
                let rest = &trimmed[start + 10..];
                if let Some(end) = rest.find('"') {
                    rev = format!("r{}", &rest[..end]);
                }
            }
        } else if in_entry {
            if rev.is_empty() && trimmed.starts_with("revision=\"") {
                // revision attribute on its own line (real svn XML formatting)
                if let Some(end) = trimmed[10..].find('"') {
                    rev = format!("r{}", &trimmed[10..10 + end]);
                }
            } else if trimmed.starts_with("<author>") {
                author = extract_xml_text(trimmed, "author").unwrap_or_default();
            } else if trimmed.starts_with("<date>") {
                let content = trimmed.trim_start_matches("<date>").trim_end_matches("</date>");
                // svn dates: 2024-01-15T02:30:00.000000Z -> 2024-01-15
                date = content.chars().take(10).collect();
            } else if trimmed == "</logentry>" {
                in_entry = false;
                let first_line = msg_lines.first().map(|s| s.as_str()).unwrap_or("");
                if !rev.is_empty() {
                    output.push_str(&format!("{} {} {} {}\n", rev, author, date, first_line));
                }
            } else if trimmed.starts_with("<msg") {
                // Try to extract inline content: <msg>text</msg>
                if let Some(content) = extract_xml_text(trimmed, "msg") {
                    msg_lines.push(content);
                } else if let Some(after) = trimmed.find('>') {
                    // <msg>content without closing tag on same line
                    let text = MSG_CLOSE_RE.replace(&trimmed[after + 1..], "");
                    msg_lines.push(text.trim().to_string());
                }
                // Otherwise content comes on following lines until </msg>
                // Keep in_entry=true so we capture body lines
            } else if trimmed == "</msg>" {
                // Content already captured in non-XML lines; nothing to do
            } else if !trimmed.starts_with('<') {
                // Capture message body, stripping trailing </msg> if present
                let text = MSG_CLOSE_RE.replace(trimmed, "");
                let cleaned = text.trim().to_string();
                if !cleaned.is_empty() {
                    msg_lines.push(cleaned);
                }
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
            // Strip revision suffix: "  (revision X)" or similar
            let trimmed = line.trim_start_matches("--- ");
            // Split on "  (" to get filename, then trim trailing space
            let plain = trimmed.split("  (").next().unwrap_or(trimmed).trim_end();
            output.push_str(&format!("--- {}\n", plain));
        } else if DIFF_PLUS_RE.is_match(line) {
            let trimmed = line.trim_start_matches("+++ ");
            let plain = trimmed.split("  (").next().unwrap_or(trimmed).trim_end();
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
    let mut in_commit = false;

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
            in_commit = false;
        } else if in_entry && revision.is_empty() && trimmed.starts_with("revision=\"") {
            // revision attribute on its own line
            if let Some(end) = trimmed[10..].find('"') {
                revision = trimmed[10..10 + end].to_string();
            }
        } else if trimmed.starts_with("<commit") {
            in_commit = true;
            // revision attribute: same line or next
            if let Some(start) = trimmed.find("revision=\"") {
                let rest = &trimmed[start + 10..];
                if let Some(end) = rest.find('"') {
                    last_changed_rev = rest[..end].to_string();
                }
            }
        } else if in_commit && last_changed_rev.is_empty() && trimmed.starts_with("revision=\"") {
            if let Some(end) = trimmed[10..].find('"') {
                last_changed_rev = trimmed[10..10 + end].to_string();
            }
        } else if trimmed == "</commit>" {
            in_commit = false;
        } else if in_commit {
            // author, date are inside <commit> (which is inside <entry>)
            if let Some(val) = extract_xml_text(trimmed, "author") {
                last_changed_author = val;
            }
            if let Some(val) = extract_xml_text(trimmed, "date") {
                last_changed_date = val.chars().take(10).collect();
            }
        } else if in_entry {
            if let Some(val) = extract_xml_text(trimmed, "url") {
                url = val;
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
            let user = &caps[2];
            let content = &caps[3];
            output.push_str(&format!("{} {} {}\n", rev, user, content));
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

/// Compact `svn add`: `+ {status} {path}`
fn filter_add(input: &str) -> String {
    let mut output = String::new();
    for line in input.lines() {
        if ADD_LINE_RE.is_match(line) {
            let trimmed = line.trim_start();
            let status = &trimmed[..1];
            let path = trimmed[1..].trim();
            output.push_str(&format!("{}  {}\n", status, path));
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
fn extract_xml_text(line: &str, element: &str) -> Option<String> {
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
        assert_eq!(output, "M src/main.rs\nM src/lib.rs\n? newfile.txt\nA added.rs\nD deleted.rs\n");
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
        assert!(savings >= 0.0, "status savings: expected >=0%, got {:.1}%", savings);
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
        assert_eq!(output, "r12345 user 2024-01-15 Fix bug in parser\nr12344 dev2 2024-01-14 Added new feature\n");
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
        assert!(savings >= 10.0, "log savings: expected >=10%, got {:.1}%", savings);
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
        assert_eq!(output, "--- src/main.rs\n+++ src/main.rs\n@@ -1,3 +1,4 @@\n line1\n+new line\n line2\n");
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
        assert!(savings >= 30.0, "diff savings: expected >=30%, got {:.1}%", savings);
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
        assert_eq!(output, "URL:      https://svn.example.com/svn/project/trunk\nRevision: 12345\nLast:     12344 user 2024-01-15\n");
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
        assert!(savings >= 0.0, "info savings: expected >=0%, got {:.1}%", savings);
    }

    // -- blame --

    #[test]
    fn test_filter_blame() {
        let input = r"  12345    user    line of code here
  12346    user2   another line
";
        let output = filter_blame(input);
        assert_eq!(output, "12345 user line of code here\n12346 user2 another line\n");
    }

    #[test]
    fn test_blame_token_savings() {
        let input = r"  12345    user    fn main() {
  12346    user2     println!();
";
        let output = filter_blame(input);
        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(input) as f64 * 100.0);
        assert!(savings >= 0.0, "blame savings: expected >=0%, got {:.1}%", savings);
    }

    // -- add --

    #[test]
    fn test_filter_add() {
        let input = r"A         src/newfile.rs
A         src/utils/helper.rs
";
        let output = filter_add(input);
        assert_eq!(output, "A  src/newfile.rs\nA  src/utils/helper.rs\n");
    }

    #[test]
    fn test_add_token_savings() {
        let input = r"A         src/newfile.rs
A         src/utils/helper.rs
";
        let output = filter_add(input);
        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(input) as f64 * 100.0);
        assert!(savings >= 0.0, "add savings: expected >=0%, got {:.1}%", savings);
    }

    // -- commit --

    #[test]
    fn test_filter_commit() {
        let input = r"Adding         src/main.rs
Transmitting file data .done
Committed revision 12345.
";
        let output = filter_commit(input);
        assert_eq!(output, "Committed revision 12345.\n");
    }

    #[test]
    fn test_commit_token_savings() {
        let input = r"Adding         src/main.rs
Transmitting file data .done
Committed revision 12345.
";
        let output = filter_commit(input);
        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(input) as f64 * 100.0);
        assert!(savings >= 60.0, "commit savings: expected >=60%, got {:.1}%", savings);
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
        assert_eq!(output, "A  src/newfile.rs\nU  src/existing.rs\nUpdated to r12345\n");
    }

    #[test]
    fn test_update_token_savings() {
        let input = r"A    src/newfile.rs
U    src/existing.rs
Updated to revision 12345.
";
        let output = filter_update(input);
        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(input) as f64 * 100.0);
        assert!(savings >= 0.0, "update savings: expected >=0%, got {:.1}%", savings);
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
        assert_eq!(output, "12345 user fn foo() {}\n");
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
}
