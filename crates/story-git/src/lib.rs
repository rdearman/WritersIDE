//! Local Git history operations. Callers provide the storage snapshot barrier.
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("Git is not installed or could not be started: {0}")]
    Launch(#[from] std::io::Error),
    #[error("Git operation failed: {0}")]
    Command(String),
    #[error("project directory is inside a different Git repository")]
    WrongRepository,
    #[error("invalid commit identifier")]
    InvalidCommit,
}
pub type Result<T> = std::result::Result<T, GitError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitCommit {
    pub id: String,
    pub date: String,
    pub subject: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitRestoreChange {
    pub status: String,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitRestorePreview {
    pub commit_id: String,
    pub changes: Vec<GitRestoreChange>,
    pub working_tree_changes: Vec<GitRestoreChange>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitRemote {
    pub name: String,
}

/// A historical occurrence of text, with the introducing commit and its evidence diff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalTextChange {
    pub commit: GitCommit,
    pub path: String,
    pub evidence_diff: String,
}

/// A line's origin in the project-local Git history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlamedLine {
    pub line: usize,
    pub commit: String,
    pub author: String,
    pub date: String,
    pub summary: String,
    pub content: String,
}

/// Project-local interval snapshot policy. A missing interval disables automation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoSnapshotPolicy {
    pub interval_minutes: Option<u32>,
    #[serde(default)]
    pub snapshot_on_exit: bool,
    #[serde(default)]
    last_checked_unix: Option<i64>,
}

impl AutoSnapshotPolicy {
    pub fn new(interval_minutes: Option<u32>) -> Result<Self> {
        if interval_minutes.is_some_and(|minutes| !(1..=10_080).contains(&minutes)) {
            return Err(GitError::Command(
                "automatic snapshot interval must be between 1 and 10080 minutes".into(),
            ));
        }
        Ok(Self {
            interval_minutes,
            snapshot_on_exit: false,
            last_checked_unix: None,
        })
    }

    pub fn set_interval(&mut self, interval_minutes: Option<u32>) -> Result<()> {
        if interval_minutes.is_some_and(|minutes| !(1..=10_080).contains(&minutes)) {
            return Err(GitError::Command(
                "automatic snapshot interval must be between 1 and 10080 minutes".into(),
            ));
        }
        self.interval_minutes = interval_minutes;
        self.last_checked_unix = None;
        Ok(())
    }

    pub fn due(&self, now_unix: i64) -> bool {
        let Some(minutes) = self.interval_minutes else {
            return false;
        };
        self.last_checked_unix
            .and_then(|last| now_unix.checked_sub(last))
            .is_none_or(|elapsed| elapsed >= i64::from(minutes) * 60)
    }

    pub fn record_check(&mut self, now_unix: i64) {
        self.last_checked_unix = Some(now_unix);
    }
}

pub fn load_auto_snapshot_policy(root: impl AsRef<Path>) -> Result<AutoSnapshotPolicy> {
    let path = root.as_ref().join(".writers-ide/git-snapshot.toml");
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let policy: AutoSnapshotPolicy = toml::from_str(&text).map_err(|error| {
                GitError::Command(format!("invalid Git snapshot policy: {error}"))
            })?;
            AutoSnapshotPolicy::new(policy.interval_minutes).map(|mut validated| {
                validated.snapshot_on_exit = policy.snapshot_on_exit;
                validated.last_checked_unix = policy.last_checked_unix;
                validated
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(AutoSnapshotPolicy::default())
        }
        Err(error) => Err(error.into()),
    }
}

pub fn save_auto_snapshot_policy(
    root: impl AsRef<Path>,
    policy: &AutoSnapshotPolicy,
) -> Result<()> {
    AutoSnapshotPolicy::new(policy.interval_minutes)?;
    let root = root.as_ref();
    let directory = root.join(".writers-ide");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("git-snapshot.toml");
    let temp = directory.join(format!("git-snapshot-{}.tmp", uuid::Uuid::new_v4()));
    let text = toml::to_string(policy).map_err(|error| GitError::Command(error.to_string()))?;
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&temp, &path)?;
    std::fs::File::open(&directory)?.sync_all()?;
    Ok(())
}

