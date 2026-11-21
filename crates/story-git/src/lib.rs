//! Local Git history operations. Callers provide the storage snapshot barrier.
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
        if !(path.starts_with("manuscript/")
            || path == "project.toml"
            || path == "snapshots/database.sql")
        {
            return Err(GitError::Command(
                "path is outside the versioned project content".into(),
            ));
        }
        let spec = format!("{commit}:{path}");
        Ok(self.output(&["show", &spec])?.stdout)
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
}
