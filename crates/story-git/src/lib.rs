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
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()?;
        check(output)
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