pub struct GitHistory {
    root: PathBuf,
}
impl GitHistory {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().canonicalize()?;
        let history = Self { root };
        if !history.root.join(".git").exists() {
            history.run(&["init", "--quiet"])?;
        }
        let top = history.output(&["rev-parse", "--show-toplevel"])?;
        let top = PathBuf::from(String::from_utf8_lossy(&top.stdout).trim()).canonicalize()?;
        if top != history.root {
            return Err(GitError::WrongRepository);
        }
        history.ensure_ignore()?;
        Ok(history)
    }

    /// Opens an existing project repository without initializing Git or
    /// repairing `.gitignore`; suitable for strictly read-only operations.
    pub fn open_existing(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().canonicalize()?;
        if !root.join(".git").exists() {
            return Err(GitError::Command(
                "project has no local Git history yet".into(),
            ));
        }
        let history = Self { root };
        let top = history.output(&["rev-parse", "--show-toplevel"])?;
        let top = PathBuf::from(String::from_utf8_lossy(&top.stdout).trim()).canonicalize()?;
        if top != history.root {
            return Err(GitError::WrongRepository);
        }
        Ok(history)
    }

    /// Stages and commits only the application-owned project paths.
    pub fn snapshot(&self, label: &str, author: &str) -> Result<Option<String>> {
        let label = label.trim();
        if label.is_empty() {
            return Err(GitError::Command("snapshot label cannot be empty".into()));
        }
        let paths = [
            ".gitignore",
            "project.toml",
            "manuscript",
            "research",
            "attachments",
            "templates",
            "snapshots/database.sql",
        ]
        .into_iter()
        .filter(|path| self.root.join(path).exists())
        .collect::<Vec<_>>();
        let staged = self.output(&["diff", "--cached", "--name-only"])?;
        for path in String::from_utf8_lossy(&staged.stdout).lines() {
            if !is_owned_path(path) {
                return Err(GitError::Command(format!(
                    "unrelated staged path {path:?}; commit or unstage it before creating a Writer's IDE snapshot"
                )));
            }
        }
        let mut add = vec!["add", "-A", "--"];
        add.extend(paths.iter().copied());
        self.run(&add)?;
        match self.run(&["diff", "--cached", "--quiet"]) {
            Ok(_) => return Ok(None),
            Err(GitError::Command(message)) if message.contains("exit status: 1") => {}
            Err(error) => return Err(error),
        }
        let who = if author.trim().is_empty() {
            "Writer's IDE"
        } else {
            author.trim()
        };
        let user_config = format!("user.name={who}");
        self.run(&[
            "-c",
            &user_config,
            "-c",
            "user.email=local@writers-ide.invalid",
            "commit",
            "--quiet",
            "-m",
            label,
        ])?;
        let output = self.output(&["rev-parse", "HEAD"])?;
        let id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        Ok(Some(id))
    }

    pub fn history(&self, limit: usize) -> Result<Vec<GitCommit>> {
        let count = limit.clamp(1, 10000).to_string();
        let output = self.output(&["log", &format!("-n{count}"), "--format=%H%x1f%cI%x1f%s"])?;
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(3, '\u{1f}');
                Some(GitCommit {
                    id: parts.next()?.into(),
                    date: parts.next()?.into(),
                    subject: parts.next()?.into(),
                })
            })
            .collect())
    }

    /// Lists committed application-owned files at a revision in stable path order.
    pub fn tracked_paths(&self, revision: &str) -> Result<Vec<String>> {
        validate_commit(revision)?;
        let output = self.output(&["ls-tree", "-r", "--name-only", "-z", revision])?;
        let mut paths = Vec::new();
        for path in output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
        {
            let path = std::str::from_utf8(path)
                .map_err(|_| GitError::Command("snapshot contains a non-UTF-8 path".into()))?
                .to_owned();
            if !is_owned_path(&path) {
                return Err(GitError::Command(format!(
                    "snapshot contains a path outside the application-owned set: {path:?}"
                )));
            }
            paths.push(path);
        }
        paths.sort();
        Ok(paths)
    }

    /// Shows the tracked file-level effects of restoring `revision` over HEAD.
    /// Uncommitted working-tree edits are reported separately by callers.
    pub fn restore_preview(&self, revision: &str) -> Result<GitRestorePreview> {
        validate_commit(revision)?;
        let mut args = vec![
            "diff",
            "--name-status",
            "-z",
            "--no-renames",
            "HEAD",
            revision,
            "--",
        ];
        args.extend(OWNED_PATHS.iter().copied());
        let output = self.output(&args)?;
        let records = output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|field| !field.is_empty())
            .map(|field| {
                std::str::from_utf8(field)
                    .map(str::to_owned)
                    .map_err(|_| GitError::Command("snapshot contains a non-UTF-8 path".into()))
            })
            .collect::<Result<Vec<_>>>()?;
        if records.len() % 2 != 0 {
            return Err(GitError::Command(
                "Git returned a malformed restore preview".into(),
            ));
        }
        let changes = records
            .chunks(2)
            .map(|pair| {
                if !is_owned_path(&pair[1]) {
                    return Err(GitError::Command(format!(
                        "restore preview returned a path outside the application-owned set: {:?}",
                        pair[1]
                    )));
                }
                let status = match pair[0].as_str() {
                    "A" => "added",
                    "D" => "removed",
                    "M" | "T" => "changed",
                    other => {
                        return Err(GitError::Command(format!(
                            "unsupported Git restore status: {other}"
                        )));
                    }
                };
                Ok(GitRestoreChange {
                    status: status.into(),
                    path: pair[1].clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut status_args = vec![
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--no-renames",
            "--",
        ];
        status_args.extend(OWNED_PATHS.iter().copied());
        let status_output = self.output(&status_args)?;
        let mut working_tree_changes = Vec::new();
        for record in status_output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|field| !field.is_empty())
        {
            let record = std::str::from_utf8(record)
                .map_err(|_| GitError::Command("working tree contains a non-UTF-8 path".into()))?;
            if record.len() < 4 {
                return Err(GitError::Command(
                    "Git returned a malformed working-tree status".into(),
                ));
            }
            let status = &record[..2];
            let path = record[3..].to_owned();
            if !is_owned_path(&path) {
                return Err(GitError::Command(format!(
                    "working-tree status returned a path outside the application-owned set: {path:?}"
                )));
            }
            let status = if status == "??" || status.contains('A') {
                "added"
            } else if status.contains('D') {
                "removed"
            } else {
                "changed"
            };
            working_tree_changes.push(GitRestoreChange {
                status: status.into(),
                path,
            });
        }
        Ok(GitRestorePreview {
            commit_id: self.resolve_commit(revision)?,
            changes,
            working_tree_changes,
        })
    }

    pub fn resolve_commit(&self, revision: &str) -> Result<String> {
        validate_commit(revision)?;
        let output = self.output(&["rev-parse", "--verify", &format!("{revision}^{{commit}}")])?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    /// Identifies the checked-out branch. Detached HEADs cannot be used for a
    /// branch retrieval because there is no branch ref to advance safely.
    pub fn current_branch(&self) -> Result<String> {
        let output = self.output(&["symbolic-ref", "--quiet", "--short", "HEAD"])?;
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        self.validate_branch(&branch)?;
        Ok(branch)
    }

    pub fn head_commit(&self) -> Result<String> {
        let output = self.output(&["rev-parse", "--verify", "HEAD^{commit}"])?;
        let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        validate_commit(&commit)?;
        Ok(commit)
    }

    pub fn add_remote(&self, name: &str, url: &str) -> Result<()> {
        validate_remote_name(name)?;
        validate_remote_url(url)?;
        self.run(&["remote", "add", name, url])?;
        Ok(())
    }

    pub fn remotes(&self) -> Result<Vec<GitRemote>> {
        let output = self.output(&["remote"])?;
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_owned)
            .filter(|name| !name.is_empty())
            .map(|name| GitRemote { name })
            .collect())
    }

    /// Pushes the current branch to the named remote branch without force options.
    pub fn push_branch(&self, remote: &str, branch: &str) -> Result<()> {
        validate_remote_name(remote)?;
        self.validate_branch(branch)?;
        self.run(&[
            "push",
            "--porcelain",
            remote,
            &format!("HEAD:refs/heads/{branch}"),
        ])?;
        Ok(())
    }

    /// Fetches one remote branch without modifying the working tree and returns
    /// the fetched commit ID.
    pub fn fetch_branch(&self, remote: &str, branch: &str) -> Result<String> {
        validate_remote_name(remote)?;
        self.validate_branch(branch)?;
        let destination = format!("refs/remotes/{remote}/{branch}");
        let source = format!("refs/heads/{branch}:{destination}");
        self.run(&["fetch", "--no-tags", remote, &source])?;
        let output = self.output(&[
            "rev-parse",
            "--verify",
            &format!("{destination}^{{commit}}"),
        ])?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    /// Returns whether `ancestor` is an ancestor of `descendant`.
    pub fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool> {
        validate_commit(ancestor)?;
        validate_commit(descendant)?;
        let output = self.raw(&["merge-base", "--is-ancestor", ancestor, descendant])?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(GitError::Command(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            )),
        }
    }

    /// Advances the current branch only when Git can perform a fast-forward.
    pub fn fast_forward_to(&self, commit: &str) -> Result<()> {
        validate_commit(commit)?;
        self.run(&["merge", "--ff-only", commit])?;
        Ok(())
    }

    /// Advances a branch after the caller has already reconciled its tracked
    /// working files to `commit`. Unlike `merge`, this does not overwrite the
    /// working tree. The old branch value is an expected-value guard against
    /// another Git process moving the branch concurrently.
    pub fn fast_forward_ref_to(&self, commit: &str, expected_old: &str) -> Result<()> {
        validate_commit(commit)?;
        validate_commit(expected_old)?;
        let branch = self.current_branch()?;
        if self.head_commit()? != expected_old {
            return Err(GitError::Command(
                "current branch changed during remote retrieval".into(),
            ));
        }
        if !self.is_ancestor(expected_old, commit)? {
            return Err(GitError::Command(
                "remote commit is not a fast-forward of the current branch".into(),
            ));
        }
        let reference = format!("refs/heads/{branch}");
        self.run(&["update-ref", &reference, commit, expected_old])?;
        self.run(&["read-tree", commit])?;
        Ok(())
    }

    fn validate_branch(&self, branch: &str) -> Result<()> {
        if branch.is_empty() || branch.starts_with('-') {
            return Err(GitError::Command("invalid remote branch name".into()));
        }
        self.run(&["check-ref-format", "--branch", branch])?;
        Ok(())
    }

    pub fn diff(&self, from: &str, to: &str, path: Option<&str>) -> Result<String> {
        validate_commit(from)?;
        validate_commit(to)?;
        let mut args = vec!["diff", "--no-ext-diff", "--unified=3", from, to, "--"];
        if let Some(path) = path {
            if !is_owned_path(path) {
                return Err(GitError::Command(
                    "path is outside the versioned project content".into(),
                ));
            }
            args.push(path)
        }
        let output = self.output(&args)?;
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    pub fn read_file_at(&self, commit: &str, path: &str) -> Result<Vec<u8>> {
        validate_commit(commit)?;
        if !is_owned_path(path) {
            return Err(GitError::Command(
                "path is outside the versioned project content".into(),
            ));
        }
        let spec = format!("{commit}:{path}");
        Ok(self.output(&["show", &spec])?.stdout)
    }

    /// Finds commits that introduced or changed a literal passage/fact. Results are oldest first.
    /// The search is read-only and restricted to project-owned paths.
    pub fn search_text_history(
        &self,
        needle: &str,
        path: Option<&str>,
        limit: usize,
    ) -> Result<Vec<HistoricalTextChange>> {
        let needle = needle.trim();
        if needle.is_empty() {
            return Err(GitError::Command(
                "history search text cannot be empty".into(),
            ));
        }
        if let Some(path) = path
            && !is_owned_path(path)
        {
            return Err(GitError::Command(
                "path is outside the versioned project content".into(),
            ));
        }
        let count = limit.clamp(1, 1000).to_string();
        let mut args = vec![
            "log".to_owned(),
            "--all".into(),
            "--reverse".into(),
            format!("-n{count}"),
            "--format=%H%x1f%cI%x1f%s".into(),
            "-S".into(),
            needle.to_owned(),
            "--".into(),
        ];
        if let Some(path) = path {
            args.push(path.to_owned());
        }
        let output = self.output(&args.iter().map(String::as_str).collect::<Vec<_>>())?;
        let commits = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let mut fields = line.splitn(3, '\u{1f}');
                Some(GitCommit {
                    id: fields.next()?.into(),
                    date: fields.next()?.into(),
                    subject: fields.next()?.into(),
                })
            })
            .collect::<Vec<_>>();
        let mut changes = Vec::new();
        for commit in commits {
            let parent = self.output(&["rev-parse", "-q", "--verify", &format!("{}^", commit.id)]);
            let mut diff_args = if let Ok(parent) = parent {
                let parent_id = String::from_utf8_lossy(&parent.stdout).trim().to_owned();
                vec![
                    "diff".to_owned(),
                    "--no-ext-diff".into(),
                    "--unified=3".into(),
                    parent_id,
                    commit.id.clone(),
                    "--".into(),
                ]
            } else {
                vec![
                    "show".to_owned(),
                    "--no-ext-diff".into(),
                    "--format=".into(),
                    "--unified=3".into(),
                    commit.id.clone(),
                    "--".into(),
                ]
            };
            if let Some(path) = path {
                diff_args.push(path.to_owned());
            }
            let diff = String::from_utf8_lossy(
                &self
                    .output(&diff_args.iter().map(String::as_str).collect::<Vec<_>>())?
                    .stdout,
            )
            .into_owned();
            if diff.contains(needle) {
                let changed_path = path
                    .map(str::to_owned)
                    .or_else(|| {
                        diff.lines()
                            .find_map(|line| line.strip_prefix("+++ b/").map(str::to_owned))
                    })
                    .unwrap_or_default();
                changes.push(HistoricalTextChange {
                    commit,
                    path: changed_path,
                    evidence_diff: diff,
                });
            }
        }
        Ok(changes)
    }

    /// Returns Git's line-level provenance for a project-owned text file.
    pub fn blame(
        &self,
        path: &str,
        first_line: usize,
        last_line: usize,
    ) -> Result<Vec<BlamedLine>> {
        if !is_owned_path(path)
            || first_line == 0
            || last_line < first_line
            || last_line - first_line > 10000
        {
            return Err(GitError::Command(
                "invalid project path or line range".into(),
            ));
        }
        let range = format!("-L{first_line},{last_line}");
        let output = self.output(&["blame", "--line-porcelain", &range, "--", path])?;
        parse_blame(&String::from_utf8_lossy(&output.stdout))
    }

    /// Restores the application-owned files from a commit into a new directory.
    /// The live project and destination are never modified if the destination already exists.
    pub fn restore_files_to_new_directory(
        &self,
        commit: &str,
        destination: impl AsRef<Path>,
    ) -> Result<Vec<PathBuf>> {
        validate_commit(commit)?;
        self.output(&["cat-file", "-e", &format!("{commit}^{{commit}}")])?;
        let destination = destination.as_ref();
        if destination.exists() {
            return Err(GitError::Command(
                "restore destination already exists; choose a new directory".into(),
            ));
        }
        let listing = self.output(&["ls-tree", "-r", "--name-only", "-z", commit])?;
        let mut files = Vec::new();
        let stage =
            destination.with_extension(format!("writers-ide-restore-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&stage)?;
        let result = (|| {
            for raw in listing
                .stdout
                .split(|byte| *byte == 0)
                .filter(|item| !item.is_empty())
            {
                let relative = std::str::from_utf8(raw)
                    .map_err(|_| GitError::Command("snapshot contains a non-UTF-8 path".into()))?;
                if !is_owned_path(relative)
                    || !(relative == ".gitignore"
                        || relative == "project.toml"
                        || relative == "snapshots/database.sql"
                        || ["manuscript/", "research/", "attachments/", "templates/"]
                            .iter()
                            .any(|prefix| relative.starts_with(prefix)))
                {
                    continue;
                }
                let target = stage.join(relative);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let bytes = self.read_file_at(commit, relative)?;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target)?;
                use std::io::Write;
                file.write_all(&bytes)?;
                file.sync_all()?;
                files.push(PathBuf::from(relative));
            }
            if !stage.join("project.toml").is_file()
                || !stage.join("snapshots/database.sql").is_file()
            {
                return Err(GitError::Command(
                    "commit does not contain a restorable Writer's IDE metadata snapshot".into(),
                ));
            }
            std::fs::rename(&stage, destination)?;
            if let Some(parent) = destination.parent() {
                std::fs::File::open(parent)?.sync_all()?;
            }
            Ok(files.clone())
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&stage);
        }
        result
    }

    fn ensure_ignore(&self) -> Result<()> {
        let path = self.root.join(".gitignore");
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        let marker = "# writers-ide generated and live state";
        if existing.contains(marker) {
            return Ok(());
        }
        let mut updated = existing;
        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push_str(marker);
        updated.push_str(
            "\n/story.db\n/story.db-wal\n/story.db-shm\n/*.sqlite\n/*.sqlite-wal\n/*.sqlite-shm\n/.writers-ide/\n/exports/\n*.tmp\n.env\n.env.*\n*.pem\n*.key\n*credentials*\n",
        );
        std::fs::write(path, updated)?;
        Ok(())
    }
    fn run(&self, args: &[&str]) -> Result<Output> {
        check(self.raw(args)?)
    }
    fn raw(&self, args: &[&str]) -> Result<Output> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()?;
        Ok(output)
    }
    fn output(&self, args: &[&str]) -> Result<Output> {
        self.run(args)
    }
}
fn check(output: Output) -> Result<Output> {
    if output.status.success() {
        Ok(output)
    } else {
        Err(GitError::Command(format!(
            "{}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}
fn validate_commit(value: &str) -> Result<()> {
    if value.len() < 7 || value.len() > 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Err(GitError::InvalidCommit)
    } else {
        Ok(())
    }
}
fn validate_remote_name(value: &str) -> Result<()> {
    if value.is_empty()
        || value.starts_with('-')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        || value.contains("..")
    {
        return Err(GitError::Command("invalid Git remote name".into()));
    }
    Ok(())
}
fn validate_remote_url(value: &str) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.starts_with('-') || trimmed.chars().any(char::is_whitespace) {
        return Err(GitError::Command("invalid Git remote URL".into()));
    }
    let lower = trimmed.to_ascii_lowercase();
    let secret_parameter = ["token=", "password=", "passwd=", "secret=", "access_key="]
        .iter()
        .any(|needle| lower.contains(needle));
    let embedded_http_credentials = ["http://", "https://"].iter().any(|scheme| {
        lower
            .find(scheme)
            .and_then(|start| {
                let authority = &trimmed[start + scheme.len()..];
                let end = authority.find(['/', '?', '#']).unwrap_or(authority.len());
                authority[..end].contains('@').then_some(())
            })
            .is_some()
    });
    if secret_parameter || embedded_http_credentials {
        return Err(GitError::Command(
            "remote URLs must not embed credentials or access tokens; configure a credential helper or SSH agent".into(),
        ));
    }
    Ok(())
}
fn parse_blame(source: &str) -> Result<Vec<BlamedLine>> {
    let mut result = Vec::new();
    let mut lines = source.lines().peekable();
    while let Some(header) = lines.next() {
        let mut fields = header.split_whitespace();
        let commit = fields
            .next()
            .ok_or_else(|| GitError::Command("malformed git blame output".into()))?;
        let _original = fields.next();
        let final_line = fields
            .next()
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or_else(|| GitError::Command("malformed git blame line number".into()))?;
        let mut author = String::new();
        let mut date = String::new();
        let mut summary = String::new();
        for metadata in lines.by_ref() {
            if let Some(value) = metadata.strip_prefix("author ") {
                author = value.into();
            } else if let Some(value) = metadata.strip_prefix("author-time ") {
                date = value.into();
            } else if let Some(value) = metadata.strip_prefix("summary ") {
                summary = value.into();
            } else if let Some(value) = metadata.strip_prefix('\t') {
                result.push(BlamedLine {
                    line: final_line,
                    commit: commit.into(),
                    author,
                    date,
                    summary,
                    content: value.into(),
                });
                break;
            }
        }
    }
    Ok(result)
}
fn is_owned_path(path: &str) -> bool {
    let safe_relative = Path::new(path)
        .components()
        .all(|component| matches!(component, std::path::Component::Normal(_)));
    safe_relative
        && (path == ".gitignore"
            || path == "project.toml"
            || path == "snapshots/database.sql"
            || ["manuscript/", "research/", "attachments/", "templates/"]
                .iter()
                .any(|prefix| path.starts_with(prefix)))
}

const OWNED_PATHS: &[&str] = &[
    ".gitignore",
    "project.toml",
    "manuscript",
    "research",
    "attachments",
    "templates",
    "snapshots/database.sql",
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_snapshot_policy_validates_interval_and_tracks_due_time() {
        assert!(AutoSnapshotPolicy::new(Some(0)).is_err());
        assert!(AutoSnapshotPolicy::new(Some(10_081)).is_err());
        let mut policy = AutoSnapshotPolicy::new(Some(2)).unwrap();
        assert!(policy.due(100));
        policy.record_check(100);
        assert!(!policy.due(219));
        assert!(policy.due(220));
        assert!(!AutoSnapshotPolicy::default().due(100));
    }

    #[test]
    fn auto_snapshot_policy_round_trips_atomically() {
        let root = tempfile::tempdir().unwrap();
        let mut policy = AutoSnapshotPolicy::new(Some(30)).unwrap();
        policy.snapshot_on_exit = true;
        policy.record_check(1234);
        save_auto_snapshot_policy(root.path(), &policy).unwrap();
        assert_eq!(load_auto_snapshot_policy(root.path()).unwrap(), policy);
    }

    #[test]
    fn local_snapshots_are_named_deterministic_and_ignore_live_database() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("project.toml"), "title='Test'\n").unwrap();
        std::fs::create_dir_all(dir.path().join("manuscript")).unwrap();
        std::fs::write(dir.path().join("manuscript/one.md"), "First draft\n").unwrap();
        std::fs::write(dir.path().join("story.db"), "live db").unwrap();
        std::fs::write(dir.path().join("snapshots.sql"), "ignored").unwrap();
        std::fs::create_dir_all(dir.path().join("snapshots")).unwrap();
        std::fs::write(dir.path().join("snapshots/database.sql"), "-- dump\n").unwrap();
        let git = GitHistory::open(dir.path()).unwrap();
        let first = git.snapshot("First draft", "Author").unwrap().unwrap();
        assert!(git.snapshot("No changes", "Author").unwrap().is_none());
        std::fs::write(dir.path().join("manuscript/one.md"), "Second draft\n").unwrap();
        let second = git.snapshot("Second draft", "Author").unwrap().unwrap();
        assert_ne!(first, second);
        let history = git.history(10).unwrap();
        assert_eq!(history.len(), 2);
        assert!(
            git.diff(&first, &second, Some("manuscript/one.md"))
                .unwrap()
                .contains("Second draft")
        );
        assert!(
            git.read_file_at(&first, "manuscript/one.md")
                .unwrap()
                .starts_with(b"First draft")
        );
        assert!(git.run(&["check-ignore", "-q", "story.db"]).is_ok());
        assert!(git.run(&["check-ignore", "-q", "snapshots.sql"]).is_err());
        assert!(
            git.diff(&first, &second, Some("manuscript/../project.toml"))
                .is_err()
        );
        std::fs::write(dir.path().join("notes.txt"), "not ours").unwrap();
        git.run(&["add", "notes.txt"]).unwrap();
        assert!(
            git.snapshot("must not consume user staging", "Author")
                .is_err()
        );
        assert_eq!(git.history(10).unwrap().len(), 2);
    }

    #[test]
    fn restore_preview_lists_owned_file_changes_and_tracked_paths() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path();
        std::fs::create_dir_all(project.join("manuscript")).unwrap();
        std::fs::create_dir_all(project.join("snapshots")).unwrap();
        std::fs::write(project.join("project.toml"), "title = 'first'\n").unwrap();
        std::fs::write(project.join("manuscript/one.md"), "first\n").unwrap();
        std::fs::write(project.join("snapshots/database.sql"), "-- first\n").unwrap();
        let history = GitHistory::open(project).unwrap();
        let first = history.snapshot("First", "Tester").unwrap().unwrap();

        std::fs::write(project.join("project.toml"), "title = 'second'\n").unwrap();
        std::fs::write(project.join("manuscript/one.md"), "second\n").unwrap();
        std::fs::write(project.join("manuscript/two.md"), "added\n").unwrap();
        std::fs::write(project.join("snapshots/database.sql"), "-- second\n").unwrap();
        let second = history.snapshot("Second", "Tester").unwrap().unwrap();

        let preview = history.restore_preview(&first).unwrap();
        assert_eq!(preview.commit_id, first);
        assert!(
            preview
                .changes
                .iter()
                .any(|change| { change.path == "project.toml" && change.status == "changed" })
        );
        assert!(
            preview
                .changes
                .iter()
                .any(|change| { change.path == "manuscript/two.md" && change.status == "removed" })
        );
        assert!(
            history
                .tracked_paths(&second)
                .unwrap()
                .contains(&"manuscript/two.md".into())
        );
        assert!(history.restore_preview("not-a-commit").is_err());
    }

    #[test]
    fn local_remote_push_and_fast_forward_retrieval_are_supported_without_force() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Novel");
        let remote = directory.path().join("backup.git");
        let cloned = directory.path().join("RemoteWork");
        std::fs::create_dir_all(project.join("manuscript")).unwrap();
        std::fs::create_dir_all(project.join("snapshots")).unwrap();
        std::fs::write(project.join("project.toml"), "title = 'Novel'\n").unwrap();
        std::fs::write(project.join("manuscript/one.md"), "first\n").unwrap();
        std::fs::write(project.join("snapshots/database.sql"), "-- first\n").unwrap();
        let history = GitHistory::open(&project).unwrap();
        let first = history.snapshot("Initial", "Tester").unwrap().unwrap();

        let initialized = Command::new("git")
            .args(["init", "--bare", remote.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(initialized.status.success());
        history
            .add_remote("backup", remote.to_str().unwrap())
            .unwrap();
        assert_eq!(
            history.remotes().unwrap(),
            vec![GitRemote {
                name: "backup".into()
            }]
        );
        assert!(
            history
                .add_remote("token", "https://user:token@example.invalid/repo.git")
                .is_err()
        );
        let branch = std::env::var("GIT_TEST_BRANCH").unwrap_or_else(|_| current_branch(&project));
        history.push_branch("backup", &branch).unwrap();
        assert_eq!(history.history(1).unwrap()[0].id, first);

        let cloned_result = Command::new("git")
            .args([
                "clone",
                "--quiet",
                "--branch",
                &branch,
                remote.to_str().unwrap(),
                cloned.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            cloned_result.status.success(),
            "{}",
            String::from_utf8_lossy(&cloned_result.stderr)
        );
        std::fs::write(cloned.join("manuscript/one.md"), "remote revision\n").unwrap();
        std::fs::write(cloned.join("snapshots/database.sql"), "-- remote\n").unwrap();
        let remote_work = GitHistory::open(&cloned).unwrap();
        let second = remote_work
            .snapshot("Remote revision", "Tester")
            .unwrap()
            .unwrap();
        remote_work.push_branch("origin", &branch).unwrap();
        let fetched = history.fetch_branch("backup", &branch).unwrap();
        assert_eq!(fetched, second);
        assert!(history.is_ancestor(&first, &fetched).unwrap());
        history.fast_forward_to(&fetched).unwrap();
        assert_eq!(
            std::fs::read_to_string(project.join("manuscript/one.md")).unwrap(),
            "remote revision\n"
        );
        assert_eq!(history.history(1).unwrap()[0].id, second);
    }

    fn current_branch(project: &Path) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(project)
            .args(["symbolic-ref", "--short", "HEAD"])
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    #[test]
    fn restore_copies_only_owned_commit_files_into_new_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("project.toml"), "title='Test'\n").unwrap();
        std::fs::create_dir_all(dir.path().join("manuscript")).unwrap();
        std::fs::write(dir.path().join("manuscript/one.md"), "First draft\n").unwrap();
        std::fs::write(dir.path().join("story.db"), "live database is excluded").unwrap();
        std::fs::create_dir_all(dir.path().join("snapshots")).unwrap();
        std::fs::write(dir.path().join("snapshots/database.sql"), "-- snapshot\n").unwrap();
        std::fs::write(dir.path().join("unrelated.txt"), "not managed").unwrap();
        let git = GitHistory::open(dir.path()).unwrap();
        let commit = git.snapshot("restore test", "Author").unwrap().unwrap();

        let destination = dir.path().join("restored");
        let restored = git
            .restore_files_to_new_directory(&commit, &destination)
            .unwrap();
        assert!(
            restored
                .iter()
                .any(|path| path == Path::new("manuscript/one.md"))
        );
        assert_eq!(
            std::fs::read(destination.join("manuscript/one.md")).unwrap(),
            b"First draft\n"
        );
        assert!(!destination.join("story.db").exists());
        assert!(!destination.join("unrelated.txt").exists());
        assert!(
            git.restore_files_to_new_directory(&commit, &destination)
                .is_err()
        );
    }

    #[test]
    fn archaeology_finds_first_literal_introduction_and_blames_lines() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("project.toml"), "title='Test'\n").unwrap();
        std::fs::create_dir_all(dir.path().join("manuscript")).unwrap();
        let manuscript = dir.path().join("manuscript/chapter.md");
        std::fs::write(&manuscript, "Opening.\n").unwrap();
        let git = GitHistory::open(dir.path()).unwrap();
        git.snapshot("initial", "Author").unwrap();
        std::fs::write(&manuscript, "Opening.\nThe key was in the chapel.\n").unwrap();
        let introduction = git.snapshot("recorded fact", "Author").unwrap().unwrap();
        std::fs::write(
            &manuscript,
            "Opening.\nThe key was in the chapel.\nLater.\n",
        )
        .unwrap();
        git.snapshot("later passage", "Author").unwrap();

        let found = git
            .search_text_history("key was in the chapel", Some("manuscript/chapter.md"), 20)
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].commit.id, introduction);
        assert!(
            found[0]
                .evidence_diff
                .contains("+The key was in the chapel.")
        );
        let blame = git.blame("manuscript/chapter.md", 2, 2).unwrap();
        assert_eq!(blame.len(), 1);
        assert_eq!(blame[0].commit, introduction);
        assert_eq!(blame[0].content, "The key was in the chapel.");
        assert!(git.search_text_history(" ", None, 2).is_err());
        assert!(git.blame("../project.toml", 1, 1).is_err());
    }
}
