//! Git channels (`hermes:git:*`), backed by the `git` CLI.
//!
//! Shapes mirror `git-review-ops.ts` / `git-worktree-ops.ts` / `git-repo-scan.ts`:
//! reads fail soft (empty list / null) and mutations reject, exactly as the
//! Electron handlers do, so the renderer's existing error handling still works.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new(tool("git"))
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("PATH", tool_path())
        .output()
        .map_err(|e| format!("spawn git: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn repo_root(raw: &str) -> Option<PathBuf> {
    // Renderer-supplied paths carry `~`; git needs a real path.
    let path = crate::fs_ops::expand(raw);
    if !path.is_dir() {
        return None;
    }
    let out = git(&path, &["rev-parse", "--show-toplevel"]).ok()?;
    let root = PathBuf::from(out.trim());
    root.is_dir().then_some(root)
}

fn default_branch(repo: &Path) -> Option<String> {
    if let Ok(out) = git(repo, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]) {
        let name = out.trim().to_string();
        if !name.is_empty() {
            return Some(name.rsplit('/').next().unwrap_or(&name).to_string());
        }
    }
    for candidate in ["main", "master"] {
        if git(repo, &["rev-parse", "--verify", candidate]).is_ok() {
            return Some(candidate.to_string());
        }
    }
    git(repo, &["config", "init.defaultBranch"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn status_letter(code: &str) -> &'static str {
    if code.contains('D') {
        "D"
    } else if code.contains('A') || code == "??" {
        "A"
    } else if code.contains('R') {
        "R"
    } else {
        "M"
    }
}

struct StatusEntry {
    path: String,
    index: char,
    worktree: char,
}

fn parse_status(repo: &Path) -> Result<Vec<StatusEntry>, String> {
    // `normal` keeps an untracked directory as ONE row instead of walking it.
    let raw = git(repo, &["status", "--porcelain=v1", "-z", "--untracked-files=normal"])?;
    let mut entries = Vec::new();
    let mut iter = raw.split('\0').filter(|s| !s.is_empty());
    while let Some(item) = iter.next() {
        if item.len() < 4 {
            continue;
        }
        let index = item.chars().next().unwrap_or(' ');
        let worktree = item.chars().nth(1).unwrap_or(' ');
        let path = item[3..].to_string();
        // Renames carry the original path as the next NUL-separated field.
        if index == 'R' || worktree == 'R' {
            let _ = iter.next();
        }
        entries.push(StatusEntry {
            path,
            index,
            worktree,
        });
    }
    Ok(entries)
}

fn numstat(repo: &Path, args: &[&str]) -> (u64, u64) {
    // `git diff --numstat <args>` — the subcommand matters: without it git
    // rejects `--numstat` as an unknown option and the caller silently reads 0.
    let mut cmd_args = vec!["diff", "--numstat"];
    cmd_args.extend_from_slice(args);
    match git(repo, &cmd_args) {
        Ok(out) => out.lines().fold((0, 0), |(a, r), line| {
            let mut parts = line.split('\t');
            let added = parts.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            let removed = parts.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            (a + added, r + removed)
        }),
        Err(_) => (0, 0),
    }
}

fn count_lines(path: &Path) -> u64 {
    match std::fs::read(path) {
        Ok(bytes) if bytes.is_empty() => 0,
        Ok(bytes) => {
            let newlines = bytes.iter().filter(|b| **b == b'\n').count() as u64;
            // A trailing newline must not be counted as an extra empty line;
            // a final unterminated line still counts.
            if bytes.last() == Some(&b'\n') {
                newlines
            } else {
                newlines + 1
            }
        }
        Err(_) => 0,
    }
}

/// `git diff --no-index` exits 1 when the files differ — that is a successful
/// diff, not a failure, so its stdout is returned regardless of status.
fn git_diff_text(repo: &Path, args: &[&str]) -> String {
    match Command::new("git").arg("-C").arg(repo).args(args).output() {
        Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
        Err(_) => String::new(),
    }
}

/// `hermes:git:repoStatus` — the composer coding rail's compact status.
pub fn repo_status(raw: &str) -> Value {
    let Some(repo) = repo_root(raw) else {
        return Value::Null;
    };
    let entries = match parse_status(&repo) {
        Ok(entries) => entries,
        Err(_) => return Value::Null,
    };

    let head = git(&repo, &["status", "--porcelain=v1", "-b", "--untracked-files=no"]).ok();
    let mut branch: Option<String> = None;
    let mut detached = false;
    let mut ahead = 0u64;
    let mut behind = 0u64;
    if let Some(first) = head.as_deref().and_then(|s| s.lines().next()) {
        if let Some(rest) = first.strip_prefix("## ") {
            if rest.starts_with("HEAD") {
                detached = true;
            } else {
                let name = rest.split("...").next().unwrap_or(rest).trim();
                branch = Some(name.to_string());
            }
            if let Some(start) = rest.find("[ahead ") {
                ahead = rest[start + 7..]
                    .split(|c: char| !c.is_ascii_digit())
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            }
            if let Some(start) = rest.find("behind ") {
                behind = rest[start + 7..]
                    .split(|c: char| !c.is_ascii_digit())
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            }
        }
    }

    let files: Vec<Value> = entries
        .iter()
        .map(|entry| {
            let untracked = entry.index == '?' || entry.worktree == '?';
            let conflicted = entry.index == 'U' || entry.worktree == 'U';
            json!({
                "path": entry.path,
                "staged": entry.index != ' ' && entry.index != '?' && !conflicted,
                "unstaged": entry.worktree != ' ' && entry.worktree != '?' && !conflicted,
                "untracked": untracked,
                "conflicted": conflicted
            })
        })
        .collect();

    let staged = files.iter().filter(|f| f["staged"] == true).count();
    let unstaged = files.iter().filter(|f| f["unstaged"] == true).count();
    let untracked_count = files.iter().filter(|f| f["untracked"] == true).count();
    let conflicted = files.iter().filter(|f| f["conflicted"] == true).count();

    let (mut added, removed) = numstat(&repo, &["HEAD"]);
    // `git diff HEAD` ignores untracked files; fold their line counts in, the
    // same best-effort adjustment the Electron rail makes.
    for entry in entries.iter().filter(|e| e.index == '?') {
        let path = repo.join(&entry.path);
        if path.is_file() {
            added += count_lines(&path);
        }
    }

    let mut capped = files;
    capped.truncate(200);

    json!({
        "branch": if detached { Value::Null } else { branch.map(Value::String).unwrap_or(Value::Null) },
        "defaultBranch": default_branch(&repo).map(Value::String).unwrap_or(Value::Null),
        "detached": detached,
        "ahead": ahead,
        "behind": behind,
        "staged": staged,
        "unstaged": unstaged,
        "untracked": untracked_count,
        "conflicted": conflicted,
        "changed": capped.len(),
        "added": added,
        "removed": removed,
        "files": capped
    })
}

/// `hermes:git:branchList` / `baseBranchList`.
pub fn branch_list(raw: &str) -> Value {
    let Some(repo) = repo_root(raw) else {
        return json!([]);
    };
    let local = git(
        &repo,
        &["for-each-ref", "--format=%(refname:short)", "--sort=-committerdate", "refs/heads"],
    )
    .unwrap_or_default();
    let remote = git(
        &repo,
        &["for-each-ref", "--format=%(refname:short)", "--sort=-committerdate", "refs/remotes"],
    )
    .unwrap_or_default();

    let locals: Vec<String> = local
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let local_set: std::collections::HashSet<&str> = locals.iter().map(String::as_str).collect();
    let trunk = default_branch(&repo);

    let mut worktrees: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if let Value::Array(items) = worktree_list(raw) {
        for item in items {
            if let (Some(branch), Some(path)) = (item["branch"].as_str(), item["path"].as_str()) {
                worktrees.insert(branch.to_string(), path.to_string());
            }
        }
    }

    let mut rows: Vec<Value> = locals
        .iter()
        .map(|name| {
            json!({
                "name": name,
                "checkedOut": worktrees.contains_key(name),
                "isDefault": trunk.as_deref() == Some(name.as_str()),
                "isRemote": false,
                "worktreePath": worktrees.get(name).cloned()
            })
        })
        .collect();

    for name in remote.lines().map(str::trim).filter(|s| !s.is_empty()) {
        if name.ends_with("/HEAD") {
            continue;
        }
        let short = name.split_once('/').map(|(_, rest)| rest).unwrap_or(name);
        if local_set.contains(short) {
            continue;
        }
        rows.push(json!({
            "name": name,
            "checkedOut": false,
            "isDefault": false,
            "isRemote": true,
            "worktreePath": Value::Null
        }));
    }
    Value::Array(rows)
}

pub fn branch_switch(raw: &str, branch: &str) -> Result<Value, String> {
    let repo = repo_root(raw).ok_or_else(|| "Not a git repository".to_string())?;
    let name = branch.trim();
    if name.is_empty() {
        return Ok(json!({ "branch": Value::Null }));
    }
    git(&repo, &["checkout", name])?;
    Ok(json!({ "branch": name }))
}

pub fn worktree_list(raw: &str) -> Value {
    let Some(repo) = repo_root(raw) else {
        return json!([]);
    };
    let out = git(&repo, &["worktree", "list", "--porcelain"]).unwrap_or_default();
    let mut rows = Vec::new();
    let mut current: Option<Value> = None;
    for line in out.lines().chain(std::iter::once("")) {
        if line.is_empty() {
            if let Some(row) = current.take() {
                rows.push(row);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("worktree ") {
            current = Some(json!({ "path": rest, "branch": Value::Null, "head": Value::Null }));
        } else if let Some(rest) = line.strip_prefix("branch ") {
            if let Some(row) = current.as_mut() {
                row["branch"] = Value::String(rest.replace("refs/heads/", ""));
            }
        } else if let Some(rest) = line.strip_prefix("HEAD ") {
            if let Some(row) = current.as_mut() {
                row["head"] = Value::String(rest.to_string());
            }
        }
    }
    if let Some(first) = rows.first_mut() {
        first["isMain"] = Value::Bool(true);
    }
    Value::Array(rows)
}

fn slugify(branch: &str) -> String {
    branch
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub fn worktree_add(raw: &str, options: &Value) -> Result<Value, String> {
    let repo = repo_root(raw).ok_or_else(|| "Not a git repository".to_string())?;
    let branch = options
        .get("branch")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if branch.is_empty() {
        return Err("Branch name is required.".into());
    }
    let base = options
        .get("baseRef")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&branch)
        .to_string();

    let dir = repo.join(".worktrees").join(slugify(&branch));
    let dir_str = dir.to_string_lossy().to_string();
    let exists = git(&repo, &["rev-parse", "--verify", &branch]).is_ok();

    if exists {
        git(&repo, &["worktree", "add", &dir_str, &branch])?;
    } else {
        git(&repo, &["worktree", "add", "-b", &branch, &dir_str, &base])?;
    }

    Ok(json!({
        "path": dir_str,
        "branch": branch,
        "repoRoot": repo.to_string_lossy()
    }))
}

pub fn worktree_remove(raw: &str, worktree_path: &str, options: &Value) -> Result<Value, String> {
    let repo = repo_root(raw).ok_or_else(|| "Not a git repository".to_string())?;
    let mut args = vec!["worktree", "remove"];
    if options.get("force").and_then(|v| v.as_bool()).unwrap_or(false) {
        args.push("--force");
    }
    args.push(worktree_path);
    git(&repo, &args)?;
    Ok(json!({ "removed": worktree_path }))
}

/// `hermes:git:review:list` — `{ files: [{path, added, removed, status, staged}], base }`.
pub fn review_list(raw: &str, scope: &str, base_ref: &str) -> Value {
    let Some(repo) = repo_root(raw) else {
        return json!({ "files": [], "base": Value::Null });
    };

    let empty = json!({ "files": [], "base": Value::Null });

    if scope == "branch" || scope == "lastTurn" {
        let base = if scope == "branch" {
            default_branch(&repo)
        } else if base_ref.is_empty() {
            None
        } else {
            Some(base_ref.to_string())
        };
        let Some(base) = base else { return empty };

        let range = format!("{base}...HEAD");
        let Ok(summary) = git(&repo, &["diff", "--numstat", &range]) else {
            return empty;
        };
        let files: Vec<Value> = summary
            .lines()
            .filter_map(|line| {
                let mut parts = line.split('\t');
                let added = parts.next()?.parse::<u64>().ok();
                let removed = parts.next()?.parse::<u64>().ok();
                let path = parts.next()?.to_string();
                Some(json!({
                    "path": path,
                    "added": added.unwrap_or(0),
                    "removed": removed.unwrap_or(0),
                    "status": "M",
                    "staged": false
                }))
            })
            .collect();
        return json!({ "files": files, "base": base });
    }

    // Default: uncommitted (staged + unstaged + untracked), one row per path.
    let entries = parse_status(&repo).unwrap_or_default();
    let staged: std::collections::HashMap<String, (u64, u64)> =
        parse_numstat_map(&git(&repo, &["diff", "--numstat", "--cached"]).unwrap_or_default());
    let unstaged: std::collections::HashMap<String, (u64, u64)> =
        parse_numstat_map(&git(&repo, &["diff", "--numstat"]).unwrap_or_default());

    let mut files: Vec<Value> = entries
        .iter()
        .map(|entry| {
            let (sa, sr) = staged.get(&entry.path).copied().unwrap_or((0, 0));
            let (ua, ur) = unstaged.get(&entry.path).copied().unwrap_or((0, 0));
            let (mut added, removed) = (sa + ua, sr + ur);
            let untracked = entry.index == '?';
            if untracked {
                added = count_lines(&repo.join(&entry.path));
            }
            json!({
                "path": entry.path,
                "added": added,
                "removed": removed,
                "status": status_letter(&format!("{}{}", entry.index, entry.worktree)),
                "staged": entry.index != ' ' && entry.index != '?'
            })
        })
        .collect();
    files.sort_by(|a, b| a["path"].as_str().unwrap_or("").cmp(b["path"].as_str().unwrap_or("")));
    files.truncate(200);
    json!({ "files": files, "base": Value::Null })
}

fn parse_numstat_map(raw: &str) -> std::collections::HashMap<String, (u64, u64)> {
    let mut map = std::collections::HashMap::new();
    for line in raw.lines() {
        let mut parts = line.split('\t');
        let added = parts.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        let removed = parts.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        if let Some(path) = parts.next() {
            map.insert(path.to_string(), (added, removed));
        }
    }
    map
}

/// `hermes:git:review:diff` / `fileDiff` — unified diff text, `""` on failure.
pub fn review_diff(raw: &str, file: &str, scope: &str, base_ref: &str, staged: bool) -> String {
    let Some(repo) = repo_root(raw) else {
        return String::new();
    };
    let tracked = git(&repo, &["ls-files", "--error-unmatch", file]).is_ok();

    if !tracked {
        // Untracked: diff against /dev/null so the pane still shows content.
        // `--no-index` exits 1 when the files differ, which is the success case.
        return git_diff_text(&repo, &["diff", "--no-index", "--", "/dev/null", file]);
    }

    if staged {
        return git(&repo, &["diff", "--cached", "--", file]).unwrap_or_default();
    }
    if (scope == "branch" || scope == "lastTurn") && !base_ref.is_empty() {
        let range = format!("{base_ref}...HEAD");
        return git(&repo, &["diff", &range, "--", file]).unwrap_or_default();
    }
    git(&repo, &["diff", "HEAD", "--", file]).unwrap_or_default()
}

pub fn stage(raw: &str, file: Option<&str>) -> Result<Value, String> {
    let repo = repo_root(raw).ok_or_else(|| "Not a git repository".to_string())?;
    match file {
        Some(file) if !file.is_empty() => git(&repo, &["add", "--", file])?,
        _ => git(&repo, &["add", "-A"])?,
    };
    Ok(json!({ "ok": true }))
}

pub fn unstage(raw: &str, file: Option<&str>) -> Result<Value, String> {
    let repo = repo_root(raw).ok_or_else(|| "Not a git repository".to_string())?;
    match file {
        Some(file) if !file.is_empty() => git(&repo, &["restore", "--staged", "--", file])?,
        _ => git(&repo, &["reset"])?,
    };
    Ok(json!({ "ok": true }))
}

pub fn revert(raw: &str, file: Option<&str>) -> Result<Value, String> {
    let repo = repo_root(raw).ok_or_else(|| "Not a git repository".to_string())?;
    let Some(file) = file.filter(|f| !f.is_empty()) else {
        return Err("A file is required to revert".into());
    };
    let tracked = git(&repo, &["ls-files", "--error-unmatch", file]).is_ok();
    if tracked {
        git(&repo, &["checkout", "--", file])?;
    } else {
        std::fs::remove_file(repo.join(file)).map_err(|e| e.to_string())?;
    }
    Ok(json!({ "ok": true }))
}

pub fn rev_parse(raw: &str, reference: &str) -> Value {
    let Some(repo) = repo_root(raw) else {
        return Value::Null;
    };
    let reference = if reference.is_empty() { "HEAD" } else { reference };
    match git(&repo, &["rev-parse", reference]) {
        Ok(out) => Value::String(out.trim().to_string()),
        Err(_) => Value::Null,
    }
}

pub fn commit(raw: &str, message: &str, push: bool) -> Result<Value, String> {
    let repo = repo_root(raw).ok_or_else(|| "Not a git repository".to_string())?;
    if message.trim().is_empty() {
        return Err("Commit message is required.".into());
    }
    git(&repo, &["commit", "-m", message])?;
    if push {
        git(&repo, &["push"])?;
    }
    Ok(json!({ "ok": true }))
}

pub fn push(raw: &str) -> Result<Value, String> {
    let repo = repo_root(raw).ok_or_else(|| "Not a git repository".to_string())?;
    git(&repo, &["push"])?;
    Ok(json!({ "ok": true }))
}

pub fn commit_context(raw: &str) -> Value {
    let Some(repo) = repo_root(raw) else {
        return json!({ "diff": "", "recent": "" });
    };
    json!({
        "diff": git(&repo, &["diff", "--cached"]).unwrap_or_default(),
        "recent": git(&repo, &["log", "-5", "--oneline"]).unwrap_or_default()
    })
}

/// `hermes:git:scanRepos` — bounded walk for `.git` directories.
pub fn scan_repos(roots: &[String], max_depth: usize) -> Value {
    fn walk(dir: &Path, depth: usize, max_depth: usize, found: &mut Vec<Value>) {
        if depth > max_depth {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        if entries.iter().any(|e| e.file_name() == ".git") {
            found.push(json!({
                "root": dir.to_string_lossy(),
                "label": dir.file_name().map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| dir.to_string_lossy().to_string())
            }));
            return;
        }
        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || matches!(name.as_str(), "node_modules" | "Library" | ".Trash") {
                continue;
            }
            if entry.path().is_dir() {
                walk(&entry.path(), depth + 1, max_depth, found);
            }
        }
    }

    let mut found = Vec::new();
    for root in roots {
        let path = crate::fs_ops::expand(root);
        if path.is_dir() {
            walk(&path, 0, max_depth, &mut found);
        }
        if found.len() >= 200 {
            break;
        }
    }
    Value::Array(found)
}

// --- gh-backed channels -----------------------------------------------------

/// Locate a CLI the way a login shell would. A `.app` launched from Finder gets
/// a minimal PATH with no `/opt/homebrew/bin`, so `Command::new("gh")` fails
/// there while the same call works in a terminal — resolve the binary first.
fn tool(name: &str) -> std::path::PathBuf {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    for dir in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
    ] {
        candidates.push(std::path::Path::new(dir).join(name));
    }
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(std::path::Path::new(&home).join(".local/bin").join(name));
    }
    candidates
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| std::path::PathBuf::from(name))
}

/// PATH handed to child processes, so tools a script shells out to (`gh pr
/// create` invoking git, hooks, editors) also resolve outside a login shell.
fn tool_path() -> String {
    let mut parts = vec![
        "/opt/homebrew/bin".to_string(),
        "/usr/local/bin".to_string(),
        "/usr/bin".to_string(),
        "/bin".to_string(),
    ];
    if let Some(existing) = std::env::var_os("PATH") {
        parts.push(existing.to_string_lossy().to_string());
    }
    parts.join(":")
}

fn gh(args: &[&str], cwd: &Path) -> Option<String> {
    let output = Command::new(tool("gh"))
        .args(args)
        .current_dir(cwd)
        .env("PATH", tool_path())
        .output()
        .ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

/// `hermes:git:review:shipInfo` — `{ ghReady, pr }`.
pub fn ship_info(raw: &str) -> Value {
    let Some(repo) = repo_root(raw) else {
        return json!({ "ghReady": false, "pr": Value::Null });
    };
    if gh(&["auth", "status"], &repo).is_none() {
        return json!({ "ghReady": false, "pr": Value::Null });
    }
    let pr = gh(
        &["pr", "view", "--json", "number,title,state,url,isDraft"],
        &repo,
    )
    .and_then(|out| serde_json::from_str::<Value>(&out).ok())
    .unwrap_or(Value::Null);
    json!({ "ghReady": true, "pr": pr })
}

pub fn pr_list(raw: &str) -> Value {
    let Some(repo) = repo_root(raw) else {
        return json!([]);
    };
    gh(
        &["pr", "list", "--json", "number,title,state,headRefName,url", "--limit", "30"],
        &repo,
    )
    .and_then(|out| serde_json::from_str::<Value>(&out).ok())
    .unwrap_or_else(|| json!([]))
}

pub fn create_pr(raw: &str) -> Result<Value, String> {
    let repo = repo_root(raw).ok_or_else(|| "Not a git repository".to_string())?;
    let out = gh(&["pr", "create", "--fill"], &repo)
        .ok_or_else(|| "gh pr create failed (is gh installed and authenticated?)".to_string())?;
    Ok(json!({ "url": out.trim() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .status()
            .expect("git");
        assert!(status.success(), "git {:?} failed", args);
    }

    fn repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hermes-git-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        run(&dir, &["init", "-b", "main"]);
        std::fs::write(dir.join("tracked.txt"), "one\n").unwrap();
        run(&dir, &["add", "-A"]);
        run(&dir, &["commit", "-m", "init"]);
        dir
    }

    #[test]
    fn repo_status_reports_branch_and_changes() {
        let dir = repo("status");
        let clean = repo_status(&dir.to_string_lossy());
        assert_eq!(clean["branch"], "main");
        assert_eq!(clean["changed"], 0);

        std::fs::write(dir.join("tracked.txt"), "one\ntwo\n").unwrap();
        std::fs::write(dir.join("new.txt"), "a\nb\n").unwrap();
        let dirty = repo_status(&dir.to_string_lossy());
        assert!(dirty["changed"].as_u64().unwrap() >= 2);
        assert!(dirty["untracked"].as_u64().unwrap() >= 1);
        assert!(dirty["added"].as_u64().unwrap() >= 3);
    }

    #[test]
    fn review_list_and_diff_expose_the_working_tree() {
        let dir = repo("review");
        std::fs::write(dir.join("tracked.txt"), "one\nchanged\n").unwrap();

        let list = review_list(&dir.to_string_lossy(), "uncommitted", "");
        let files = list["files"].as_array().unwrap();
        assert!(files.iter().any(|f| f["path"] == "tracked.txt"));
        assert_eq!(list["base"], Value::Null);

        let diff = review_diff(&dir.to_string_lossy(), "tracked.txt", "uncommitted", "", false);
        assert!(diff.contains("+changed"), "diff was: {diff}");
    }

    #[test]
    fn stage_then_unstage_moves_the_file() {
        let dir = repo("stage");
        std::fs::write(dir.join("tracked.txt"), "one\ntwo\n").unwrap();

        stage(&dir.to_string_lossy(), Some("tracked.txt")).unwrap();
        let staged = review_list(&dir.to_string_lossy(), "uncommitted", "");
        let row = staged["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["path"] == "tracked.txt")
            .cloned()
            .unwrap();
        assert_eq!(row["staged"], true);

        unstage(&dir.to_string_lossy(), Some("tracked.txt")).unwrap();
        let after = review_list(&dir.to_string_lossy(), "uncommitted", "");
        let row = after["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["path"] == "tracked.txt")
            .cloned()
            .unwrap();
        assert_eq!(row["staged"], false);
    }

    #[test]
    fn branch_list_marks_the_default_branch() {
        let dir = repo("branches");
        let list = branch_list(&dir.to_string_lossy());
        let rows = list.as_array().unwrap();
        assert!(rows.iter().any(|row| row["name"] == "main" && row["isDefault"] == true));

        branch_switch(&dir.to_string_lossy(), "feature").ok();
        run(&dir, &["checkout", "-b", "feature"]);
        let after = branch_list(&dir.to_string_lossy());
        assert!(after
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["name"] == "feature"));
    }

    #[test]
    fn commit_and_push_errors_are_not_swallowed() {
        let dir = repo("commit");
        std::fs::write(dir.join("tracked.txt"), "one\ntwo\n").unwrap();
        stage(&dir.to_string_lossy(), None).unwrap();
        let ok = commit(&dir.to_string_lossy(), "second", false).unwrap();
        assert_eq!(ok["ok"], true);

        // No remote configured: push must fail loudly, not report success.
        assert!(push(&dir.to_string_lossy()).is_err());
    }

    #[test]
    fn worktree_list_reports_the_main_tree() {
        let dir = repo("worktree");
        let list = worktree_list(&dir.to_string_lossy());
        let rows = list.as_array().unwrap();
        assert!(!rows.is_empty());
        assert!(rows.iter().any(|row| row["isMain"] == true));
    }

    #[test]
    fn scan_repos_finds_sibling_repositories_and_treats_a_repo_as_a_leaf() {
        let parent = std::env::temp_dir().join(format!("hermes-git-{}-scan", std::process::id()));
        let _ = std::fs::remove_dir_all(&parent);
        let alpha = parent.join("alpha");
        let beta = parent.join("beta");
        std::fs::create_dir_all(&alpha).unwrap();
        std::fs::create_dir_all(&beta).unwrap();
        run(&alpha, &["init", "-b", "main"]);
        run(&beta, &["init", "-b", "main"]);
        // A repo nested INSIDE another repo is a leaf, not a second result.
        let nested = alpha.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        run(&nested, &["init", "-b", "main"]);

        let found = scan_repos(&[parent.to_string_lossy().to_string()], 3);
        let roots: Vec<String> = found
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["root"].as_str().unwrap_or("").to_string())
            .collect();

        assert!(roots.iter().any(|r| r.ends_with("/alpha")));
        assert!(roots.iter().any(|r| r.ends_with("/beta")));
        assert!(
            !roots.iter().any(|r| r.ends_with("/nested")),
            "a repository inside a repository must not be listed again: {roots:?}"
        );
    }

    #[test]
    fn untracked_files_diff_and_count_without_an_extra_line() {
        let dir = repo("untracked");
        std::fs::write(dir.join("brand-new.txt"), "hello\nworld\n").unwrap();

        let diff = review_diff(&dir.to_string_lossy(), "brand-new.txt", "uncommitted", "", false);
        assert!(diff.contains("+hello"), "untracked diff was: {diff:?}");

        let list = review_list(&dir.to_string_lossy(), "uncommitted", "");
        let row = list["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["path"] == "brand-new.txt")
            .cloned()
            .expect("untracked file listed");
        assert_eq!(row["added"], 2, "trailing newline must not add a line: {row}");
        assert_eq!(row["status"], "A");
    }

    #[test]
    fn non_repo_paths_return_the_documented_empty_shape() {
        assert_eq!(repo_status("/tmp"), Value::Null);
        assert_eq!(branch_list("/tmp"), json!([]));
        assert_eq!(worktree_list("/tmp"), json!([]));
        assert_eq!(review_list("/tmp", "uncommitted", ""), json!({"files": [], "base": null}));
    }
}
