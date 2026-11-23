use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    fs,
    path::{Path, PathBuf},
};
use story_core::*;
use story_storage::{
    ApprovedKnowledgeClaim, ConflictResolution, DiagnosticSuppressionRecord, ProjectStore,
    Result as StorageResult, SceneRecord, StorageError,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("invalid domain input: {0}")]
    Domain(String),
    #[error(transparent)]
    Pipeline(#[from] story_pipeline::PipelineError),
    #[error(transparent)]
    Git(#[from] story_git::GitError),
    #[error("user preferences error: {0}")]
    Preferences(String),
}
pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSymbolDefinition {
    pub entity_kind: String,
    pub entity_id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub source_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmacsTagsExport {
    pub symbols_markdown: String,
    pub tags_file: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompileBuildResult {
    pub configuration: String,
    pub format: String,
    pub status: String,
    pub outputs: Vec<String>,
    pub elapsed_millis: u128,
    pub warnings: Vec<String>,
    pub diagnostics: Vec<story_document::CompileDiagnostic>,
    pub log_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompileRunRecord {
    pub version: u16,
    #[serde(default)]
    pub run_id: String,
    pub configuration: String,
    #[serde(default)]
    pub format: Option<String>,
    pub succeeded: bool,
    pub created_unix_millis: u128,
    pub elapsed_millis: u128,
    pub outputs: Vec<String>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub diagnostics: Vec<story_document::CompileDiagnostic>,
    pub error: Option<String>,
    #[serde(default)]
    pub source_fingerprint: Option<String>,
    #[serde(skip)]
    pub stale: bool,
}

/// Optional, per-book chronology behavior. Story-time fields remain stored even
/// while the feature is disabled; disabling it only suppresses timeline analysis
/// and any optional missing-time prompts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChronologyPreferences {
    pub enabled: bool,
    pub prompt_for_missing_scene_time: bool,
    pub missing_scene_time_prompt_dismissed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ChronologyPreferencesFile {
    version: u16,
    preferences: ChronologyPreferences,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
struct ManagedCompileManifest {
    version: u16,
    configurations: std::collections::BTreeMap<String, Vec<ManagedCompileArtifact>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ManagedCompileArtifact {
    path: String,
    sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum CompilePublicationState {
    Prepared,
    Committed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum CompilePublicationPath {
    ProjectRelative(PathBuf),
    Absolute(PathBuf),
}

impl CompilePublicationPath {
    fn store(path: &Path, project_root: &Path) -> Self {
        match path.strip_prefix(project_root) {
            Ok(relative) => Self::ProjectRelative(relative.to_path_buf()),
            Err(_) => Self::Absolute(path.to_path_buf()),
        }
    }

    fn resolve(&self, project_root: &Path) -> PathBuf {
        match self {
            Self::ProjectRelative(path) => project_root.join(path),
            Self::Absolute(path) => path.clone(),
        }
    }

    fn validate(&self) -> Result<()> {
        match self {
            Self::ProjectRelative(path)
                if path.is_absolute()
                    || path
                        .components()
                        .any(|component| !matches!(component, std::path::Component::Normal(_))) =>
            {
                Err(AppError::Domain(
                    "Compile recovery journal contains an unsafe project-relative path".into(),
                ))
            }
            Self::Absolute(path) if !path.is_absolute() => Err(AppError::Domain(
                "Compile recovery journal contains a non-absolute external path".into(),
            )),
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct CompilePublicationFile {
    destination: CompilePublicationPath,
    staged: CompilePublicationPath,
    backup: CompilePublicationPath,
    previous_sha256: Option<String>,
    proposed_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct CompilePublicationJournal {
    version: u16,
    project_id: String,
    transaction_id: String,
    state: CompilePublicationState,
    files: Vec<CompilePublicationFile>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompilePublishFailpoint {
    IntentWritten,
    OutputPublished(usize),
    CommitWritten,
}

fn compile_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn file_sha256(path: &Path) -> Result<Option<String>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(compile_sha256(&bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AppError::Domain(format!(
            "read Compile recovery file {}: {error}",
            path.display()
        ))),
    }
}

fn sync_output_directory(path: &Path) -> Result<()> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            AppError::Domain(format!(
                "synchronize Compile output directory {}: {error}",
                path.display()
            ))
        })
}

fn compile_publication_journal_path(project_root: &Path) -> PathBuf {
    project_root.join(".writers-ide/recovery/compile-publication.json")
}

fn remove_compile_journal_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(AppError::Domain(format!(
                "remove Compile recovery file {}: {error}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn write_project_file_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Domain("project export path has no parent".into()))?;
    fs::create_dir_all(parent)
        .map_err(|error| AppError::Domain(format!("create project output directory: {error}")))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| AppError::Domain(format!("create symbol export temporary: {error}")))?;
    use std::io::Write;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|error| AppError::Domain(format!("write symbol export: {error}")))?;
    temporary
        .persist(path)
        .map_err(|error| AppError::Domain(format!("publish symbol export: {}", error.error)))?;
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| AppError::Domain(format!("sync symbol export directory: {error}")))?;
    Ok(())
}

fn create_project_file_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Domain("project file path has no parent".into()))?;
    fs::create_dir_all(parent)
        .map_err(|error| AppError::Domain(format!("create project directory: {error}")))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| AppError::Domain(format!("create project file temporary: {error}")))?;
    use std::io::Write;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|error| AppError::Domain(format!("write project file: {error}")))?;
    temporary.persist_noclobber(path).map_err(|error| {
        AppError::Domain(format!(
            "create project file without replacement: {}",
            error.error
        ))
    })?;
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| AppError::Domain(format!("sync project directory: {error}")))?;
    Ok(())
}

fn validate_compile_configuration_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 48
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || name.starts_with('-')
        || name.ends_with('-')
    {
        return Err(AppError::Domain(
            "compile configuration name must be a lowercase slug (letters, digits, hyphens)".into(),
        ));
    }
    Ok(())
}

fn validate_managed_compile_output(name: &str, path: &str) -> Result<()> {
    let required_prefix = format!("exports/compile/{name}/");
    let candidate = Path::new(path);
    if !path.starts_with(&required_prefix)
        || candidate
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(AppError::Domain(format!(
            "compile output must be inside {required_prefix}"
        )));
    }
    Ok(())
}

fn reject_project_symlink_components(root: &Path, relative: &str) -> Result<()> {
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        let std::path::Component::Normal(part) = component else {
            return Err(AppError::Domain(
                "invalid project-relative compile path".into(),
            ));
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(AppError::Domain(format!(
                    "compile output path contains a symlink: {relative}"
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(AppError::Domain(format!(
                    "inspect compile output path {relative}: {error}"
                )));
            }
        }
    }
    Ok(())
}

fn reject_absolute_symlink_components(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(AppError::Domain(
            "Compile output path must be absolute".into(),
        ));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(AppError::Domain(format!(
                    "Compile output path contains a symlink: {}",
                    path.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(AppError::Domain(format!(
                    "inspect Compile output path {}: {error}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

fn compile_source_map_bytes(
    destination: &Path,
    format: story_document::CompileFormat,
    entries: &[story_document::CompileSourceMapEntry],
) -> Result<Vec<u8>> {
    serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": 1,
        "output": destination.file_name().and_then(|name| name.to_str()).unwrap_or("compiled"),
        "output_format": format,
        "offset_space": "compile-ir-markdown-utf8",
        "visual_layout_mapping": false,
        "entries": entries,
    }))
    .map_err(|error| AppError::Domain(format!("serialize compile source map: {error}")))
}

fn publish_compile_file_set(
    project_root: &Path,
    project_id: ProjectId,
    files: &[(PathBuf, Vec<u8>)],
) -> Result<()> {
    publish_compile_file_set_with_failpoint(project_root, project_id, files, None)
}

fn publish_compile_file_set_with_failpoint(
    project_root: &Path,
    project_id: ProjectId,
    files: &[(PathBuf, Vec<u8>)],
    failpoint: Option<CompilePublishFailpoint>,
) -> Result<()> {
    let project_root = project_root
        .canonicalize()
        .map_err(|error| AppError::Domain(format!("resolve project root: {error}")))?;
    let transaction_id = uuid::Uuid::new_v4().to_string();
    let mut journal_files = Vec::with_capacity(files.len());
    let mut touched_directories = std::collections::BTreeSet::new();

    for (index, (destination, bytes)) in files.iter().enumerate() {
        let parent = destination
            .parent()
            .ok_or_else(|| AppError::Domain("Compile output has no parent directory".into()))?;
        fs::create_dir_all(parent).map_err(|error| {
            AppError::Domain(format!("create Compile output directory: {error}"))
        })?;
        let parent = parent.canonicalize().map_err(|error| {
            AppError::Domain(format!("resolve Compile output directory: {error}"))
        })?;
        let file_name = destination
            .file_name()
            .ok_or_else(|| AppError::Domain("Compile output has no file name".into()))?;
        let destination = parent.join(file_name);
        let (previous_sha256, previous_exists) = match fs::symlink_metadata(&destination) {
            Ok(metadata) if metadata.is_file() => (
                Some(compile_sha256(&fs::read(&destination).map_err(
                    |error| {
                        AppError::Domain(format!("read prior Compile output for recovery: {error}"))
                    },
                )?)),
                true,
            ),
            Ok(_) => {
                return Err(AppError::Domain(format!(
                    "Compile output is not a regular file: {}",
                    destination.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (None, false),
            Err(error) => {
                return Err(AppError::Domain(format!(
                    "inspect prior Compile output: {error}"
                )));
            }
        };
        let stage_path = parent.join(format!(".writers-ide-compile-{transaction_id}-{index}.new"));
        let backup_path = parent.join(format!(".writers-ide-compile-{transaction_id}-{index}.old"));
        use std::io::Write;
        let mut staged = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage_path)
            .map_err(|error| AppError::Domain(format!("stage Compile output: {error}")))?;
        staged
            .write_all(bytes)
            .and_then(|()| staged.sync_all())
            .map_err(|error| AppError::Domain(format!("write staged Compile output: {error}")))?;
        if previous_exists {
            fs::copy(&destination, &backup_path).map_err(|error| {
                AppError::Domain(format!("stage Compile rollback copy: {error}"))
            })?;
            fs::File::open(&backup_path)
                .and_then(|backup| backup.sync_all())
                .map_err(|error| {
                    AppError::Domain(format!("sync Compile rollback copy: {error}"))
                })?;
        }
        sync_output_directory(&parent)?;
        touched_directories.insert(parent.clone());
        journal_files.push(CompilePublicationFile {
            destination: CompilePublicationPath::store(&destination, &project_root),
            staged: CompilePublicationPath::store(&stage_path, &project_root),
            backup: CompilePublicationPath::store(&backup_path, &project_root),
            previous_sha256,
            proposed_sha256: compile_sha256(bytes),
        });
    }

    let journal_path = compile_publication_journal_path(&project_root);
    reject_project_symlink_components(
        &project_root,
        ".writers-ide/recovery/compile-publication.json",
    )?;
    if journal_path.exists() {
        return Err(AppError::Domain(
            "a Compile output recovery journal already exists; inspect it before publishing".into(),
        ));
    }
    let mut journal = CompilePublicationJournal {
        version: 1,
        project_id: project_id.to_string(),
        transaction_id: transaction_id.clone(),
        state: CompilePublicationState::Prepared,
        files: journal_files,
    };
    let serialized = serde_json::to_vec_pretty(&journal)
        .map_err(|error| AppError::Domain(format!("serialize Compile recovery intent: {error}")))?;
    write_project_file_atomically(&journal_path, &serialized)?;
    if failpoint == Some(CompilePublishFailpoint::IntentWritten) {
        return Err(AppError::Domain(
            "injected failure after Compile recovery intent".into(),
        ));
    }

    for (index, file) in journal.files.iter().enumerate() {
        let destination = file.destination.resolve(&project_root);
        let staged = file.staged.resolve(&project_root);
        fs::rename(&staged, &destination).map_err(|error| {
            AppError::Domain(format!(
                "publish Compile output {} with recovery journal retained: {error}",
                destination.display()
            ))
        })?;
        let parent = destination
            .parent()
            .ok_or_else(|| AppError::Domain("Compile output has no parent directory".into()))?;
        sync_output_directory(parent)?;
        if failpoint == Some(CompilePublishFailpoint::OutputPublished(index)) {
            return Err(AppError::Domain(format!(
                "injected failure after Compile output {index}; recovery journal retained"
            )));
        }
    }

    journal.state = CompilePublicationState::Committed;
    let serialized = serde_json::to_vec_pretty(&journal)
        .map_err(|error| AppError::Domain(format!("serialize Compile commit record: {error}")))?;
    write_project_file_atomically(&journal_path, &serialized)?;
    if failpoint == Some(CompilePublishFailpoint::CommitWritten) {
        return Err(AppError::Domain(
            "injected failure after Compile commit record; cleanup journal retained".into(),
        ));
    }
    for file in &journal.files {
        remove_compile_journal_file(&file.backup.resolve(&project_root))?;
        remove_compile_journal_file(&file.staged.resolve(&project_root))?;
    }
    for directory in touched_directories {
        sync_output_directory(&directory)?;
    }
    remove_compile_journal_file(&journal_path)?;
    sync_output_directory(journal_path.parent().unwrap())?;
    Ok(())
}

fn recover_compile_file_set(project_root: &Path, project_id: ProjectId) -> Result<Option<String>> {
    let project_root = project_root
        .canonicalize()
        .map_err(|error| AppError::Domain(format!("resolve project root: {error}")))?;
    let journal_path = compile_publication_journal_path(&project_root);
    reject_project_symlink_components(
        &project_root,
        ".writers-ide/recovery/compile-publication.json",
    )?;
    let journal = match fs::read(&journal_path) {
        Ok(bytes) => {
            serde_json::from_slice::<CompilePublicationJournal>(&bytes).map_err(|error| {
                AppError::Domain(format!("invalid Compile recovery journal: {error}"))
            })?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(AppError::Domain(format!(
                "read Compile recovery journal: {error}"
            )));
        }
    };
    if journal.version != 1 || journal.files.is_empty() {
        return Err(AppError::Domain(
            "unsupported or empty Compile recovery journal".into(),
        ));
    }
    if journal.project_id != project_id.to_string() {
        return Err(AppError::Domain(
            "Compile recovery journal belongs to a different project identity".into(),
        ));
    }
    let transaction = uuid::Uuid::parse_str(&journal.transaction_id)
        .map_err(|_| AppError::Domain("invalid Compile recovery transaction ID".into()))?;
    let transaction = transaction.to_string();
    let mut resolved = Vec::with_capacity(journal.files.len());
    for (index, file) in journal.files.iter().enumerate() {
        file.destination.validate()?;
        file.staged.validate()?;
        file.backup.validate()?;
        if let CompilePublicationPath::ProjectRelative(relative) = &file.destination
            && (relative.starts_with("manuscript")
                || relative.starts_with(".writers-ide")
                || relative == Path::new("project.toml")
                || relative == Path::new("story.db"))
        {
            return Err(AppError::Domain(
                "Compile recovery journal targets protected project data".into(),
            ));
        }
        let destination = file.destination.resolve(&project_root);
        let staged = file.staged.resolve(&project_root);
        let backup = file.backup.resolve(&project_root);
        let parent = destination
            .parent()
            .ok_or_else(|| AppError::Domain("Compile recovery target has no parent".into()))?;
        if staged.parent() != Some(parent)
            || backup.parent() != Some(parent)
            || staged.file_name().and_then(|name| name.to_str())
                != Some(format!(".writers-ide-compile-{transaction}-{index}.new").as_str())
            || backup.file_name().and_then(|name| name.to_str())
                != Some(format!(".writers-ide-compile-{transaction}-{index}.old").as_str())
        {
            return Err(AppError::Domain(
                "Compile recovery journal paths are invalid".into(),
            ));
        }
        reject_absolute_symlink_components(&destination)?;
        resolved.push((file, destination, staged, backup));
    }

    let mut conflicts = Vec::new();
    for (index, (file, destination, _, backup)) in resolved.iter().enumerate() {
        let current = file_sha256(destination)?;
        match journal.state {
            CompilePublicationState::Prepared => {
                let is_previous = current == file.previous_sha256;
                let is_proposed = current.as_deref() == Some(file.proposed_sha256.as_str());
                if !is_previous && !is_proposed {
                    conflicts.push(format!("output {index}: {}", destination.display()));
                } else if is_proposed
                    && file.previous_sha256.is_some()
                    && file_sha256(backup)?.as_deref() != file.previous_sha256.as_deref()
                {
                    conflicts.push(format!(
                        "missing or changed rollback copy {index}: {}",
                        backup.display()
                    ));
                }
            }
            CompilePublicationState::Committed => {
                if current.as_deref() != Some(file.proposed_sha256.as_str()) {
                    conflicts.push(format!(
                        "committed output changed {index}: {}",
                        destination.display()
                    ));
                }
            }
        }
    }
    if !conflicts.is_empty() {
        return Ok(Some(format!(
            "Compile output recovery found external or conflicting versions; all versions were preserved. Review {0} and targets: {1}",
            journal_path.display(),
            conflicts.join(", ")
        )));
    }

    match journal.state {
        CompilePublicationState::Prepared => {
            for (file, destination, _, backup) in &resolved {
                let current = file_sha256(destination)?;
                if current == file.previous_sha256 {
                    continue;
                }
                if file.previous_sha256.is_some() {
                    let previous = fs::read(backup).map_err(|error| {
                        AppError::Domain(format!("read previous Compile output: {error}"))
                    })?;
                    write_project_file_atomically(destination, &previous)?;
                } else {
                    remove_compile_journal_file(destination)?;
                }
                sync_output_directory(destination.parent().unwrap())?;
            }
        }
        CompilePublicationState::Committed => {}
    }
    let mut cleanup_directories = std::collections::BTreeSet::new();
    for (_, _, staged, backup) in &resolved {
        remove_compile_journal_file(staged)?;
        remove_compile_journal_file(backup)?;
        if let Some(parent) = staged.parent() {
            cleanup_directories.insert(parent.to_path_buf());
        }
    }
    for directory in cleanup_directories {
        sync_output_directory(&directory)?;
    }
    remove_compile_journal_file(&journal_path)?;
    sync_output_directory(journal_path.parent().unwrap())?;
    Ok(None)
}

fn copy_scrivener_source_tree(source: &Path, destination: &Path) -> Result<()> {
    fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
        fs::create_dir(destination).map_err(|error| {
            AppError::Domain(format!("create Scrivener source snapshot: {error}"))
        })?;
        let mut entries = fs::read_dir(source)
            .map_err(|error| AppError::Domain(format!("read Scrivener source: {error}")))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| AppError::Domain(format!("read Scrivener source entry: {error}")))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let source_path = entry.path();
            let target_path = destination.join(entry.file_name());
            let kind = entry.file_type().map_err(|error| {
                AppError::Domain(format!("inspect Scrivener source entry: {error}"))
            })?;
            if kind.is_symlink() {
                return Err(AppError::Domain(format!(
                    "Scrivener source snapshots do not copy symlinks: {}",
                    source_path.display()
                )));
            }
            if kind.is_dir() {
                copy_tree(&source_path, &target_path)?;
            } else if kind.is_file() {
                fs::copy(&source_path, &target_path).map_err(|error| {
                    AppError::Domain(format!("copy Scrivener source file: {error}"))
                })?;
                fs::File::open(&target_path)
                    .and_then(|file| file.sync_all())
                    .map_err(|error| {
                        AppError::Domain(format!("sync Scrivener source snapshot: {error}"))
                    })?;
            } else {
                return Err(AppError::Domain(format!(
                    "unsupported Scrivener source entry: {}",
                    source_path.display()
                )));
            }
        }
        fs::File::open(destination)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                AppError::Domain(format!("sync Scrivener snapshot directory: {error}"))
            })?;
        Ok(())
    }

    if destination.exists() {
        return Err(AppError::Domain(format!(
            "Scrivener snapshot destination already exists: {}",
            destination.display()
        )));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| AppError::Domain("Scrivener snapshot has no parent directory".into()))?;
    fs::create_dir_all(parent)
        .map_err(|error| AppError::Domain(format!("create import reference directory: {error}")))?;
    if let Err(error) = copy_tree(source, destination) {
        let _ = fs::remove_dir_all(destination);
        return Err(error);
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct UserPreferences {
    /// Interface language is independent of manuscript language.
    pub ui_language: String,
    /// Applied to projects created through the default project creation flow.
    pub default_writing_language: String,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            ui_language: "en".into(),
            default_writing_language: default_language_code(),
        }
    }
}

fn user_preferences_path() -> Option<PathBuf> {
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(config_home).join("writers-ide/preferences.toml"));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config/writers-ide/preferences.toml"))
}

fn read_user_preferences_at(path: &Path) -> Result<UserPreferences> {
    let preferences = match fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text)
            .map_err(|error| AppError::Preferences(format!("{}: {error}", path.display())))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => UserPreferences::default(),
        Err(error) => {
            return Err(AppError::Preferences(format!(
                "{}: {error}",
                path.display()
            )));
        }
    };
    if !is_valid_language_tag(&preferences.ui_language)
        || !is_valid_language_tag(&preferences.default_writing_language)
    {
        return Err(AppError::Preferences(format!(
            "{} contains an invalid BCP 47 language tag",
            path.display()
        )));
    }
    Ok(preferences)
}

fn write_user_preferences_at(path: &Path, preferences: &UserPreferences) -> Result<()> {
    if !is_valid_language_tag(&preferences.ui_language)
        || !is_valid_language_tag(&preferences.default_writing_language)
    {
        return Err(AppError::Domain(
            "preferences require valid BCP 47 language tags".into(),
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Preferences("preferences path has no parent".into()))?;
    fs::create_dir_all(parent).map_err(|error| AppError::Preferences(error.to_string()))?;
    let temporary = path.with_extension("toml.tmp");
    let bytes = toml::to_string_pretty(preferences)
        .map_err(|error| AppError::Preferences(error.to_string()))?;
    use std::io::Write;
    let mut file =
        fs::File::create(&temporary).map_err(|error| AppError::Preferences(error.to_string()))?;
    file.write_all(bytes.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| AppError::Preferences(error.to_string()))?;
    fs::rename(&temporary, path).map_err(|error| AppError::Preferences(error.to_string()))?;
    Ok(())
}

fn escape_svg_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if matches!(ch, '\t' | '\n' | '\r')
                || (ch >= '\u{20}' && ch != '\u{fffe}' && ch != '\u{ffff}')
            {
                ch
            } else {
                '\u{fffd}'
            }
        })
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}

/// Application façade: frontends use use cases here and never issue domain SQL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityMentionAppearance {
    pub scene_id: SceneId,
    pub scene_title: String,
    pub mention: story_storage::DetectedMentionRecord,
}

pub struct WriterApp {
    store: ProjectStore,
    compile_recovery_notice: RefCell<Option<String>>,
}
impl WriterApp {
    pub fn user_preferences() -> Result<UserPreferences> {
        match user_preferences_path() {
            Some(path) => read_user_preferences_at(&path),
            None => Ok(UserPreferences::default()),
        }
    }
    pub fn save_user_preferences(preferences: &UserPreferences) -> Result<()> {
        let path = user_preferences_path().ok_or_else(|| {
            AppError::Preferences("HOME and XDG_CONFIG_HOME are both unset".into())
        })?;
        write_user_preferences_at(&path, preferences)
    }
    pub fn set_default_writing_language(language_code: &str) -> Result<()> {
        let mut preferences = Self::user_preferences()?;
        preferences.default_writing_language = language_code.into();
        Self::save_user_preferences(&preferences)
    }
    pub fn project_root(&self) -> &Path {
        self.store.project_root()
    }
    pub fn create_project(path: impl AsRef<Path>, title: &str) -> Result<Self> {
        let preferences = Self::user_preferences()?;
        Self::create_project_with_preferences(path, title, &preferences)
    }
    pub fn create_project_with_preferences(
        path: impl AsRef<Path>,
        title: &str,
        preferences: &UserPreferences,
    ) -> Result<Self> {
        if !is_valid_language_tag(&preferences.default_writing_language) {
            return Err(AppError::Domain(
                "default writing language must be a valid BCP 47 language tag".into(),
            ));
        }
        let project = Project {
            id: ProjectId::new(),
            title: title.into(),
            author: String::new(),
            genre: String::new(),
            synopsis: String::new(),
            book_type: BookType::Fiction,
            template_id: None,
            target_words: None,
            story_epoch_label: "Day 0".into(),
            story_calendar_id: None,
            language_code: preferences.default_writing_language.clone(),
            word_count_policy: WordCountPolicy::default(),
        };
        Self::create_project_with_metadata(path, &project)
    }
    pub fn create_project_with_type(
        path: impl AsRef<Path>,
        title: &str,
        book_type: BookType,
    ) -> Result<Self> {
        let project = Project {
            id: ProjectId::new(),
            title: title.into(),
            author: String::new(),
            genre: String::new(),
            synopsis: String::new(),
            book_type,
            template_id: None,
            target_words: None,
            story_epoch_label: "Day 0".into(),
            story_calendar_id: None,
            language_code: Self::user_preferences()?.default_writing_language,
            word_count_policy: WordCountPolicy::default(),
        };
        Self::create_project_with_metadata(path, &project)
    }
    pub fn create_project_from_template(
        path: impl AsRef<Path>,
        title: &str,
        template: BuiltinTemplate,
    ) -> Result<Self> {
        Self::create_project_from_definition(path, title, &template.definition())
    }
    /// Create a project from a validated declarative template in a sibling
    /// staging directory, then publish it with one directory rename.
    pub fn create_project_from_definition(
        path: impl AsRef<Path>,
        title: &str,
        template: &ProjectTemplate,
    ) -> Result<Self> {
        template.validate().map_err(AppError::Domain)?;
        let path = path.as_ref();
        if path.exists() {
            return Err(AppError::Domain(
                "project destination already exists".into(),
            ));
        }
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|error| AppError::Domain(format!("create project parent: {error}")))?;
        let staging_root = tempfile::tempdir_in(parent).map_err(|error| {
            AppError::Domain(format!("create project staging directory: {error}"))
        })?;
        let staged = staging_root.path().join("project");
        let project = Project {
            id: ProjectId::new(),
            title: title.into(),
            author: String::new(),
            genre: String::new(),
            synopsis: String::new(),
            book_type: template.book_type,
            template_id: Some(template.id.clone()),
            target_words: template.target_words,
            story_epoch_label: "Day 0".into(),
            story_calendar_id: None,
            language_code: Self::user_preferences()?.default_writing_language,
            word_count_policy: WordCountPolicy::default(),
        };
        let mut app = Self::create_project_with_metadata(&staged, &project)?;
        let mut node_ids = std::collections::BTreeMap::new();
        let mut pending = template.nodes.clone();
        while !pending.is_empty() {
            let before = pending.len();
            let mut next = Vec::new();
            for node in pending {
                let parent = node
                    .parent_key
                    .as_deref()
                    .and_then(|key| node_ids.get(key).copied());
                if node.parent_key.is_some() && parent.is_none() {
                    next.push(node);
                    continue;
                }
                let id =
                    app.create_structural_node(parent, node.kind, &node.title, node.position)?;
                node_ids.insert(node.key, id);
            }
            if next.len() == before {
                return Err(AppError::Domain(
                    "template structural nodes cannot be ordered".into(),
                ));
            }
            pending = next;
        }
        for unit in &template.content_units {
            let parent = unit
                .parent_key
                .as_deref()
                .and_then(|key| node_ids.get(key).copied());
            app.create_content_unit(
                unit.kind,
                &unit.title,
                parent,
                unit.position,
                unit.target_words,
            )?;
        }
        for scene in &template.scenes {
            let chapter = node_ids.get(&scene.chapter_key).copied().ok_or_else(|| {
                AppError::Domain(format!(
                    "template scene chapter {} is missing",
                    scene.chapter_key
                ))
            })?;
            let id = app.create_scene(chapter, &scene.title, scene.position)?;
            if let Some(words) = scene.target_words {
                let mut metadata = app.get_scene(id)?.scene;
                metadata.target_words = Some(words);
                app.update_scene_metadata(&metadata)?;
            }
        }
        for field in &template.custom_fields {
            app.define_custom_field(&CustomFieldDefinition {
                id: FieldId::new(),
                name: field.name.clone(),
                field_type: field.field_type,
                required: field.required,
                default_value: field.default_value.clone(),
                enum_values: field.enum_values.clone(),
                reference_kind: field.reference_kind.clone(),
            })?;
        }
        app.validate_project()?;
        drop(app);
        fs::rename(&staged, path)
            .map_err(|error| AppError::Domain(format!("publish created project: {error}")))?;
        Self::open_project(path)
    }

    pub fn create_structural_node(
        &mut self,
        parent: Option<NodeId>,
        kind: NodeKind,
        title: &str,
        position: u32,
    ) -> Result<NodeId> {
        Ok(self.store.create_node(parent, kind, title, position)?)
    }

    pub fn create_project_with_metadata(path: impl AsRef<Path>, project: &Project) -> Result<Self> {
        Ok(Self {
            store: ProjectStore::create_for(path, project)?,
            compile_recovery_notice: RefCell::new(None),
        })
    }
    pub fn get_project(&self) -> Result<Project> {
        Ok(self.store.get_project()?)
    }
    pub fn chronology_preferences(&self) -> Result<ChronologyPreferences> {
        let path = self
            .store
            .project_root()
            .join(".writers-ide/chronology.toml");
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ChronologyPreferences::default());
            }
            Err(error) => {
                return Err(AppError::Preferences(format!(
                    "read chronology preferences: {error}"
                )));
            }
        };
        let file: ChronologyPreferencesFile = toml::from_str(&source).map_err(|error| {
            AppError::Preferences(format!("invalid chronology preferences: {error}"))
        })?;
        if file.version != 1 {
            return Err(AppError::Preferences(format!(
                "unsupported chronology preferences version {}",
                file.version
            )));
        }
        Ok(file.preferences)
    }
    pub fn set_chronology_preferences(&mut self, preferences: ChronologyPreferences) -> Result<()> {
        let file = ChronologyPreferencesFile {
            version: 1,
            preferences,
        };
        let source = toml::to_string_pretty(&file).map_err(|error| {
            AppError::Preferences(format!("serialize chronology preferences: {error}"))
        })?;
        write_project_file_atomically(
            &self
                .store
                .project_root()
                .join(".writers-ide/chronology.toml"),
            source.as_bytes(),
        )
    }
    pub fn update_scene_story_time(
        &mut self,
        id: SceneId,
        start: StoryTime,
        end: StoryTime,
    ) -> Result<()> {
        let (start_boundary, end_boundary) = self.store.scene_interval_boundaries(id)?;
        self.update_scene_story_interval(
            id,
            StoryInterval {
                start,
                end,
                start_boundary,
                end_boundary,
            },
        )
    }
    pub fn update_scene_story_interval(
        &mut self,
        id: SceneId,
        interval: StoryInterval,
    ) -> Result<()> {
        Ok(self.store.update_scene_story_interval(id, interval)?)
    }
    pub fn scene_story_interval(&self, id: SceneId) -> Result<StoryInterval> {
        let scene = self.get_scene(id)?.scene;
        let (start_boundary, end_boundary) = self.store.scene_interval_boundaries(id)?;
        Ok(StoryInterval {
            start: scene.start_time,
            end: scene.end_time,
            start_boundary,
            end_boundary,
        })
    }
    pub fn should_prompt_for_missing_scene_time(&self, id: SceneId) -> Result<bool> {
        let preferences = self.chronology_preferences()?;
        if !preferences.enabled
            || !preferences.prompt_for_missing_scene_time
            || preferences.missing_scene_time_prompt_dismissed
        {
            return Ok(false);
        }
        let scene = self.get_scene(id)?.scene;
        Ok(scene.start_time.precision == TimePrecision::Unknown
            && scene.end_time.precision == TimePrecision::Unknown)
    }
    pub fn set_project_language(&mut self, language_code: &str) -> Result<()> {
        Ok(self.store.set_project_language(language_code)?)
    }
    pub fn set_scene_language(
        &mut self,
        scene_id: SceneId,
        language_code: Option<&str>,
    ) -> Result<()> {
        Ok(self.store.set_scene_language(scene_id, language_code)?)
    }
    pub fn tag_passage_language(
        &mut self,
        scene_id: SceneId,
        start_byte: u64,
        end_byte: u64,
        language_code: &str,
    ) -> Result<AnchorId> {
        Ok(self
            .store
            .tag_passage_language(scene_id, start_byte, end_byte, language_code)?)
    }
    pub fn list_passage_language_tags(
        &self,
        scene_id: SceneId,
    ) -> Result<Vec<story_storage::PassageLanguageRecord>> {
        Ok(self.store.list_passage_language_tags(scene_id)?)
    }
    pub fn remove_passage_language_tag(&mut self, anchor_id: AnchorId) -> Result<()> {
        Ok(self.store.remove_passage_language_tag(anchor_id)?)
    }
    pub fn set_project_word_count_policy(&mut self, policy: WordCountPolicy) -> Result<()> {
        Ok(self.store.set_project_word_count_policy(policy)?)
    }
    pub fn open_project(path: impl AsRef<Path>) -> Result<Self> {
        let store = ProjectStore::open(path)?;
        let recovery_notice =
            recover_compile_file_set(store.project_root(), store.get_project()?.id)?;
        Ok(Self {
            store,
            compile_recovery_notice: RefCell::new(recovery_notice),
        })
    }

    pub fn compile_recovery_notice(&self) -> Option<String> {
        self.compile_recovery_notice.borrow().clone()
    }

    /// Recheck the durable compile-output journal after the author resolves a
    /// reported conflict in the preserved target or recovery files.
    pub fn retry_compile_publication_recovery(&self) -> Result<Option<String>> {
        let notice = recover_compile_file_set(self.store.project_root(), self.get_project()?.id)?;
        *self.compile_recovery_notice.borrow_mut() = notice.clone();
        Ok(notice)
    }
    pub fn create_section(&mut self, title: &str, position: u32) -> Result<NodeId> {
        Ok(self
            .store
            .create_node(None, NodeKind::Section, title, position)?)
    }
    pub fn create_chapter(
        &mut self,
        parent: Option<NodeId>,
        title: &str,
        position: u32,
    ) -> Result<NodeId> {
        Ok(self
            .store
            .create_node(parent, NodeKind::Chapter, title, position)?)
    }
    pub fn move_structural_node(
        &mut self,
        node: NodeId,
        parent: Option<NodeId>,
        position: u32,
    ) -> Result<()> {
        Ok(self.store.move_node(node, parent, position)?)
    }
    pub fn create_scene(&mut self, chapter: NodeId, title: &str, position: u32) -> Result<SceneId> {
        Ok(self.store.create_scene(chapter, position, title)?)
    }
    pub fn create_unplaced_scene(&mut self, title: &str) -> Result<SceneId> {
        Ok(self.store.create_unplaced_scene(title)?)
    }
    pub fn create_content_unit(
        &mut self,
        kind: ContentUnitKind,
        title: &str,
        parent: Option<NodeId>,
        position: u32,
        target_words: Option<u64>,
    ) -> Result<ContentUnitId> {
        Ok(self
            .store
            .create_content_unit(kind, title, parent, position, target_words)?)
    }
    pub fn get_content_unit(&self, id: ContentUnitId) -> Result<story_storage::ContentUnitRecord> {
        Ok(self.store.get_content_unit(id)?)
    }
    pub fn set_content_unit_language(
        &mut self,
        id: ContentUnitId,
        language_code: Option<&str>,
    ) -> Result<()> {
        Ok(self.store.set_content_unit_language(id, language_code)?)
    }
    pub fn list_content_units(
        &self,
        include_archived: bool,
    ) -> Result<Vec<story_storage::ContentUnitRecord>> {
        Ok(self.store.list_content_units(include_archived)?)
    }
    pub fn load_content_unit_text(&self, id: ContentUnitId) -> Result<(String, String)> {
        Ok(self.store.load_content_unit_text(id)?)
    }
    pub fn save_content_unit_text(
        &mut self,
        id: ContentUnitId,
        expected: Option<&str>,
        text: &str,
    ) -> Result<String> {
        Ok(self.store.save_content_unit_text(id, expected, text)?)
    }
    pub fn rename_content_unit(&mut self, id: ContentUnitId, title: &str) -> Result<()> {
        Ok(self.store.rename_content_unit(id, title)?)
    }
    pub fn list_structural_nodes(&self) -> Result<Vec<story_storage::StructuralNodeSummary>> {
        Ok(self.store.list_structural_nodes()?)
    }
    /// Capture the current document structure as a reusable, declarative
    /// template. Canonical prose, project identity, attachments, research and
    /// other project-specific facts are deliberately excluded.
    pub fn project_template_from_current(&self, id: &str, name: &str) -> Result<ProjectTemplate> {
        let project = self.get_project()?;
        let structural_nodes = self.list_structural_nodes()?;
        let node_keys = structural_nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id, format!("node-{:04}", index + 1)))
            .collect::<std::collections::BTreeMap<_, _>>();
        let field_definitions = self.store.list_custom_field_definitions()?;
        if let Some(field) = field_definitions.iter().find(|field| {
            matches!(
                field.field_type,
                FieldType::EntityRef | FieldType::EntityRefs
            ) && field.default_value.is_some()
        }) {
            return Err(AppError::Domain(format!(
                "cannot save template: entity-reference field {:?} has a project-specific default",
                field.name
            )));
        }
        let mut template = ProjectTemplate {
            schema_version: 1,
            template_revision: 1,
            id: id.into(),
            name: name.into(),
            book_type: project.book_type,
            target_words: project.target_words,
            nodes: Vec::new(),
            content_units: Vec::new(),
            scenes: Vec::new(),
            custom_fields: field_definitions
                .into_iter()
                .enumerate()
                .map(|(index, field)| ProjectTemplateCustomField {
                    key: format!("field-{:04}", index + 1),
                    name: field.name,
                    field_type: field.field_type,
                    required: field.required,
                    default_value: if matches!(
                        field.field_type,
                        FieldType::EntityRef | FieldType::EntityRefs
                    ) {
                        None
                    } else {
                        field.default_value
                    },
                    enum_values: field.enum_values,
                    reference_kind: field.reference_kind,
                })
                .collect(),
        };
        for (index, node) in structural_nodes.iter().enumerate() {
            let kind = match node.kind.as_str() {
                "section" => NodeKind::Section,
                "chapter" => NodeKind::Chapter,
                "group" => NodeKind::Group,
                _ => {
                    return Err(AppError::Domain(format!(
                        "cannot template unsupported structural node kind {}",
                        node.kind
                    )));
                }
            };
            let parent_key = node
                .parent_id
                .map(|parent| {
                    node_keys.get(&parent).cloned().ok_or_else(|| {
                        AppError::Domain(format!(
                            "structural node {} has a missing template parent",
                            node.id
                        ))
                    })
                })
                .transpose()?;
            template.nodes.push(ProjectTemplateNode {
                key: node_keys[&node.id].clone(),
                parent_key,
                kind,
                title: node.title.clone(),
                position: node.position,
            });
            debug_assert_eq!(index + 1, template.nodes.len());
        }
        let content_keys = self
            .list_content_units(false)?
            .into_iter()
            .enumerate()
            .map(|(index, unit)| (unit, format!("content-{:04}", index + 1)))
            .collect::<Vec<_>>();
        for (unit, key) in content_keys {
            let parent_key = unit
                .parent_id
                .map(|parent| {
                    node_keys.get(&parent).cloned().ok_or_else(|| {
                        AppError::Domain(format!(
                            "content unit {} has a missing template parent",
                            unit.id
                        ))
                    })
                })
                .transpose()?;
            template.content_units.push(ProjectTemplateContentUnit {
                key,
                parent_key,
                kind: unit.kind,
                title: unit.title,
                position: unit.position,
                target_words: unit.target_words,
            });
        }
        for (index, scene) in self.list_scenes(false)?.into_iter().enumerate() {
            let chapter_id = scene.chapter_id.ok_or_else(|| {
                AppError::Domain(format!(
                    "cannot save template: active scene {} has no chapter placement",
                    scene.scene.id
                ))
            })?;
            let chapter_key = node_keys.get(&chapter_id).ok_or_else(|| {
                AppError::Domain(format!(
                    "cannot save template: scene {} refers to a missing chapter",
                    scene.scene.id
                ))
            })?;
            if !structural_nodes
                .iter()
                .any(|node| node.id == chapter_id && node.kind == NodeKind::Chapter.as_str())
            {
                return Err(AppError::Domain(format!(
                    "cannot save template: scene {} is placed outside a chapter",
                    scene.scene.id
                )));
            }
            template.scenes.push(ProjectTemplateScene {
                key: format!("scene-{:04}", index + 1),
                chapter_key: chapter_key.clone(),
                title: scene.scene.title,
                position: scene.position.ok_or_else(|| {
                    AppError::Domain(format!(
                        "cannot save template: scene {} has no manuscript position",
                        scene.scene.id
                    ))
                })?,
                target_words: scene.scene.target_words,
            });
        }
        template.validate().map_err(AppError::Domain)?;
        Ok(template)
    }
    pub fn get_scene(&self, id: SceneId) -> Result<SceneRecord> {
        Ok(self.store.get_scene(id)?)
    }
    pub fn list_scenes(&self, include_archived: bool) -> Result<Vec<SceneRecord>> {
        Ok(self.store.list_scenes(include_archived)?)
    }
    pub fn scene_is_included(&self, id: SceneId) -> Result<bool> {
        Ok(self.store.scene_is_included(id)?)
    }
    pub fn set_scene_included(&mut self, id: SceneId, included: bool) -> Result<()> {
        Ok(self.store.set_scene_included(id, included)?)
    }
    pub fn list_excluded_scene_ids(&self) -> Result<Vec<SceneId>> {
        Ok(self.store.list_excluded_scene_ids()?)
    }
    pub fn list_project_symbols(&self) -> Result<Vec<ProjectSymbolDefinition>> {
        let mut symbols = Vec::new();
        for scene in self.list_scenes(false)? {
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "scene".into(),
                entity_id: scene.scene.id.to_string(),
                name: scene.scene.title,
                aliases: Vec::new(),
                source_path: Some(self.store.scene_content_relative_path(scene.scene.id)?),
            });
        }
        for node in self
            .list_structural_nodes()?
            .into_iter()
            .filter(|node| node.kind == "chapter")
        {
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "chapter".into(),
                entity_id: node.id.to_string(),
                name: node.title,
                aliases: Vec::new(),
                source_path: None,
            });
        }
        for character in self.list_characters()? {
            let record = self.get_character(character.id)?;
            let aliases = std::iter::once(record.full_name)
                .chain(std::iter::once(record.preferred_name))
                .chain(record.aliases)
                .filter(|alias| !alias.is_empty())
                .collect::<Vec<_>>();
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "character".into(),
                entity_id: character.id.to_string(),
                name: character.name,
                aliases,
                source_path: None,
            });
        }
        for location in self.list_locations()? {
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "location".into(),
                entity_id: location.id.to_string(),
                name: location.name,
                aliases: Vec::new(),
                source_path: None,
            });
        }
        for object in self.list_story_objects()? {
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "object".into(),
                entity_id: object.id.to_string(),
                name: object.name,
                aliases: Vec::new(),
                source_path: None,
            });
        }
        for plotline in self.list_plotlines()? {
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "plotline".into(),
                entity_id: plotline.id.to_string(),
                name: plotline.name,
                aliases: Vec::new(),
                source_path: None,
            });
        }
        for event in self.list_events()? {
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "event".into(),
                entity_id: event.id.to_string(),
                name: event.title,
                aliases: Vec::new(),
                source_path: None,
            });
        }
        for research in self.list_research_items()? {
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "research".into(),
                entity_id: research.id.to_string(),
                name: research.title,
                aliases: Vec::new(),
                source_path: None,
            });
        }
        symbols.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.entity_kind.cmp(&right.entity_kind))
                .then_with(|| left.entity_id.cmp(&right.entity_id))
        });
        Ok(symbols)
    }

    pub fn build_emacs_tags_export(&self) -> Result<EmacsTagsExport> {
        let symbols = self.list_project_symbols()?;
        let mut markdown = String::from(
            "# Generated Writer's IDE symbol index\n\nGenerated from stable project entity IDs. Re-export after project changes.\n",
        );
        let mut entries = Vec::new();
        for symbol in symbols {
            let clean = |value: &str| {
                value
                    .chars()
                    .map(|character| {
                        if character.is_control() {
                            ' '
                        } else {
                            character
                        }
                    })
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let name = clean(&symbol.name);
            if name.is_empty() {
                continue;
            }
            let heading = format!(
                "## {}: {} [{}]",
                clean(&symbol.entity_kind),
                name,
                clean(&symbol.entity_id)
            );
            let offset = markdown.len();
            let line = markdown.bytes().filter(|byte| *byte == b'\n').count() + 1;
            markdown.push_str(&heading);
            markdown.push('\n');
            markdown.push_str("Entity ID: `");
            markdown.push_str(&clean(&symbol.entity_id));
            markdown.push_str("`\n");
            let aliases = symbol
                .aliases
                .iter()
                .map(|alias| clean(alias))
                .filter(|alias| !alias.is_empty())
                .collect::<Vec<_>>();
            if !aliases.is_empty() {
                markdown.push_str("Aliases: ");
                markdown.push_str(
                    &aliases
                        .iter()
                        .map(|alias| format!("`{alias}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                markdown.push('\n');
            }
            if let Some(path) = symbol.source_path {
                markdown.push_str("Project source: `");
                markdown.push_str(&clean(&path.to_string_lossy()));
                markdown.push_str("`\n");
            } else {
                markdown.push_str("Definition: structured project record\n");
            }
            let mut tag_names = vec![name];
            tag_names.extend(aliases);
            tag_names.sort();
            tag_names.dedup();
            for tag_name in tag_names
                .into_iter()
                .filter(|tag_name| !tag_name.is_empty())
            {
                entries.push((heading.clone(), tag_name, line, offset));
            }
        }
        let mut tags_file = format!("\u{c}\nsymbols.md,{}\n", markdown.len());
        for (text, name, line, offset) in entries {
            tags_file.push_str(&text);
            tags_file.push('\u{7f}');
            tags_file.push_str(&name);
            tags_file.push('\u{1}');
            tags_file.push_str(&format!("{line},{offset}\n"));
        }
        Ok(EmacsTagsExport {
            symbols_markdown: markdown,
            tags_file,
        })
    }

    pub fn export_emacs_tags(&self) -> Result<PathBuf> {
        let root = self
            .project_root()
            .canonicalize()
            .map_err(|error| AppError::Domain(format!("open project for TAGS export: {error}")))?;
        let control = root.join(".writers-ide");
        fs::create_dir_all(&control).map_err(|error| {
            AppError::Domain(format!("create symbol export directory: {error}"))
        })?;
        let metadata = fs::symlink_metadata(&control).map_err(|error| {
            AppError::Domain(format!("inspect symbol export directory: {error}"))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(AppError::Domain(
                "symbol export directory must be a real project directory".into(),
            ));
        }
        let directory = control.join("tags");
        fs::create_dir_all(&directory)
            .map_err(|error| AppError::Domain(format!("create TAGS directory: {error}")))?;
        let metadata = fs::symlink_metadata(&directory)
            .map_err(|error| AppError::Domain(format!("inspect TAGS directory: {error}")))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(AppError::Domain(
                "symbol TAGS directory must be a real project directory".into(),
            ));
        }
        let directory = directory
            .canonicalize()
            .map_err(|error| AppError::Domain(format!("resolve TAGS directory: {error}")))?;
        if !directory.starts_with(&root) {
            return Err(AppError::Domain(
                "symbol export path escapes the project".into(),
            ));
        }
        let export = self.build_emacs_tags_export()?;
        write_project_file_atomically(
            &directory.join("symbols.md"),
            export.symbols_markdown.as_bytes(),
        )?;
        write_project_file_atomically(&directory.join("TAGS"), export.tags_file.as_bytes())?;
        Ok(directory.join("TAGS"))
    }
    pub fn analytics_dataset(&self) -> Result<Vec<story_analysis::SceneAnalysisInput>> {
        let mut result = Vec::new();
        let locations = self
            .store
            .list_locations()?
            .into_iter()
            .map(|item| (item.id.to_string(), item.name))
            .collect::<std::collections::HashMap<_, _>>();
        let events = self
            .store
            .list_events()?
            .into_iter()
            .map(|item| (item.id.to_string(), item.title))
            .collect::<std::collections::HashMap<_, _>>();
        for row in self.store.list_scenes(false)? {
            let characters = self
                .store
                .scene_character_analytics(row.scene.id)?
                .into_iter()
                .map(|(id, name, role)| {
                    let role = match role.as_str() {
                        "pov" => ParticipationRole::Pov,
                        "major" => ParticipationRole::Major,
                        "minor" => ParticipationRole::Minor,
                        "off_camera" => ParticipationRole::OffCamera,
                        _ => ParticipationRole::Mentioned,
                    };
                    story_analysis::CharacterOnScene { id, name, role }
                })
                .collect();
            let plotlines = self
                .store
                .scene_plotline_analytics(row.scene.id)?
                .into_iter()
                .map(|(id, name)| story_analysis::PlotlineOnScene { id, name })
                .collect();
            let mut location_ids = std::collections::BTreeSet::new();
            location_ids.extend(row.scene.start_location.map(|id| id.to_string()));
            location_ids.extend(row.scene.end_location.map(|id| id.to_string()));
            let locations = location_ids
                .into_iter()
                .map(|id| story_analysis::StoryEntityOnScene {
                    name: locations.get(&id).cloned().unwrap_or_else(|| id.clone()),
                    id,
                })
                .collect();
            let events = self
                .store
                .scene_event_links(row.scene.id)?
                .into_iter()
                .map(|(id, _kind)| id.to_string())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .map(|id| story_analysis::StoryEntityOnScene {
                    name: events.get(&id).cloned().unwrap_or_else(|| id.clone()),
                    id,
                })
                .collect();
            let (start_boundary, end_boundary) =
                self.store.scene_interval_boundaries(row.scene.id)?;
            result.push(story_analysis::SceneAnalysisInput {
                scene_id: row.scene.id.to_string(),
                title: row.scene.title,
                chapter_id: row.chapter_id.map(|id| id.to_string()),
                actual_words: row.actual_words,
                target_words: row.scene.target_words,
                start_time: row.scene.start_time,
                end_time: row.scene.end_time,
                start_boundary,
                end_boundary,
                characters,
                plotlines,
                locations,
                events,
            });
        }
        Ok(result)
    }
    pub fn processing_input(&self) -> Result<story_pipeline::PipelineInput> {
        self.processing_input_mode(false)
    }
    pub fn processing_input_with_transclusions(&self) -> Result<story_pipeline::PipelineInput> {
        self.processing_input_mode(true)
    }
    /// Render the current manuscript snapshot through the shared Compile IR.
    /// Non-Markdown formats require a local Pandoc installation; PDF also
    /// requires LuaLaTeX. No network access or source-file mutation occurs.
    pub fn compile_artifact(
        &self,
        format: story_document::CompileFormat,
        expand_transclusions: bool,
    ) -> Result<story_document::CompileArtifact> {
        self.compile_artifact_with_design(
            format,
            expand_transclusions,
            story_document::BookDesign::default(),
        )
    }
    pub fn compile_artifact_with_design(
        &self,
        format: story_document::CompileFormat,
        expand_transclusions: bool,
        design: story_document::BookDesign,
    ) -> Result<story_document::CompileArtifact> {
        self.compile_artifact_with_profile(format, expand_transclusions, design.profile())
    }
    pub fn compile_artifact_with_profile(
        &self,
        format: story_document::CompileFormat,
        expand_transclusions: bool,
        design: story_document::BookDesignProfile,
    ) -> Result<story_document::CompileArtifact> {
        self.compile_artifact_with_profile_and_metadata(
            format,
            expand_transclusions,
            design,
            &story_document::CompileMetadata::default(),
        )
    }

    pub fn compile_artifact_with_profile_and_metadata(
        &self,
        format: story_document::CompileFormat,
        expand_transclusions: bool,
        design: story_document::BookDesignProfile,
        metadata: &story_document::CompileMetadata,
    ) -> Result<story_document::CompileArtifact> {
        let input = self.processing_input_mode(expand_transclusions)?;
        let ir = story_pipeline::compile_ir(&input);
        let renderer = story_document::PandocRenderer::default();
        renderer
            .render_with_profile_and_metadata(
                &ir,
                format,
                Some(self.store.project_root()),
                &design,
                metadata,
            )
            .map_err(AppError::Domain)
    }

    /// Render a one-off Compile request and persist a result if rendering
    /// fails before an artifact exists. Successful publication is recorded by
    /// `publish_compile_artifact_with_sidecars`.
    pub fn compile_artifact_recording_failure(
        &self,
        format: story_document::CompileFormat,
        expand_transclusions: bool,
        design: story_document::BookDesignProfile,
    ) -> Result<story_document::CompileArtifact> {
        self.render_artifact_recording_failure("One-off Compile", format, || {
            self.compile_artifact_with_profile(format, expand_transclusions, design)
        })
    }

    /// Render an edition recipe and persist a result if validation or rendering
    /// fails before an artifact exists. Publication is recorded separately.
    pub fn compile_edition_artifact_recording_failure(
        &self,
        recipe: &story_document::EditionRecipe,
    ) -> Result<story_document::CompileArtifact> {
        self.render_artifact_recording_failure(&recipe.name, recipe.format, || {
            self.compile_artifact_for_edition(recipe)
        })
    }

    fn render_artifact_recording_failure<F>(
        &self,
        configuration: &str,
        format: story_document::CompileFormat,
        render: F,
    ) -> Result<story_document::CompileArtifact>
    where
        F: FnOnce() -> Result<story_document::CompileArtifact>,
    {
        use std::time::Instant;

        let timer = Instant::now();
        let result = render();
        if let Err(error) = &result {
            let diagnostics = self.compile_diagnostics().unwrap_or_default();
            let warnings = diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.severity != story_document::DiagnosticSeverity::Error
                })
                .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
                .collect();
            let record = CompileRunRecord {
                version: 6,
                run_id: uuid::Uuid::new_v4().to_string(),
                configuration: configuration.to_owned(),
                format: Some(format.extension().to_ascii_uppercase()),
                succeeded: false,
                created_unix_millis: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
                elapsed_millis: timer.elapsed().as_millis(),
                outputs: Vec::new(),
                warnings,
                diagnostics,
                error: Some(error.to_string()),
                source_fingerprint: self.compile_source_fingerprint().ok(),
                stale: false,
            };
            self.persist_compile_run_record(&record)?;
        }
        result
    }

    pub fn compile_artifact_for_assembly(
        &self,
        assembly: AssemblyId,
        format: story_document::CompileFormat,
        design: story_document::BookDesignProfile,
    ) -> Result<story_document::CompileArtifact> {
        self.compile_artifact_for_assembly_with_metadata(
            assembly,
            format,
            design,
            &story_document::CompileMetadata::default(),
        )
    }

    pub fn compile_artifact_for_assembly_with_metadata(
        &self,
        assembly: AssemblyId,
        format: story_document::CompileFormat,
        design: story_document::BookDesignProfile,
        metadata: &story_document::CompileMetadata,
    ) -> Result<story_document::CompileArtifact> {
        let ir = self.compile_assembly_ir(assembly)?;
        let renderer = story_document::PandocRenderer::default();
        renderer
            .render_with_profile_and_metadata(
                &ir,
                format,
                Some(self.store.project_root()),
                &design,
                metadata,
            )
            .map_err(AppError::Domain)
    }

    /// Render a complete project-local edition recipe through the same path
    /// used by managed builds and the CLI.
    pub fn compile_artifact_for_edition(
        &self,
        recipe: &story_document::EditionRecipe,
    ) -> Result<story_document::CompileArtifact> {
        recipe.validate().map_err(AppError::Domain)?;
        let metadata = story_document::CompileMetadata {
            title: recipe.metadata_title.clone(),
            author: recipe.metadata_author.clone(),
            cover_image: recipe.cover_image.clone(),
        };
        let design = recipe.design.profile();
        if let Some(assembly) = recipe.assembly_id.as_deref() {
            let assembly =
                AssemblyId(uuid::Uuid::parse_str(assembly).map_err(|error| {
                    AppError::Domain(format!("invalid assembly UUID: {error}"))
                })?);
            self.compile_artifact_for_assembly_with_metadata(
                assembly,
                recipe.format,
                design,
                &metadata,
            )
        } else {
            self.compile_artifact_with_profile_and_metadata(
                recipe.format,
                recipe.expand_transclusions,
                design,
                &metadata,
            )
        }
    }

    /// Publish a one-off Compile result without touching canonical project files.
    /// Replacement is refused unless the caller makes it explicit.
    pub fn publish_compile_artifact(
        &self,
        destination: impl AsRef<Path>,
        artifact: &story_document::CompileArtifact,
        overwrite: bool,
    ) -> Result<Vec<PathBuf>> {
        self.publish_compile_artifact_with_sidecars(destination, artifact, overwrite, &[])
    }

    /// Publish a Compile artifact and additional named sidecars as one staged
    /// file set. Sidecar names are restricted to filenames beside the artifact.
    pub fn publish_compile_artifact_with_sidecars(
        &self,
        destination: impl AsRef<Path>,
        artifact: &story_document::CompileArtifact,
        overwrite: bool,
        sidecars: &[(String, Vec<u8>)],
    ) -> Result<Vec<PathBuf>> {
        use std::time::Instant;
        let timer = Instant::now();
        let result = self.publish_compile_artifact_with_sidecars_inner(
            destination,
            artifact,
            overwrite,
            sidecars,
        );
        let elapsed_millis = timer.elapsed().as_millis();
        let (succeeded, outputs, error) = match &result {
            Ok(paths) => (
                true,
                paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect(),
                None,
            ),
            Err(error) => (false, Vec::new(), Some(error.to_string())),
        };
        let mut diagnostics = self.compile_diagnostics().unwrap_or_default();
        diagnostics.extend(artifact.diagnostics.iter().cloned());
        let warnings = diagnostics
            .iter()
            .filter(|item| item.severity != story_document::DiagnosticSeverity::Error)
            .map(|item| format!("{}: {}", item.code, item.message))
            .collect();
        let record = CompileRunRecord {
            version: 6,
            run_id: uuid::Uuid::new_v4().to_string(),
            configuration: "One-off Compile".into(),
            format: Some(artifact.format.extension().to_ascii_uppercase()),
            succeeded,
            created_unix_millis: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            elapsed_millis,
            outputs,
            warnings,
            diagnostics,
            error,
            source_fingerprint: self.compile_source_fingerprint().ok(),
            stale: false,
        };
        self.persist_compile_run_record(&record)?;
        result
    }

    fn publish_compile_artifact_with_sidecars_inner(
        &self,
        destination: impl AsRef<Path>,
        artifact: &story_document::CompileArtifact,
        overwrite: bool,
        sidecars: &[(String, Vec<u8>)],
    ) -> Result<Vec<PathBuf>> {
        let project = self.get_project()?;
        let project_root = self
            .store
            .project_root()
            .canonicalize()
            .map_err(|error| AppError::Domain(format!("resolve project root: {error}")))?;
        if let Some(notice) = recover_compile_file_set(&project_root, project.id)? {
            *self.compile_recovery_notice.borrow_mut() = Some(notice.clone());
            return Err(AppError::Domain(notice));
        }
        *self.compile_recovery_notice.borrow_mut() = None;
        let destination = destination.as_ref();
        let destination = if destination.is_absolute() {
            destination.to_path_buf()
        } else {
            self.store.project_root().join(destination)
        };
        reject_absolute_symlink_components(&destination)?;
        let file_name = destination
            .file_name()
            .ok_or_else(|| AppError::Domain("Compile output path has no file name".into()))?;
        let parent = destination
            .parent()
            .ok_or_else(|| AppError::Domain("Compile output path has no parent".into()))?;
        fs::create_dir_all(parent).map_err(|error| {
            AppError::Domain(format!("create Compile output directory: {error}"))
        })?;
        let parent = parent.canonicalize().map_err(|error| {
            AppError::Domain(format!("resolve Compile output directory: {error}"))
        })?;
        let destination = parent.join(file_name);
        if fs::symlink_metadata(&destination)
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(AppError::Domain(
                "refusing to replace a symlink with Compile output".into(),
            ));
        }
        let project_root = self
            .store
            .project_root()
            .canonicalize()
            .map_err(|error| AppError::Domain(format!("resolve project root: {error}")))?;
        if destination.starts_with(&project_root) {
            let relative = destination
                .strip_prefix(&project_root)
                .map_err(|error| AppError::Domain(error.to_string()))?;
            if relative.starts_with("manuscript")
                || relative.starts_with(".writers-ide")
                || relative == Path::new("project.toml")
                || relative == Path::new("story.db")
            {
                return Err(AppError::Domain(format!(
                    "Compile output cannot replace project source or metadata: {}",
                    destination.display()
                )));
            }
        }
        let mut files = vec![(destination.clone(), artifact.bytes.clone())];
        let source_map = PathBuf::from(format!("{}.source-map.json", destination.display()));
        let relative_label = destination
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let source_map_bytes = serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1,
            "output": relative_label,
            "output_format": artifact.format,
            "offset_space": "compile-ir-markdown-utf8",
            "visual_layout_mapping": false,
            "entries": artifact.source_map,
        }))
        .map_err(|error| AppError::Domain(format!("serialize Compile source map: {error}")))?;
        files.push((source_map, source_map_bytes));
        for auxiliary in &artifact.auxiliary_files {
            let extension = Path::new(&auxiliary.name)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("log");
            files.push((
                destination.with_extension(extension),
                auxiliary.bytes.clone(),
            ));
        }
        for (name, bytes) in sidecars {
            let sidecar_name = Path::new(name);
            if sidecar_name.components().count() != 1
                || !matches!(
                    sidecar_name.components().next(),
                    Some(std::path::Component::Normal(_))
                )
            {
                return Err(AppError::Domain(format!(
                    "Compile sidecar must be a filename: {name}"
                )));
            }
            files.push((parent.join(sidecar_name), bytes.clone()));
        }
        let mut unique = std::collections::BTreeSet::new();
        for (path, _) in &files {
            if !unique.insert(path.clone()) {
                return Err(AppError::Domain(format!(
                    "Compile output set contains a path collision: {}",
                    path.display()
                )));
            }
            if !overwrite && path.exists() {
                return Err(AppError::Domain(format!(
                    "Compile output already exists: {}; pass --overwrite after review",
                    path.display()
                )));
            }
        }
        publish_compile_file_set(&project_root, project.id, &files)?;
        Ok(files.into_iter().map(|(path, _)| path).collect())
    }

    fn compile_configurations_dir(&self) -> PathBuf {
        self.store
            .project_root()
            .join(".writers-ide/compile-configurations")
    }

    fn compile_manifest_path(&self) -> PathBuf {
        self.store
            .project_root()
            .join(".writers-ide/compile-artifacts.json")
    }

    pub fn list_compile_configurations(&self) -> Result<Vec<String>> {
        let directory = self.compile_configurations_dir();
        reject_project_symlink_components(
            self.store.project_root(),
            ".writers-ide/compile-configurations",
        )?;
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(AppError::Domain(format!(
                    "read compile configurations: {error}"
                )));
            }
        };
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| {
                AppError::Domain(format!("read compile configuration entry: {error}"))
            })?;
            if entry.file_type().is_ok_and(|kind| kind.is_file())
                && entry.path().extension().is_some_and(|ext| ext == "toml")
                && let Some(name) = entry.path().file_stem().and_then(|name| name.to_str())
            {
                validate_compile_configuration_name(name)?;
                names.push(name.to_owned());
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn create_compile_configuration(
        &self,
        name: &str,
        recipe: &story_document::EditionRecipe,
    ) -> Result<PathBuf> {
        validate_compile_configuration_name(name)?;
        recipe.validate().map_err(AppError::Domain)?;
        validate_managed_compile_output(name, &recipe.output)?;
        let directory = self.compile_configurations_dir();
        reject_project_symlink_components(
            self.store.project_root(),
            &format!(".writers-ide/compile-configurations/{name}.toml"),
        )?;
        fs::create_dir_all(&directory).map_err(|error| {
            AppError::Domain(format!("create compile configuration directory: {error}"))
        })?;
        let path = directory.join(format!("{name}.toml"));
        let bytes = recipe.to_toml().map_err(AppError::Domain)?;
        create_project_file_new(&path, bytes.as_bytes())?;
        Ok(path)
    }

    pub fn create_builtin_compile_configuration(
        &self,
        name: &str,
        preset: &str,
    ) -> Result<PathBuf> {
        let (format, design) = match preset {
            "submission" => (
                story_document::CompileFormat::Docx,
                story_document::BookDesign::Manuscript,
            ),
            "ebook" => (
                story_document::CompileFormat::Epub,
                story_document::BookDesign::TradePaperback,
            ),
            "paperback" => (
                story_document::CompileFormat::Pdf,
                story_document::BookDesign::TradePaperback,
            ),
            "proof" => (
                story_document::CompileFormat::Pdf,
                story_document::BookDesign::Manuscript,
            ),
            "plain" => (
                story_document::CompileFormat::PlainText,
                story_document::BookDesign::Manuscript,
            ),
            "markdown" => (
                story_document::CompileFormat::Markdown,
                story_document::BookDesign::Manuscript,
            ),
            "odt" => (
                story_document::CompileFormat::Odt,
                story_document::BookDesign::Manuscript,
            ),
            "rtf" => (
                story_document::CompileFormat::Rtf,
                story_document::BookDesign::Manuscript,
            ),
            "azw3" => (
                story_document::CompileFormat::Azw3,
                story_document::BookDesign::TradePaperback,
            ),
            "mobi" => (
                story_document::CompileFormat::Mobi,
                story_document::BookDesign::TradePaperback,
            ),
            _ => {
                return Err(AppError::Domain(
                    "preset must be submission, ebook, paperback, proof, plain, markdown, odt, rtf, azw3, or mobi".into(),
                ));
            }
        };
        let recipe = story_document::EditionRecipe {
            version: 1,
            name: name.replace('-', " "),
            format,
            assembly_id: None,
            output: format!("exports/compile/{name}/manuscript.{}", format.extension()),
            metadata_title: None,
            metadata_author: None,
            cover_image: None,
            design,
            expand_transclusions: false,
            spellcheck: false,
            language: None,
            dictionary: None,
            spelling_engine: story_document::SpellcheckEngine::default(),
            diagnostics: story_document::DiagnosticPolicy::default(),
        };
        self.create_compile_configuration(name, &recipe)
    }

    pub fn compile_configuration(
        &self,
        name: &str,
        overwrite_managed_output: bool,
    ) -> Result<CompileBuildResult> {
        self.compile_configuration_with_diagnostic_overrides(
            name,
            overwrite_managed_output,
            &story_document::DiagnosticOverrides::default(),
        )
    }

    pub fn compile_configuration_with_diagnostic_overrides(
        &self,
        name: &str,
        overwrite_managed_output: bool,
        overrides: &story_document::DiagnosticOverrides,
    ) -> Result<CompileBuildResult> {
        use std::time::Instant;
        let timer = Instant::now();
        let result = self.compile_configuration_inner(name, overwrite_managed_output, overrides);
        let elapsed_millis = timer.elapsed().as_millis();
        let created_unix_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let configured_format = self
            .compile_configurations_dir()
            .join(format!("{name}.toml"));
        let configured_format = fs::read_to_string(configured_format)
            .ok()
            .and_then(|source| story_document::EditionRecipe::from_toml(&source).ok())
            .map(|recipe| recipe.format.extension().to_ascii_uppercase());
        let (succeeded, outputs, warnings, error, format, diagnostics) = match &result {
            Ok(build) => (
                build.status == "success",
                build.outputs.clone(),
                build.warnings.clone(),
                (build.status != "success")
                    .then(|| "configured diagnostics promoted findings to errors".into()),
                Some(build.format.clone()),
                build.diagnostics.clone(),
            ),
            Err(error) => (
                false,
                Vec::new(),
                Vec::new(),
                Some(error.to_string()),
                configured_format,
                Vec::new(),
            ),
        };
        let record = CompileRunRecord {
            version: 6,
            run_id: uuid::Uuid::new_v4().to_string(),
            configuration: name.to_owned(),
            format,
            succeeded,
            created_unix_millis,
            elapsed_millis,
            outputs,
            warnings,
            diagnostics,
            error: error.clone(),
            source_fingerprint: self.compile_source_fingerprint().ok(),
            stale: false,
        };
        let log_relative = self.persist_compile_run_record(&record)?;
        match result {
            Ok(mut build) => {
                build.elapsed_millis = elapsed_millis;
                build.log_path = log_relative;
                Ok(build)
            }
            Err(error) => Err(error),
        }
    }

    fn compile_configuration_inner(
        &self,
        name: &str,
        overwrite_managed_output: bool,
        overrides: &story_document::DiagnosticOverrides,
    ) -> Result<CompileBuildResult> {
        validate_compile_configuration_name(name)?;
        let recipe_path = self
            .compile_configurations_dir()
            .join(format!("{name}.toml"));
        reject_project_symlink_components(
            self.store.project_root(),
            &format!(".writers-ide/compile-configurations/{name}.toml"),
        )?;
        let source = fs::read_to_string(&recipe_path).map_err(|error| {
            AppError::Domain(format!("read compile configuration '{name}': {error}"))
        })?;
        let mut recipe =
            story_document::EditionRecipe::from_toml(&source).map_err(AppError::Domain)?;
        recipe.diagnostics.apply_overrides(overrides);
        validate_managed_compile_output(name, &recipe.output)?;
        let mut manifest = self.load_managed_compile_manifest()?;
        let output = recipe.output.clone();
        let mut output_paths = vec![output.clone()];
        output_paths.push(format!("{output}.source-map.json"));
        let artifact = self.compile_artifact_for_edition(&recipe)?;
        let mut diagnostics = if let Some(assembly_id) = recipe.assembly_id.as_deref() {
            let assembly =
                AssemblyId(uuid::Uuid::parse_str(assembly_id).map_err(|error| {
                    AppError::Domain(format!("invalid assembly UUID: {error}"))
                })?);
            let ir = self.compile_assembly_ir(assembly)?;
            self.compile_diagnostics_for_ir(&ir, &recipe.diagnostics)?
        } else {
            self.compile_diagnostics_with_policy(&recipe.diagnostics)?
        };
        diagnostics.extend(artifact.diagnostics.iter().cloned());
        for diagnostic in &mut diagnostics {
            diagnostic.effective_level = Some(recipe.diagnostics.effective_level(diagnostic));
        }
        diagnostics.retain(|diagnostic| {
            diagnostic.effective_level != Some(story_document::DiagnosticLevel::Off)
        });
        let warnings = diagnostics
            .iter()
            .filter(|diagnostic| {
                matches!(
                    diagnostic.effective_level,
                    Some(
                        story_document::DiagnosticLevel::Information
                            | story_document::DiagnosticLevel::Warning
                    )
                )
            })
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>();
        if diagnostics.iter().any(|diagnostic| {
            diagnostic.effective_level == Some(story_document::DiagnosticLevel::Error)
                && !diagnostic.suppressed
        }) {
            return Ok(CompileBuildResult {
                configuration: name.to_owned(),
                format: recipe.format.extension().to_ascii_uppercase(),
                status: "failed".into(),
                outputs: Vec::new(),
                elapsed_millis: 0,
                warnings,
                diagnostics,
                log_path: String::new(),
            });
        }
        for auxiliary in &artifact.auxiliary_files {
            let extension = Path::new(&auxiliary.name)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("log");
            output_paths.push(
                Path::new(&output)
                    .with_extension(extension)
                    .to_string_lossy()
                    .into(),
            );
        }
        output_paths.sort();
        output_paths.dedup();
        let previously_managed = manifest.configurations.get(name);
        for relative in &output_paths {
            let path = self.store.project_root().join(relative);
            reject_project_symlink_components(self.store.project_root(), relative)?;
            if path.exists()
                && !overwrite_managed_output
                && !previously_managed
                    .is_some_and(|items| items.iter().any(|item| item.path == *relative))
            {
                return Err(AppError::Domain(format!(
                    "compile output already exists and is not managed by this configuration: {relative}; use explicit overwrite after review"
                )));
            }
            if path.exists()
                && !overwrite_managed_output
                && let Some(managed) = previously_managed
                    .and_then(|items| items.iter().find(|item| item.path == *relative))
            {
                let current_hash = format!(
                    "{:x}",
                    Sha256::digest(fs::read(&path).map_err(|error| AppError::Domain(format!(
                        "read managed Compile output: {error}"
                    )),)?)
                );
                if current_hash != managed.sha256 {
                    return Err(AppError::Domain(format!(
                        "managed Compile output was modified outside Writer's IDE: {relative}; use Rebuild only after reviewing the changed file"
                    )));
                }
            }
        }
        let destination = self.store.project_root().join(&output);
        let mut generated = std::collections::BTreeMap::new();
        generated.insert(output.clone(), artifact.bytes.clone());
        generated.insert(
            format!("{output}.source-map.json"),
            compile_source_map_bytes(&destination, recipe.format, &artifact.source_map)?,
        );
        for auxiliary in &artifact.auxiliary_files {
            let extension = Path::new(&auxiliary.name)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("log");
            let auxiliary_path = Path::new(&output)
                .with_extension(extension)
                .to_string_lossy()
                .into_owned();
            generated.insert(auxiliary_path, auxiliary.bytes.clone());
        }
        manifest.version = 1;
        let managed_files = output_paths
            .iter()
            .map(|path| {
                let bytes = generated.get(path).ok_or_else(|| {
                    AppError::Domain(format!("Compile artifact set is missing {path}"))
                })?;
                Ok(ManagedCompileArtifact {
                    path: path.clone(),
                    sha256: format!("{:x}", Sha256::digest(bytes)),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        manifest
            .configurations
            .insert(name.to_owned(), managed_files);
        let mut complete_file_set = generated;
        complete_file_set.insert(
            ".writers-ide/compile-artifacts.json".into(),
            serde_json::to_vec_pretty(&manifest).map_err(|error| {
                AppError::Domain(format!("serialize compile manifest: {error}"))
            })?,
        );
        let publish_files = complete_file_set
            .into_iter()
            .map(|(relative, bytes)| {
                reject_project_symlink_components(self.store.project_root(), &relative)?;
                Ok((self.store.project_root().join(relative), bytes))
            })
            .collect::<Result<Vec<_>>>()?;
        let project = self.get_project()?;
        let project_root = self
            .store
            .project_root()
            .canonicalize()
            .map_err(|error| AppError::Domain(format!("resolve project root: {error}")))?;
        if let Some(notice) = recover_compile_file_set(&project_root, project.id)? {
            *self.compile_recovery_notice.borrow_mut() = Some(notice.clone());
            return Err(AppError::Domain(notice));
        }
        *self.compile_recovery_notice.borrow_mut() = None;
        publish_compile_file_set(&project_root, project.id, &publish_files)?;
        Ok(CompileBuildResult {
            configuration: name.to_owned(),
            format: recipe.format.extension().to_ascii_uppercase(),
            status: "success".into(),
            outputs: output_paths,
            elapsed_millis: 0,
            warnings,
            diagnostics,
            log_path: String::new(),
        })
    }

    pub fn list_compile_results(&self) -> Result<Vec<CompileRunRecord>> {
        let directory = self
            .store
            .project_root()
            .join(".writers-ide/compile-results");
        reject_project_symlink_components(
            self.store.project_root(),
            ".writers-ide/compile-results",
        )?;
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(AppError::Domain(format!("read Compile results: {error}"))),
        };
        let mut records = Vec::new();
        let current_fingerprint = self.compile_source_fingerprint().ok();
        for entry in entries {
            let entry = entry
                .map_err(|error| AppError::Domain(format!("read Compile result entry: {error}")))?;
            if entry.file_type().is_ok_and(|kind| kind.is_file())
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
            {
                let mut record: CompileRunRecord =
                    serde_json::from_slice(&fs::read(entry.path()).map_err(|error| {
                        AppError::Domain(format!("read Compile result file: {error}"))
                    })?)
                    .map_err(|error| {
                        AppError::Domain(format!("invalid Compile result file: {error}"))
                    })?;
                if !matches!(record.version, 1..=6) {
                    return Err(AppError::Domain(format!(
                        "unsupported Compile result version {}",
                        record.version
                    )));
                }
                record.stale =
                    record.source_fingerprint.as_deref() != current_fingerprint.as_deref();
                records.push(record);
            }
        }
        records.sort_by_key(|record| record.created_unix_millis);
        records.reverse();
        Ok(records)
    }

    fn persist_compile_run_record(&self, record: &CompileRunRecord) -> Result<String> {
        let log_relative = format!(
            ".writers-ide/compile-results/{}-{}.json",
            record.created_unix_millis,
            uuid::Uuid::new_v4()
        );
        reject_project_symlink_components(self.store.project_root(), &log_relative)?;
        let log_path = self.store.project_root().join(&log_relative);
        let log_bytes = serde_json::to_vec_pretty(record)
            .map_err(|error| AppError::Domain(format!("serialize Compile result: {error}")))?;
        create_project_file_new(&log_path, &log_bytes)?;
        Ok(log_relative)
    }

    pub fn clean_compile_configuration(&self, name: &str) -> Result<Vec<String>> {
        validate_compile_configuration_name(name)?;
        let mut manifest = self.load_managed_compile_manifest()?;
        let Some(outputs) = manifest.configurations.remove(name) else {
            return Ok(Vec::new());
        };
        for artifact in &outputs {
            validate_managed_compile_output(name, &artifact.path)?;
            reject_project_symlink_components(self.store.project_root(), &artifact.path)?;
            let path = self.store.project_root().join(&artifact.path);
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.is_file() => {
                    let current_hash = format!(
                        "{:x}",
                        Sha256::digest(fs::read(&path).map_err(|error| {
                            AppError::Domain(format!(
                                "read managed compile output {}: {error}",
                                artifact.path
                            ))
                        })?)
                    );
                    if current_hash != artifact.sha256 {
                        return Err(AppError::Domain(format!(
                            "refusing to clean modified Compile output: {}",
                            artifact.path
                        )));
                    }
                }
                Ok(_) => {
                    return Err(AppError::Domain(format!(
                        "refusing to clean non-file compile output: {}",
                        artifact.path
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(AppError::Domain(format!(
                        "inspect managed compile output {}: {error}",
                        artifact.path
                    )));
                }
            }
        }
        for artifact in &outputs {
            let path = self.store.project_root().join(&artifact.path);
            match fs::symlink_metadata(&path) {
                Ok(_) => fs::remove_file(&path).map_err(|error| {
                    AppError::Domain(format!(
                        "remove managed compile output {}: {error}",
                        artifact.path
                    ))
                })?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(AppError::Domain(format!(
                        "inspect managed compile output {}: {error}",
                        artifact.path
                    )));
                }
            }
        }
        write_project_file_atomically(
            &self.compile_manifest_path(),
            &serde_json::to_vec_pretty(&manifest).map_err(|error| {
                AppError::Domain(format!("serialize compile manifest: {error}"))
            })?,
        )?;
        Ok(outputs.into_iter().map(|artifact| artifact.path).collect())
    }

    fn load_managed_compile_manifest(&self) -> Result<ManagedCompileManifest> {
        reject_project_symlink_components(
            self.store.project_root(),
            ".writers-ide/compile-artifacts.json",
        )?;
        match fs::read(self.compile_manifest_path()) {
            Ok(bytes) => {
                let manifest: ManagedCompileManifest =
                    serde_json::from_slice(&bytes).map_err(|error| {
                        AppError::Domain(format!("invalid compile manifest: {error}"))
                    })?;
                if manifest.version != 1 {
                    return Err(AppError::Domain(format!(
                        "unsupported compile manifest version {}",
                        manifest.version
                    )));
                }
                for (name, paths) in &manifest.configurations {
                    validate_compile_configuration_name(name)?;
                    for artifact in paths {
                        validate_managed_compile_output(name, &artifact.path)?;
                    }
                }
                Ok(manifest)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(ManagedCompileManifest {
                    version: 1,
                    configurations: Default::default(),
                })
            }
            Err(error) => Err(AppError::Domain(format!("read compile manifest: {error}"))),
        }
    }
    pub fn compile_diagnostics(&self) -> Result<Vec<story_document::CompileDiagnostic>> {
        self.compile_diagnostics_with_policy(&story_document::DiagnosticPolicy::default())
    }

    pub fn add_diagnostic_suppression(
        &mut self,
        diagnostic: &story_document::CompileDiagnostic,
        scope: &str,
        reason: &str,
        expires_unix_ms: Option<i64>,
    ) -> Result<String> {
        if matches!(diagnostic.category.as_str(), "renderer" | "operational")
            || (diagnostic.severity == story_document::DiagnosticSeverity::Error
                && diagnostic.category == "renderer")
        {
            return Err(AppError::Domain(
                "operational and renderer diagnostics cannot be suppressed".into(),
            ));
        }
        let (scene_id, finding_fingerprint) = match scope {
            "rule" => (None, None),
            "scene" => (
                Some(diagnostic.source_entity_id.clone().ok_or_else(|| {
                    AppError::Domain("scene suppression requires a source entity".into())
                })?),
                None,
            ),
            "finding" => (
                Some(diagnostic.source_entity_id.clone().ok_or_else(|| {
                    AppError::Domain("finding suppression requires a source entity".into())
                })?),
                Some(diagnostic.finding_fingerprint()),
            ),
            _ => {
                return Err(AppError::Domain(
                    "suppression scope must be rule, scene, or finding".into(),
                ));
            }
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64;
        if reason.trim().is_empty() || expires_unix_ms.is_some_and(|expiry| expiry <= now) {
            return Err(AppError::Domain(
                "suppression requires a reason and any expiry must be in the future".into(),
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let record = DiagnosticSuppressionRecord {
            id: id.clone(),
            rule_id: diagnostic.code.clone(),
            scope: scope.to_owned(),
            scene_id,
            finding_fingerprint,
            reason: reason.trim().to_owned(),
            created_unix_ms: now,
            expires_unix_ms,
            triggering_fingerprint: self.compile_source_fingerprint().ok(),
            status: "active".into(),
            last_checked_unix_ms: now,
        };
        self.store.create_diagnostic_suppression(&record)?;
        Ok(id)
    }

    pub fn diagnostic_suppressions(&self) -> Result<Vec<DiagnosticSuppressionRecord>> {
        self.store
            .list_diagnostic_suppressions()
            .map_err(Into::into)
    }

    pub fn revoke_diagnostic_suppression(&self, id: &str) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64;
        self.store
            .update_diagnostic_suppression_state(id, "revoked", now)
            .map_err(Into::into)
    }

    fn compile_diagnostics_with_policy(
        &self,
        policy: &story_document::DiagnosticPolicy,
    ) -> Result<Vec<story_document::CompileDiagnostic>> {
        let input = self.processing_input()?;
        let ir = story_pipeline::compile_ir(&input);
        self.compile_diagnostics_for_ir(&ir, policy)
    }

    fn compile_diagnostics_for_ir(
        &self,
        ir: &story_document::CompileIr,
        policy: &story_document::DiagnosticPolicy,
    ) -> Result<Vec<story_document::CompileDiagnostic>> {
        let mut diagnostics = story_document::deterministic_diagnostics(ir, policy);
        let proofreading = self.proofreading_config()?;
        for unit in &ir.units {
            for segment in &unit.segments {
                for finding in
                    story_analysis::proofread_with_config(&segment.document.source, &proofreading)
                        .map_err(AppError::Domain)?
                {
                    let severity = match finding.severity.as_str() {
                        "error" => story_document::DiagnosticSeverity::Error,
                        "warning" => story_document::DiagnosticSeverity::Warning,
                        "information" | "info" => story_document::DiagnosticSeverity::Information,
                        "off" => continue,
                        other => {
                            return Err(AppError::Domain(format!(
                                "unsupported proofreading severity {other}"
                            )));
                        }
                    };
                    diagnostics.push(story_document::CompileDiagnostic {
                        code: finding.category,
                        category: "mechanical-proofreading".into(),
                        severity,
                        effective_level: None,
                        message: finding.message,
                        remediation_hint: None,
                        source_entity_id: Some(segment.source_entity_id.clone()),
                        source_range: Some(story_document::SourceRange {
                            start: finding.byte_range.start,
                            end: finding.byte_range.end,
                        }),
                        generated_file: None,
                        generated_line: None,
                        suppressed_by: None,
                        suppressed: false,
                    });
                }
            }
        }
        let chronology_enabled = self.chronology_preferences()?.enabled;
        let plotline_gap_threshold = policy.minimum_plotline_gap_scenes.or(match policy.preset {
            story_document::DiagnosticPreset::Thorough => Some(8),
            story_document::DiagnosticPreset::Extended => Some(4),
            story_document::DiagnosticPreset::Pedantic => Some(1),
            story_document::DiagnosticPreset::Quiet
            | story_document::DiagnosticPreset::Standard => None,
        });
        if plotline_gap_threshold.is_some() || chronology_enabled {
            let scenes = self.analytics_dataset()?;
            if let Some(threshold) = plotline_gap_threshold {
                for gap in story_analysis::subplot_gaps(&scenes, threshold as usize) {
                    diagnostics.push(Self::source_compile_diagnostic(
                        "PLOT001",
                        "plotting",
                        story_document::DiagnosticSeverity::Information,
                        format!(
                            "plotline '{}' has a gap of {} intervening scene(s), meeting the configured {}-scene threshold",
                            gap.plotline_name, gap.intervening_scenes, threshold
                        ),
                        Some(gap.from_scene_id),
                        Some("Review the recorded gap against the intended subplot rhythm"),
                    ));
                }
            }
            if chronology_enabled {
                for finding in story_analysis::character_chronology_findings(&scenes) {
                    let (code, severity, label) = match finding.status {
                        story_analysis::CharacterChronologyStatus::DefiniteOverlap => (
                            "CHR001",
                            story_document::DiagnosticSeverity::Warning,
                            "recorded character intervals overlap",
                        ),
                        story_analysis::CharacterChronologyStatus::PossibleOverlap => (
                            "CHR002",
                            story_document::DiagnosticSeverity::Information,
                            "recorded character intervals may overlap",
                        ),
                        story_analysis::CharacterChronologyStatus::Unverified => (
                            "CHR003",
                            story_document::DiagnosticSeverity::Information,
                            "character chronology cannot be verified from incomplete times",
                        ),
                    };
                    diagnostics.push(Self::source_compile_diagnostic(
                        code,
                        "chronology",
                        severity,
                        format!(
                            "{label} in scenes {} and {} for character record(s) {}",
                            finding.first_scene_id,
                            finding.second_scene_id,
                            finding.shared_character_ids.join(", ")
                        ),
                        Some(finding.first_scene_id),
                        Some("Review both recorded story-time intervals and participation roles"),
                    ));
                }
                for finding in self.inspect_location_incompatibilities()? {
                    let (code, severity, label) = match finding.status {
                        story_analysis::LocationRuleFindingStatus::DefiniteConflict => (
                            "LOC001",
                            story_document::DiagnosticSeverity::Warning,
                            "explicit incompatible-location rule conflicts with the recorded intervals",
                        ),
                        story_analysis::LocationRuleFindingStatus::PossibleConflict => (
                            "LOC002",
                            story_document::DiagnosticSeverity::Information,
                            "explicit incompatible-location rule may conflict with the recorded intervals",
                        ),
                        story_analysis::LocationRuleFindingStatus::Unverified => (
                            "LOC003",
                            story_document::DiagnosticSeverity::Information,
                            "explicit incompatible-location rule cannot be verified from incomplete times",
                        ),
                    };
                    diagnostics.push(Self::source_compile_diagnostic(
                        code,
                        "continuity",
                        severity,
                        format!(
                            "{label}: {} between scenes {} and {}",
                            finding.description, finding.first_scene_id, finding.second_scene_id
                        ),
                        Some(finding.first_scene_id),
                        Some("Review the rule, location assignments, and story-time intervals"),
                    ));
                }
                let event_scene_ids = scenes
                    .iter()
                    .flat_map(|scene| {
                        scene
                            .events
                            .iter()
                            .map(move |event| (event.id.clone(), scene.scene_id.clone()))
                    })
                    .collect::<std::collections::BTreeMap<_, _>>();
                for (dependency, result) in self.inspect_event_dependencies()? {
                    let (code, severity) = match result.status {
                        story_analysis::InspectionStatus::Fail => {
                            ("EVT001", story_document::DiagnosticSeverity::Warning)
                        }
                        story_analysis::InspectionStatus::Unknown => {
                            ("EVT002", story_document::DiagnosticSeverity::Information)
                        }
                        story_analysis::InspectionStatus::Pass
                        | story_analysis::InspectionStatus::Skip => continue,
                    };
                    let source = event_scene_ids
                        .get(&dependency.predecessor.to_string())
                        .or_else(|| event_scene_ids.get(&dependency.successor.to_string()))
                        .cloned();
                    diagnostics.push(Self::source_compile_diagnostic(
                        code,
                        "chronology",
                        severity,
                        format!(
                            "recorded event prerequisite is {:?}: {}",
                            result.status, result.assertion
                        ),
                        source,
                        Some("Review the prerequisite relation and both event times"),
                    ));
                }
            }
        }
        self.apply_diagnostic_suppressions(&mut diagnostics)?;
        Ok(diagnostics)
    }

    fn source_compile_diagnostic(
        code: &str,
        category: &str,
        severity: story_document::DiagnosticSeverity,
        message: String,
        source_entity_id: Option<String>,
        remediation_hint: Option<&str>,
    ) -> story_document::CompileDiagnostic {
        story_document::CompileDiagnostic {
            code: code.into(),
            category: category.into(),
            severity,
            effective_level: None,
            message,
            remediation_hint: remediation_hint.map(str::to_owned),
            source_entity_id,
            source_range: None,
            generated_file: None,
            generated_line: None,
            suppressed_by: None,
            suppressed: false,
        }
    }

    fn apply_diagnostic_suppressions(
        &self,
        diagnostics: &mut [story_document::CompileDiagnostic],
    ) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64;
        let suppressions = self.store.list_diagnostic_suppressions()?;
        for suppression in suppressions {
            if suppression.status == "revoked" {
                continue;
            }
            let source_match = diagnostics.iter().any(|diagnostic| {
                diagnostic.code == suppression.rule_id
                    && match suppression.scope.as_str() {
                        "rule" => true,
                        "scene" => diagnostic.source_entity_id == suppression.scene_id,
                        "finding" => {
                            diagnostic.source_entity_id == suppression.scene_id
                                && Some(diagnostic.finding_fingerprint())
                                    == suppression.finding_fingerprint
                        }
                        _ => false,
                    }
            });
            let state = if suppression
                .expires_unix_ms
                .is_some_and(|expiry| expiry <= now)
            {
                "expired"
            } else if source_match || suppression.scope == "rule" {
                "active"
            } else {
                "stale"
            };
            self.store
                .update_diagnostic_suppression_state(&suppression.id, state, now)?;
            if state != "active" {
                continue;
            }
            if suppression.scope == "rule" || source_match {
                for diagnostic in diagnostics.iter_mut().filter(|diagnostic| {
                    diagnostic.code == suppression.rule_id
                        && match suppression.scope.as_str() {
                            "rule" => true,
                            "scene" => diagnostic.source_entity_id == suppression.scene_id,
                            "finding" => {
                                diagnostic.source_entity_id == suppression.scene_id
                                    && Some(diagnostic.finding_fingerprint())
                                        == suppression.finding_fingerprint
                            }
                            _ => false,
                        }
                        && !matches!(diagnostic.category.as_str(), "renderer" | "operational")
                }) {
                    diagnostic.suppressed = true;
                    diagnostic.suppressed_by = Some(suppression.id.clone());
                }
            }
        }
        Ok(())
    }
    fn compile_source_fingerprint(&self) -> Result<String> {
        let input = self.processing_input()?;
        let ir = story_pipeline::compile_ir(&input);
        let bytes = serde_json::to_vec(&ir).map_err(|error| {
            AppError::Domain(format!("serialize Compile source snapshot: {error}"))
        })?;
        Ok(compile_sha256(&bytes))
    }
    pub fn proofreading_config(&self) -> Result<story_analysis::ProofreadingConfig> {
        let path = self
            .store
            .project_root()
            .join(".writers-ide/diagnostics.toml");
        match std::fs::read_to_string(path) {
            Ok(source) => {
                story_analysis::ProofreadingConfig::from_toml(&source).map_err(AppError::Domain)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(story_analysis::ProofreadingConfig::default())
            }
            Err(error) => Err(AppError::Domain(format!(
                "read diagnostics config: {error}"
            ))),
        }
    }
    pub fn proofread_text(&self, text: &str) -> Result<Vec<story_analysis::ProofreadingFinding>> {
        story_analysis::proofread_with_config(text, &self.proofreading_config()?)
            .map_err(AppError::Domain)
    }
    pub fn write_default_proofreading_config(&self) -> Result<std::path::PathBuf> {
        self.write_proofreading_config(story_analysis::ProofreadingPreset::Drafting)
    }
    pub fn write_proofreading_config(
        &self,
        preset: story_analysis::ProofreadingPreset,
    ) -> Result<std::path::PathBuf> {
        let path = self
            .store
            .project_root()
            .join(".writers-ide/diagnostics.toml");
        let source = story_analysis::ProofreadingConfig::preset(preset)
            .to_toml()
            .map_err(AppError::Domain)?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| AppError::Domain(format!("create diagnostics config: {error}")))?;
        file.write_all(source.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| AppError::Domain(format!("write diagnostics config: {error}")))?;
        Ok(path)
    }
    fn processing_input_mode(
        &self,
        expand_transclusions: bool,
    ) -> Result<story_pipeline::PipelineInput> {
        let project = self.get_project()?;
        let chronology_enabled = self.chronology_preferences()?.enabled;
        let analytics = self.analytics_dataset()?;
        let groups = self
            .store
            .list_structural_nodes()?
            .into_iter()
            .map(|node| story_pipeline::PipelineGroup {
                id: node.id.to_string(),
                parent_id: node.parent_id.map(|id| id.to_string()),
                kind: match node.kind.as_str() {
                    "section" => story_document::StructuralKind::Section,
                    "chapter" => story_document::StructuralKind::Chapter,
                    _ => story_document::StructuralKind::Group,
                },
                title: node.title,
            })
            .collect();
        let mut scenes = Vec::new();
        let mut content_units = Vec::new();
        for unit in self.store.list_content_units(false)? {
            let (text, _) = self.store.load_content_unit_text(unit.id)?;
            let language_code = Some(unit.effective_language_code(&project).to_owned());
            content_units.push(story_pipeline::PipelineContentUnit {
                id: unit.id.to_string(),
                parent_id: unit.parent_id.map(|id| id.to_string()),
                kind: match unit.kind {
                    ContentUnitKind::FrontMatter => story_document::ContentUnitKind::FrontMatter,
                    ContentUnitKind::SectionBody => story_document::ContentUnitKind::SectionBody,
                    ContentUnitKind::ChapterBody => story_document::ContentUnitKind::ChapterBody,
                    ContentUnitKind::BackMatter => story_document::ContentUnitKind::BackMatter,
                },
                title: unit.title,
                language_code,
                text,
            });
        }
        let mut snapshots = 0usize;
        for row in self.store.list_scenes(false)? {
            if !self.store.scene_is_included(row.scene.id)? {
                continue;
            }
            let (text, _) = self.store.load_text(row.scene.id)?;
            let language_code = Some(row.scene.effective_language_code(&project).to_owned());
            let language_spans = self
                .store
                .list_passage_language_tags(row.scene.id)?
                .into_iter()
                .filter(|tag| tag.anchor.state == "current")
                .map(|tag| story_document::CompileLanguageSpan {
                    start_byte: tag.anchor.start_byte,
                    end_byte: tag.anchor.end_byte,
                    language_code: tag.language_code,
                })
                .collect();
            let expanded_passages = if expand_transclusions {
                Some(
                    self.store
                        .expand_document_inclusions(EntityRef::Scene(row.scene.id))?
                        .into_iter()
                        .map(|passage| story_pipeline::PipelinePassage {
                            source_entity_id: format!(
                                "{}:{}",
                                passage.source.kind(),
                                passage.source.id()
                            ),
                            text: passage.text,
                        })
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            };
            let expanded_text = expanded_passages.as_ref().map(|passages| {
                passages
                    .iter()
                    .map(|passage| passage.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n---\n\n")
            });
            let (start_boundary, end_boundary) =
                self.store.scene_interval_boundaries(row.scene.id)?;
            let linked = analytics
                .iter()
                .find(|data| data.scene_id == row.scene.id.to_string());
            let characters = linked
                .map(|d| {
                    d.characters
                        .iter()
                        .map(|c| story_pipeline::PipelineCharacter {
                            id: c.id.clone(),
                            name: c.name.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let plotlines = linked
                .map(|d| d.plotlines.iter().map(|p| p.name.clone()).collect())
                .unwrap_or_default();
            snapshots += self.store.list_snapshots(row.scene.id)?.len();
            scenes.push(story_pipeline::PipelineScene {
                id: row.scene.id.to_string(),
                chapter_id: row.chapter_id.map(|id| id.to_string()),
                title: row.scene.title,
                language_code,
                language_spans,
                text,
                expanded_text,
                expanded_passages,
                target_words: row.scene.target_words,
                word_count_policy: row.scene.word_count_policy,
                characters,
                plotlines,
                start: row.scene.start_time,
                end: row.scene.end_time,
                start_boundary,
                end_boundary,
            });
        }
        let events = self
            .store
            .list_events()?
            .into_iter()
            .map(|event| {
                let (start_boundary, end_boundary) =
                    self.store.story_event_interval_boundaries(event.id)?;
                Ok(story_pipeline::PipelineEvent {
                    id: event.id.to_string(),
                    title: event.title,
                    start: event.start,
                    end: event.end,
                    start_boundary,
                    end_boundary,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let event_dependencies = self
            .store
            .list_event_dependencies()?
            .into_iter()
            .map(|dependency| story_pipeline::PipelineEventDependency {
                predecessor: dependency.predecessor.to_string(),
                successor: dependency.successor.to_string(),
                kind: dependency.kind,
            })
            .collect();
        Ok(story_pipeline::PipelineInput {
            title: project.title,
            author: (!project.author.trim().is_empty()).then_some(project.author),
            language_code: Some(project.language_code.clone()),
            target_words: project.target_words,
            word_count_policy: project.word_count_policy,
            expand_transclusions,
            chronology_enabled,
            groups,
            content_units,
            scenes,
            events,
            event_dependencies,
            revision_summary: format!(
                "{} scenes; {snapshots} stored snapshots",
                self.store.list_scenes(false)?.len()
            ),
        })
    }
    pub fn run_processing(&self) -> Result<Vec<story_pipeline::StageReport>> {
        self.run_processing_stages(&story_pipeline::Stage::ALL, false)
    }
    pub fn run_processing_with_transclusions(&self) -> Result<Vec<story_pipeline::StageReport>> {
        self.run_processing_stages(&story_pipeline::Stage::ALL, true)
    }
    pub fn run_processing_stages(
        &self,
        stages: &[story_pipeline::Stage],
        expand_transclusions: bool,
    ) -> Result<Vec<story_pipeline::StageReport>> {
        let input = self.processing_input_mode(expand_transclusions)?;
        Ok(
            story_pipeline::PipelineRunner::new(self.store.project_root())
                .run_selected(&input, stages)?,
        )
    }
    pub fn run_processing_recipe(
        &self,
        recipe: &story_pipeline::ProcessingRecipe,
    ) -> Result<Vec<story_pipeline::StageReport>> {
        let input = self.processing_input_mode(recipe.expand_transclusions)?;
        Ok(
            story_pipeline::PipelineRunner::new(self.store.project_root())
                .run_recipe(&input, recipe)?,
        )
    }
    pub fn save_extraction_proposal(
        &self,
        filename: &str,
        json: &[u8],
    ) -> Result<std::path::PathBuf> {
        Ok(self.store.save_proposal_file(filename, json)?)
    }
    pub fn list_extraction_proposal_files(&self) -> Result<Vec<std::path::PathBuf>> {
        Ok(self.store.list_proposal_files()?)
    }
    pub fn load_extraction_proposal_file(&self, path: &Path) -> Result<(Vec<u8>, String)> {
        let bytes = self.store.load_proposal_file(path)?;
        let fingerprint = format!("{:x}", Sha256::digest(&bytes));
        Ok((bytes, fingerprint))
    }
    pub fn update_extraction_proposal_file(
        &self,
        path: &Path,
        expected_fingerprint: &str,
        json: &[u8],
    ) -> Result<()> {
        Ok(self
            .store
            .update_proposal_file(path, expected_fingerprint, json)?)
    }
    pub fn dismiss_extraction_claim(
        &self,
        proposal: &mut story_ollama::ProvisionalExtraction,
        claim_index: usize,
    ) -> Result<()> {
        let claim = proposal.claims.get_mut(claim_index).ok_or_else(|| {
            AppError::Domain(format!("claim index {claim_index} is out of range"))
        })?;
        if claim.approved {
            return Err(AppError::Domain(
                "an approved claim cannot be dismissed; retract it through knowledge management"
                    .into(),
            ));
        }
        claim.dismissed = true;
        Ok(())
    }
    /// Promotes exactly one evidence-anchored provisional claim after revalidating its source.
    pub fn approve_extraction_claim(
        &mut self,
        proposal: &story_ollama::ProvisionalExtraction,
        claim_index: usize,
    ) -> Result<KnowledgeClaimId> {
        let claim = proposal.claims.get(claim_index).ok_or_else(|| {
            AppError::Domain(format!("claim index {claim_index} is out of range"))
        })?;
        if claim.dismissed {
            return Err(AppError::Domain(
                "dismissed extraction claims cannot be approved".into(),
            ));
        }
        let (source_id, source_text) = match proposal.source_kind.as_str() {
            "scene" => {
                let id = SceneId(
                    proposal
                        .source_id
                        .parse::<uuid::Uuid>()
                        .map_err(|error| AppError::Domain(error.to_string()))?,
                );
                let (text, fingerprint) = self.load_scene_text(id)?;
                if fingerprint != proposal.source_fingerprint {
                    return Err(AppError::Domain(
                        "proposal source changed since extraction; regenerate or review the evidence".into(),
                    ));
                }
                (id.to_string(), text)
            }
            "reading" => {
                let id = ReadingId(
                    proposal
                        .source_id
                        .parse::<uuid::Uuid>()
                        .map_err(|error| AppError::Domain(error.to_string()))?,
                );
                let reading = self.get_reading_document(id)?;
                let text = reading.extracted_text;
                let fingerprint = format!("{:x}", Sha256::digest(text.as_bytes()));
                if fingerprint != proposal.source_fingerprint {
                    return Err(AppError::Domain(
                        "proposal reading source changed since extraction; regenerate or review the evidence".into(),
                    ));
                }
                (id.to_string(), text)
            }
            other => {
                return Err(AppError::Domain(format!(
                    "unsupported extraction source kind: {other}"
                )));
            }
        };
        if proposal.source_length_bytes != source_text.len() {
            return Err(AppError::Domain(
                "proposal source length no longer matches its source".into(),
            ));
        }
        let start = claim
            .evidence_start_byte
            .ok_or_else(|| AppError::Domain("claim has no source evidence offset".into()))?;
        let end = claim
            .evidence_end_byte
            .ok_or_else(|| AppError::Domain("claim has no source evidence end offset".into()))?;
        let evidence = source_text
            .get(start..end)
            .ok_or_else(|| AppError::Domain("claim evidence offsets are invalid".into()))?;
        if evidence != claim.claim.evidence_quote || evidence.is_empty() {
            return Err(AppError::Domain(
                "claim evidence no longer matches its exact source passage".into(),
            ));
        }
        let matches_existing = |existing: &ApprovedKnowledgeClaim| {
            existing.source_kind == proposal.source_kind
                && existing.source_id == source_id
                && existing.source_fingerprint == proposal.source_fingerprint
                && existing.evidence_start_byte == start
                && existing.evidence_end_byte == end
                && existing.kind == claim.claim.kind
                && existing.name == claim.claim.name
                && existing.claim == claim.claim.claim
        };
        if let Some(existing) = self
            .store
            .list_approved_knowledge_claims()?
            .into_iter()
            .find(matches_existing)
        {
            return Ok(existing.id);
        }
        if claim.claim.kind.trim().is_empty()
            || claim.claim.name.trim().is_empty()
            || claim.claim.claim.trim().is_empty()
        {
            return Err(AppError::Domain(
                "approved claim kind, name, and statement must be non-empty".into(),
            ));
        }
        let approved_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| AppError::Domain(error.to_string()))?;
        let record = ApprovedKnowledgeClaim {
            id: KnowledgeClaimId::new(),
            kind: claim.claim.kind.clone(),
            name: claim.claim.name.clone(),
            claim: claim.claim.claim.clone(),
            approximate_story_time: claim.claim.approximate_story_time.clone(),
            source_kind: proposal.source_kind.clone(),
            source_id,
            source_fingerprint: proposal.source_fingerprint.clone(),
            source_length_bytes: proposal.source_length_bytes,
            evidence_quote: claim.claim.evidence_quote.clone(),
            evidence_start_byte: start,
            evidence_end_byte: end,
            model: proposal.model.clone(),
            approved_at: format!(
                "{:010}.{:03}",
                approved_at.as_secs(),
                approved_at.subsec_millis()
            ),
        };
        self.store.add_approved_knowledge_claim(&record)?;
        Ok(record.id)
    }
    pub fn list_approved_knowledge_claims(&self) -> Result<Vec<ApprovedKnowledgeClaim>> {
        Ok(self.store.list_approved_knowledge_claims()?)
    }
    pub fn extract_with_ollama(
        &self,
        source_kind: &str,
        source_id: &str,
        model: &str,
        endpoint: &str,
    ) -> Result<story_ollama::ProvisionalExtraction> {
        let project = self.get_project()?;
        let (source, language_code, passage_languages) = match source_kind {
            "scene" => {
                let scene_id = SceneId(
                    source_id
                        .parse::<uuid::Uuid>()
                        .map_err(|e| AppError::Domain(e.to_string()))?,
                );
                let scene = self.get_scene(scene_id)?;
                let passage_languages = self
                    .store
                    .list_passage_language_tags(scene_id)?
                    .into_iter()
                    .filter(|tag| tag.anchor.state == "current")
                    .map(|tag| {
                        Ok(story_ollama::LanguageContextSpan {
                            start_byte: usize::try_from(tag.anchor.start_byte)
                                .map_err(|error| AppError::Domain(error.to_string()))?,
                            end_byte: usize::try_from(tag.anchor.end_byte)
                                .map_err(|error| AppError::Domain(error.to_string()))?,
                            language_code: tag.language_code,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                (
                    self.load_scene_text(scene_id)?.0,
                    scene.scene.effective_language_code(&project).to_owned(),
                    passage_languages,
                )
            }
            "reading" => {
                let document = self.get_reading_document(ReadingId(
                    source_id
                        .parse::<uuid::Uuid>()
                        .map_err(|e| AppError::Domain(e.to_string()))?,
                ))?;
                (
                    document.extracted_text,
                    document.language_code.unwrap_or(project.language_code),
                    Vec::new(),
                )
            }
            _ => {
                return Err(AppError::Domain(
                    "source kind must be scene or reading".into(),
                ));
            }
        };
        let result = story_ollama::OllamaClient::new(endpoint, model)
            .map_err(|e| AppError::Domain(e.to_string()))?
            .extract_with_language_context(&source, Some(&language_code), &passage_languages)
            .map_err(|e| AppError::Domain(e.to_string()))?;
        Ok(story_ollama::attach_provenance(
            source_kind,
            source_id,
            &source,
            model,
            result,
        ))
    }
    pub fn create_snapshot(&mut self, scene: SceneId, label: &str) -> Result<SnapshotId> {
        Ok(self.store.create_snapshot(scene, label)?)
    }
    pub fn list_snapshots(&self, scene: SceneId) -> Result<Vec<story_storage::SnapshotRecord>> {
        Ok(self.store.list_snapshots(scene)?)
    }
    pub fn load_snapshot(
        &self,
        snapshot: SnapshotId,
    ) -> Result<(story_storage::SnapshotRecord, String)> {
        Ok(self.store.load_snapshot(snapshot)?)
    }
    pub fn diff_snapshot(
        &self,
        scene: SceneId,
        snapshot: SnapshotId,
    ) -> Result<Vec<story_storage::SnapshotDiffLine>> {
        Ok(self.store.diff_snapshot(scene, snapshot)?)
    }
    pub fn restore_snapshot(
        &mut self,
        scene: SceneId,
        snapshot: SnapshotId,
        expected_hash: &str,
    ) -> Result<String> {
        Ok(self
            .store
            .restore_snapshot(scene, snapshot, expected_hash)?)
    }
    pub fn restore_snapshot_lines(
        &mut self,
        scene: SceneId,
        snapshot: SnapshotId,
        expected_hash: &str,
        selection: SnapshotRestoreSelection,
    ) -> Result<String> {
        Ok(self
            .store
            .restore_snapshot_lines(scene, snapshot, expected_hash, selection)?)
    }
    pub fn update_scene_metadata(&mut self, scene: &Scene) -> Result<()> {
        Ok(self.store.update_scene(scene)?)
    }
    pub fn move_scene(&mut self, scene: SceneId, destination: NodeId, position: u32) -> Result<()> {
        Ok(self.store.move_scene(scene, destination, position)?)
    }
    pub fn reorder_scene(&mut self, scene: SceneId, position: u32) -> Result<()> {
        let record = self.store.get_scene(scene)?;
        let chapter = record
            .chapter_id
            .ok_or_else(|| AppError::Domain("scene has no chapter".into()))?;
        Ok(self.store.move_scene(scene, chapter, position)?)
    }
    pub fn archive_scene(&mut self, id: SceneId) -> Result<()> {
        Ok(self.store.archive_scene(id)?)
    }
    pub fn create_character(&mut self, name: &str) -> Result<CharacterId> {
        let id = CharacterId::new();
        self.store.add_character(id, name)?;
        Ok(id)
    }
    pub fn get_character(&self, id: CharacterId) -> Result<story_core::Character> {
        Ok(self.store.get_character(id)?)
    }
    pub fn update_character(&mut self, character: &story_core::Character) -> Result<()> {
        Ok(self.store.update_character(character)?)
    }
    pub fn set_character_field(&mut self, id: CharacterId, field: &str, value: &str) -> Result<()> {
        let mut character = self.get_character(id)?;
        match field {
            "full_name" => character.full_name = value.into(),
            "preferred_name" => character.preferred_name = value.into(),
            "description" => character.description = value.into(),
            "background" => character.background = value.into(),
            "goals" => character.goals = value.into(),
            "motivations" => character.motivations = value.into(),
            "secrets" => character.secrets = value.into(),
            "internal_conflict" => character.internal_conflict = value.into(),
            "external_conflict" => character.external_conflict = value.into(),
            "status" => character.status = value.into(),
            _ => {
                return Err(AppError::Domain(format!(
                    "unknown character field '{field}'"
                )));
            }
        }
        self.update_character(&character)
    }
    pub fn add_character_alias(&mut self, id: CharacterId, alias: &str) -> Result<()> {
        let mut character = self.get_character(id)?;
        if character
            .aliases
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(alias.trim()))
        {
            return Err(AppError::Domain("character alias already exists".into()));
        }
        character.aliases.push(alias.into());
        self.update_character(&character)
    }
    pub fn remove_character_alias(&mut self, id: CharacterId, alias: &str) -> Result<()> {
        let mut character = self.get_character(id)?;
        let Some(index) = character
            .aliases
            .iter()
            .position(|existing| existing.eq_ignore_ascii_case(alias.trim()))
        else {
            return Err(AppError::Domain("character alias does not exist".into()));
        };
        character.aliases.remove(index);
        self.update_character(&character)
    }
    pub fn create_location(&mut self, name: &str) -> Result<LocationId> {
        let id = LocationId::new();
        self.store.add_location(id, name)?;
        Ok(id)
    }
    pub fn get_location(&self, id: story_core::LocationId) -> Result<story_core::Location> {
        Ok(self.store.get_location(id)?)
    }
    pub fn update_location(&mut self, location: &story_core::Location) -> Result<()> {
        Ok(self.store.update_location(location)?)
    }
    pub fn create_plotline(&mut self, name: &str) -> Result<PlotlineId> {
        let id = PlotlineId::new();
        self.store.add_plotline(id, name)?;
        Ok(id)
    }
    pub fn link_character(
        &mut self,
        scene: SceneId,
        character: CharacterId,
        role: ParticipationRole,
    ) -> Result<()> {
        Ok(self.store.link_character(scene, character, role)?)
    }
    pub fn set_scene_plotlines(
        &mut self,
        scene: SceneId,
        links: &[ScenePlotline],
        policy: WordAttributionPolicy,
    ) -> Result<()> {
        Ok(self.store.set_scene_plotlines(scene, links, policy)?)
    }
    pub fn scene_plotline_links(&self, scene: SceneId) -> Result<Vec<ScenePlotline>> {
        Ok(self.store.scene_plotline_links(scene)?)
    }
    pub fn add_event(&mut self, title: &str, source_time: &str) -> Result<EventId> {
        let id = EventId::new();
        self.store.add_event(id, title, source_time)?;
        Ok(id)
    }
    pub fn add_story_event(&mut self, event: &StoryEvent) -> Result<()> {
        Ok(self.store.add_story_event(event)?)
    }
    pub fn update_story_event(&mut self, event: &StoryEvent) -> Result<()> {
        Ok(self.store.update_story_event(event)?)
    }
    pub fn update_story_event_time(
        &mut self,
        id: EventId,
        start: story_core::StoryTime,
        end: story_core::StoryTime,
        source_time_text: &str,
    ) -> Result<()> {
        Ok(self
            .store
            .update_story_event_time(id, start, end, source_time_text)?)
    }
    pub fn story_event_interval(&self, id: EventId) -> Result<StoryInterval> {
        let event = self
            .store
            .list_events()?
            .into_iter()
            .find(|event| event.id == id)
            .ok_or_else(|| AppError::Domain(format!("story event {id} does not exist")))?;
        let (start_boundary, end_boundary) = self.store.story_event_interval_boundaries(id)?;
        Ok(StoryInterval {
            start: event.start,
            end: event.end,
            start_boundary,
            end_boundary,
        })
    }
    pub fn update_story_event_interval(
        &mut self,
        id: EventId,
        interval: StoryInterval,
    ) -> Result<()> {
        Ok(self.store.update_story_event_interval(id, interval)?)
    }
    pub fn link_event(&mut self, scene: SceneId, event: EventId, kind: &str) -> Result<()> {
        Ok(self.store.link_event(scene, event, kind)?)
    }
    pub fn scene_event_links(&self, scene: SceneId) -> Result<Vec<(EventId, String)>> {
        Ok(self.store.scene_event_links(scene)?)
    }
    pub fn set_event_characters(
        &mut self,
        event: EventId,
        participants: &[EventCharacterParticipation],
    ) -> Result<()> {
        Ok(self.store.set_event_characters(event, participants)?)
    }
    pub fn event_characters(&self, event: EventId) -> Result<Vec<EventCharacterParticipation>> {
        Ok(self.store.event_characters(event)?)
    }
    pub fn set_event_locations(
        &mut self,
        event: EventId,
        locations: &[EventLocationAssociation],
    ) -> Result<()> {
        Ok(self.store.set_event_locations(event, locations)?)
    }
    pub fn set_event_relationships(
        &mut self,
        event: EventId,
        participants: &[EventCharacterParticipation],
        locations: &[EventLocationAssociation],
    ) -> Result<()> {
        Ok(self
            .store
            .set_event_relationships(event, participants, locations)?)
    }
    pub fn event_locations(&self, event: EventId) -> Result<Vec<EventLocationAssociation>> {
        Ok(self.store.event_locations(event)?)
    }
    pub fn add_event_dependency(&mut self, dependency: &StoryEventDependency) -> Result<()> {
        Ok(self.store.add_event_dependency(dependency)?)
    }
    pub fn list_event_dependencies(&self) -> Result<Vec<StoryEventDependency>> {
        Ok(self.store.list_event_dependencies()?)
    }
    pub fn update_event_dependency(&mut self, dependency: &StoryEventDependency) -> Result<()> {
        Ok(self.store.update_event_dependency(dependency)?)
    }
    pub fn remove_event_dependency(
        &mut self,
        predecessor: EventId,
        successor: EventId,
        kind: &str,
    ) -> Result<()> {
        Ok(self
            .store
            .remove_event_dependency(predecessor, successor, kind)?)
    }
    pub fn inspect_event_dependencies(
        &self,
    ) -> Result<Vec<(StoryEventDependency, story_analysis::InspectionResult)>> {
        let events = self.store.list_events()?;
        self.store
            .list_event_dependencies()
            .map(|dependencies| {
                dependencies
                    .into_iter()
                    .map(|dependency| {
                        let earlier = events
                            .iter()
                            .find(|event| event.id == dependency.predecessor);
                        let later = events.iter().find(|event| event.id == dependency.successor);
                        let comparison = earlier.zip(later).map(|(a, b)| a.end.compare(b.start));
                        let result = story_analysis::evaluate_event_precedence(
                            &dependency.predecessor.to_string(),
                            &dependency.successor.to_string(),
                            comparison,
                        );
                        (dependency, result)
                    })
                    .collect()
            })
            .map_err(Into::into)
    }
    pub fn add_location_incompatibility_rule(
        &mut self,
        rule: &LocationIncompatibilityRule,
    ) -> Result<()> {
        Ok(self.store.add_location_incompatibility_rule(rule)?)
    }
    pub fn list_location_incompatibility_rules(&self) -> Result<Vec<LocationIncompatibilityRule>> {
        Ok(self.store.list_location_incompatibility_rules()?)
    }
    pub fn update_location_incompatibility_rule(
        &mut self,
        rule: &LocationIncompatibilityRule,
    ) -> Result<()> {
        Ok(self.store.update_location_incompatibility_rule(rule)?)
    }
    pub fn remove_location_incompatibility_rule(&mut self, id: ContinuityRuleId) -> Result<()> {
        Ok(self.store.remove_location_incompatibility_rule(id)?)
    }
    pub fn inspect_location_incompatibilities(
        &self,
    ) -> Result<Vec<story_analysis::LocationRuleFinding>> {
        Ok(story_analysis::location_incompatibility_findings(
            &self.analytics_dataset()?,
            &self.store.list_location_incompatibility_rules()?,
        ))
    }
    pub fn create_passage_anchor(
        &mut self,
        scene: SceneId,
        start: u64,
        end: u64,
        quote: &str,
        prefix: &str,
        suffix: &str,
    ) -> Result<AnchorId> {
        Ok(self
            .store
            .create_anchor(scene, start, end, quote, prefix, suffix)?)
    }
    pub fn list_passage_anchors(
        &self,
        scene: SceneId,
    ) -> Result<Vec<story_storage::PassageAnchorRecord>> {
        Ok(self.store.list_scene_anchors(scene)?)
    }
    pub fn reanchor_passage(
        &mut self,
        anchor: story_core::AnchorId,
        start: u64,
        end: u64,
    ) -> Result<()> {
        Ok(self.store.reanchor_passage(anchor, start, end)?)
    }
    pub fn create_story_link(&mut self, link: &StoryLink) -> Result<()> {
        Ok(self.store.create_story_link(link)?)
    }
    pub fn delete_story_link(&mut self, id: LinkId) -> Result<()> {
        Ok(self.store.delete_story_link(id)?)
    }
    pub fn create_research_item(
        &mut self,
        title: &str,
        body: &str,
        media_type: &str,
        source_path: Option<&str>,
        source_hash: Option<&str>,
    ) -> Result<ResearchId> {
        Ok(self
            .store
            .create_research_item(title, body, media_type, source_path, source_hash)?)
    }
    pub fn list_research_items(&self) -> Result<Vec<story_storage::ResearchSummary>> {
        Ok(self.store.list_research_items()?)
    }
    pub fn load_research_text(&self, id: ResearchId) -> Result<String> {
        Ok(self.store.load_research_text(id)?)
    }
    pub fn create_issue(&mut self, title: &str) -> Result<IssueId> {
        Ok(self.store.create_issue(title)?)
    }
    pub fn set_issue_status(&mut self, issue: IssueId, status: &str) -> Result<()> {
        Ok(self.store.set_issue_status(issue, status)?)
    }
    pub fn link_issue(&mut self, scene: SceneId, issue: IssueId) -> Result<()> {
        Ok(self.store.link_issue(scene, issue)?)
    }
    pub fn list_issues(&self) -> Result<Vec<story_storage::IssueSummary>> {
        Ok(self.store.list_issues()?)
    }
    pub fn start_writing_challenge(
        &mut self,
        target_words: u32,
        duration_seconds: u32,
        mode: &str,
    ) -> Result<String> {
        Ok(self
            .store
            .start_writing_challenge(target_words, duration_seconds, mode)?)
    }
    pub fn finish_writing_challenge(
        &mut self,
        id: &str,
        words_written: u32,
        abandoned: bool,
    ) -> Result<()> {
        Ok(self
            .store
            .finish_writing_challenge(id, words_written, abandoned)?)
    }
    pub fn finish_writing_challenge_detailed(
        &mut self,
        id: &str,
        words_written: u32,
        words_deleted: u32,
        pauses_consumed: u32,
        outcome: &str,
    ) -> Result<()> {
        Ok(self.store.finish_writing_challenge_detailed(
            id,
            words_written,
            words_deleted,
            pauses_consumed,
            outcome,
        )?)
    }
    pub fn finish_writing_challenge_with_activity(
        &mut self,
        id: &str,
        words_written: u32,
        words_deleted: u32,
        pauses_consumed: u32,
        outcome: &str,
        scene_id: Option<SceneId>,
    ) -> Result<()> {
        Ok(self.store.finish_writing_challenge_with_activity(
            id,
            words_written,
            words_deleted,
            pauses_consumed,
            outcome,
            scene_id,
        )?)
    }
    pub fn list_writing_challenges(&self) -> Result<Vec<story_storage::WritingChallengeRecord>> {
        Ok(self.store.list_writing_challenges()?)
    }
    pub fn create_writing_goal(&mut self, goal: &story_storage::NewWritingGoal) -> Result<String> {
        Ok(self.store.create_writing_goal(goal)?)
    }
    pub fn update_writing_goal(
        &mut self,
        id: &str,
        title: &str,
        target_words: i64,
        baseline_words: i64,
        success_metric: &str,
    ) -> Result<()> {
        Ok(self.store.update_writing_goal(
            id,
            title,
            target_words,
            baseline_words,
            success_metric,
        )?)
    }
    pub fn archive_writing_goal(&mut self, id: &str) -> Result<()> {
        Ok(self.store.archive_writing_goal(id)?)
    }
    pub fn list_writing_goals(
        &self,
        include_archived: bool,
    ) -> Result<Vec<story_storage::WritingGoalRecord>> {
        Ok(self.store.list_writing_goals(include_archived)?)
    }
    pub fn record_writing_goal_event(
        &mut self,
        event: &story_storage::NewWritingGoalEvent,
    ) -> Result<String> {
        Ok(self.store.record_writing_goal_event(event)?)
    }
    pub fn record_writing_activity_for_goals(
        &mut self,
        activity: &story_storage::NewWritingActivity,
    ) -> Result<usize> {
        Ok(self.store.record_writing_activity_for_goals(activity)?)
    }
    pub fn list_writing_goal_events(
        &self,
        goal_id: &str,
    ) -> Result<Vec<story_storage::WritingGoalEventRecord>> {
        Ok(self.store.list_writing_goal_events(goal_id)?)
    }
    pub fn reconcile_writing_goal_event(
        &mut self,
        event_id: &str,
        expected_fingerprint: &str,
        corrected_raw_delta: i64,
        corrected_real_delta: i64,
        reason: &str,
    ) -> Result<String> {
        Ok(self.store.reconcile_writing_goal_event(
            event_id,
            expected_fingerprint,
            corrected_raw_delta,
            corrected_real_delta,
            reason,
        )?)
    }
    pub fn writing_goal_progress(
        &self,
        goal_id: &str,
        as_of_timestamp: i64,
    ) -> Result<Vec<story_storage::WritingGoalProgressPoint>> {
        Ok(self.store.writing_goal_progress(goal_id, as_of_timestamp)?)
    }
    pub fn recover_interrupted_writing_challenges(&mut self) -> Result<u64> {
        Ok(self.store.recover_interrupted_writing_challenges()?)
    }
    pub fn start_writing_session(
        &mut self,
        scene_id: Option<SceneId>,
        start_words: u32,
    ) -> Result<String> {
        Ok(self.store.start_writing_session(scene_id, start_words)?)
    }
    pub fn finish_writing_session(
        &mut self,
        id: &str,
        end_words: u32,
        abandoned: bool,
    ) -> Result<()> {
        Ok(self
            .store
            .finish_writing_session(id, end_words, abandoned)?)
    }
    pub fn list_writing_sessions(&self) -> Result<Vec<story_storage::WritingSessionRecord>> {
        Ok(self.store.list_writing_sessions()?)
    }
    pub fn list_characters(&self) -> Result<Vec<story_storage::CharacterSummary>> {
        Ok(self.store.list_characters()?)
    }
    pub fn list_locations(&self) -> Result<Vec<story_storage::LocationSummary>> {
        Ok(self.store.list_locations()?)
    }
    pub fn list_plotlines(&self) -> Result<Vec<story_storage::PlotlineSummary>> {
        Ok(self.store.list_plotlines()?)
    }
    pub fn list_events(&self) -> Result<Vec<story_storage::EventSummary>> {
        Ok(self.store.list_events()?)
    }
    pub fn import_reading_document(
        &mut self,
        document: &story_reading::ReferenceDocument,
        source: &str,
    ) -> Result<ReadingId> {
        Ok(self.store.import_reading_document(document, source)?)
    }
    pub fn set_reading_document_language(
        &mut self,
        id: ReadingId,
        language_code: Option<&str>,
    ) -> Result<()> {
        Ok(self
            .store
            .set_reading_document_language(id, language_code)?)
    }
    /// Applies a previously inspected manuscript staging object to a new project.
    /// Callers should import into a fresh destination so any failed write can
    /// be discarded without touching the source document.
    pub fn import_staged_manuscript(
        &mut self,
        staging: &story_import::ManuscriptStaging,
    ) -> Result<(ImportRunId, Vec<SceneId>)> {
        let source_type = match &staging.source_type {
            story_import::SourceType::Scrivener => "scrivener",
            story_import::SourceType::PlainText => "plain-text",
            story_import::SourceType::Markdown => "markdown",
            story_import::SourceType::Org => "org",
            story_import::SourceType::Docx => "docx",
            story_import::SourceType::Rtf => "rtf",
            story_import::SourceType::Odt => "odt",
            story_import::SourceType::Epub => "epub",
            _ => {
                return Err(AppError::Domain(
                    "unsupported manuscript import source type".into(),
                ));
            }
        };
        let run = ImportRunId::new();
        self.store.record_import_run(
            run,
            source_type,
            &staging.source_path,
            &staging.source_fingerprint,
        )?;
        let mut imported = Vec::new();
        let mut unsupported = staging.unsupported.clone();
        let mut chapter_position = self
            .list_structural_nodes()?
            .iter()
            .filter(|node| node.parent_id.is_none())
            .count() as u32;
        let mut current_chapter = None;
        let mut scene_position = 0u32;
        for (position, unit) in staging.units.iter().enumerate() {
            let nested_org_heading = source_type == "org"
                && unit
                    .heading_level
                    .is_some_and(|heading_level| heading_level >= 2);
            let chapter = if nested_org_heading {
                if current_chapter.is_none() {
                    let chapter = self.create_chapter(None, "Imported Text", chapter_position)?;
                    chapter_position = chapter_position
                        .checked_add(1)
                        .ok_or_else(|| AppError::Domain("chapter position overflow".into()))?;
                    current_chapter = Some(chapter);
                    scene_position = 0;
                }
                current_chapter.ok_or_else(|| {
                    AppError::Domain("Org heading could not be assigned to a chapter".into())
                })?
            } else {
                let chapter_title = if source_type == "org" && unit.heading_level == Some(1) {
                    unit.title.clone()
                } else {
                    format!("Chapter {}", position + 1)
                };
                let chapter = self.create_chapter(None, &chapter_title, chapter_position)?;
                chapter_position = chapter_position
                    .checked_add(1)
                    .ok_or_else(|| AppError::Domain("chapter position overflow".into()))?;
                current_chapter = Some(chapter);
                scene_position = 0;
                chapter
            };
            let scene_title = if source_type == "org" && unit.heading_level == Some(1) {
                format!("{} — opening", unit.title)
            } else {
                unit.title.clone()
            };
            let scene = self.create_scene(chapter, &scene_title, scene_position)?;
            scene_position = scene_position
                .checked_add(1)
                .ok_or_else(|| AppError::Domain("scene position overflow".into()))?;
            let (_, current_hash) = self.load_scene_text(scene)?;
            self.save_scene_text(scene, Some(&current_hash), &unit.body_markdown)?;
            let source_item = SourceItemId::new();
            let source_fingerprint = format!("{:x}", Sha256::digest(unit.body_markdown.as_bytes()));
            let raw_fields =
                serde_json::to_string(unit).map_err(|error| AppError::Domain(error.to_string()))?;
            self.store
                .record_source_item(story_storage::ImportSourceItemRecord {
                    id: source_item,
                    run,
                    source_document_id: &unit.source_document_id,
                    source_sheet: None,
                    source_row: Some(position as u32 + 1),
                    fingerprint: &source_fingerprint,
                    inventory_type: "manuscript-unit",
                    raw_fields: &raw_fields,
                })?;
            for (source_field, destination_field) in
                [("title", "title"), ("body_markdown", "manuscript-text")]
            {
                self.store
                    .record_provenance(story_storage::EntityProvenanceRecord {
                        id: ProvenanceId::new(),
                        source_item,
                        destination_type: "scene",
                        destination_id: &scene.to_string(),
                        source_field: Some(source_field),
                        destination_field: Some(destination_field),
                        mapping: "manuscript-import-v1",
                    })?;
            }
            unsupported.extend(unit.unsupported.iter().cloned());
            imported.push(scene);
        }
        unsupported.sort();
        unsupported.dedup();
        let status = if unsupported.is_empty() && staging.warnings.is_empty() {
            "completed"
        } else {
            "incomplete"
        };
        self.store.finish_import_run(
            run,
            status,
            &serde_json::to_string(&staging.warnings)
                .map_err(|error| AppError::Domain(error.to_string()))?,
            &serde_json::to_string(&unsupported)
                .map_err(|error| AppError::Domain(error.to_string()))?,
        )?;
        Ok((run, imported))
    }

    /// Imports a previously inventoried Scrivener binder using explicit
    /// preview decisions. This does not imply format compatibility: callers
    /// must validate a real source fixture before presenting the operation as
    /// supported. The destination must be a fresh project.
    pub fn import_scrivener_inventory(
        &mut self,
        inventory: &story_import::ScrivenerProjectInventory,
        plan: &story_import::ScrivenerImportPlan,
        mode: story_import::ScrivenerImportMode,
    ) -> Result<(ImportRunId, Vec<SceneId>)> {
        use story_import::ScrivenerDestinationKind as Target;

        inventory
            .validate()
            .map_err(|error| AppError::Domain(error.to_string()))?;
        plan.validate_for_import(inventory)
            .map_err(|error| AppError::Domain(error.to_string()))?;
        let preview = plan
            .preview(inventory)
            .map_err(|error| AppError::Domain(error.to_string()))?;
        let decisions = plan
            .decisions
            .iter()
            .map(|decision| (decision.source_document_id.as_str(), decision))
            .collect::<std::collections::BTreeMap<_, _>>();
        for item in &inventory.items {
            let decision = decisions
                .get(item.source_document_id.as_str())
                .ok_or_else(|| AppError::Domain("Scrivener import plan is incomplete".into()))?;
            if let Some(parent) = item.parent_source_id.as_deref() {
                let parent_decision = decisions.get(parent).ok_or_else(|| {
                    AppError::Domain(format!("missing mapping for binder parent {parent}"))
                })?;
                if parent_decision.destination_kind == Target::Skip
                    && decision.destination_kind != Target::Skip
                {
                    return Err(AppError::Domain(format!(
                        "binder item {} is selected under a skipped parent",
                        item.source_document_id
                    )));
                }
            }
        }

        let run = ImportRunId::new();
        let mut provenance_source = inventory.source_path.clone();
        if mode == story_import::ScrivenerImportMode::Both {
            let source = Path::new(&inventory.source_path);
            let current_source = story_import::inspect_scrivener_project(source)
                .map_err(|error| AppError::Domain(error.to_string()))?;
            if current_source.source_fingerprint != inventory.source_fingerprint {
                return Err(AppError::Domain(
                    "Scrivener source changed after preview; inspect it again before importing"
                        .into(),
                ));
            }
            let snapshot = self
                .project_root()
                .join("research/imported_sources")
                .join(format!("{run}.scriv"));
            copy_scrivener_source_tree(source, &snapshot)?;
            let copied = story_import::inspect_scrivener_project(&snapshot)
                .map_err(|error| AppError::Domain(error.to_string()))?;
            if copied.source_fingerprint != inventory.source_fingerprint {
                let _ = fs::remove_dir_all(&snapshot);
                return Err(AppError::Domain(
                    "Scrivener source snapshot failed fingerprint verification".into(),
                ));
            }
            provenance_source = format!("research/imported_sources/{run}.scriv");
        }
        self.store.record_import_run(
            run,
            "scrivener",
            &provenance_source,
            &inventory.source_fingerprint,
        )?;
        let mut structural_ids = std::collections::BTreeMap::<String, (NodeId, Target)>::new();
        let mut structural_positions = std::collections::BTreeMap::<Option<NodeId>, u32>::new();
        for existing in self.list_structural_nodes()? {
            let next = existing.position.checked_add(1).ok_or_else(|| {
                AppError::Domain("structural ordering has no available position".into())
            })?;
            let position = structural_positions.entry(existing.parent_id).or_default();
            *position = (*position).max(next);
        }
        let mut scene_positions = std::collections::BTreeMap::<NodeId, u32>::new();
        let mut imported_scenes = Vec::new();
        let mut unsupported = preview.unsupported.clone();

        for item in &inventory.items {
            let decision = decisions[item.source_document_id.as_str()];
            let title = decision
                .title_override
                .as_deref()
                .unwrap_or(item.title.as_str());
            let mut destinations = Vec::new();
            match decision.destination_kind {
                Target::Skip => {}
                Target::Unmapped => unreachable!("preview rejected unresolved mappings"),
                Target::Section => {
                    let parent = match item.parent_source_id.as_deref() {
                        None => None,
                        Some(source_parent) => Some(
                            structural_ids
                                .get(source_parent)
                                .filter(|(_, kind)| *kind == Target::Section)
                                .map(|(id, _)| *id)
                                .ok_or_else(|| {
                                    AppError::Domain(format!(
                                        "section {} must be nested under a mapped section",
                                        item.source_document_id
                                    ))
                                })?,
                        ),
                    };
                    let position = *structural_positions.get(&parent).unwrap_or(&0);
                    let node =
                        self.store
                            .create_node(parent, NodeKind::Section, title, position)?;
                    structural_positions.insert(parent, position.saturating_add(1));
                    structural_ids.insert(item.source_document_id.clone(), (node, Target::Section));
                    destinations.push(("structural_node", node.to_string(), "section"));
                    if let Some(text) = item.content_markdown.as_deref()
                        && !text.is_empty()
                    {
                        let unit = self.create_content_unit(
                            ContentUnitKind::SectionBody,
                            title,
                            Some(node),
                            0,
                            None,
                        )?;
                        self.save_content_unit_text(unit, None, text)?;
                        destinations.push(("content_unit", unit.to_string(), "content_markdown"));
                    }
                }
                Target::Chapter => {
                    let parent = match item.parent_source_id.as_deref() {
                        None => None,
                        Some(source_parent) => Some(
                            structural_ids
                                .get(source_parent)
                                .filter(|(_, kind)| *kind == Target::Section)
                                .map(|(id, _)| *id)
                                .ok_or_else(|| {
                                    AppError::Domain(format!(
                                        "chapter {} must be nested under a mapped section",
                                        item.source_document_id
                                    ))
                                })?,
                        ),
                    };
                    let position = *structural_positions.get(&parent).unwrap_or(&0);
                    let node = self.create_chapter(parent, title, position)?;
                    structural_positions.insert(parent, position.saturating_add(1));
                    structural_ids.insert(item.source_document_id.clone(), (node, Target::Chapter));
                    if let Some(text) = item.content_markdown.as_deref()
                        && !text.is_empty()
                    {
                        let unit = self.create_content_unit(
                            ContentUnitKind::ChapterBody,
                            title,
                            Some(node),
                            0,
                            None,
                        )?;
                        self.save_content_unit_text(unit, None, text)?;
                        destinations.push(("content_unit", unit.to_string(), "content_markdown"));
                    }
                    destinations.push(("structural_node", node.to_string(), "chapter"));
                }
                Target::Scene => {
                    let chapter = item
                        .parent_source_id
                        .as_deref()
                        .and_then(|parent| structural_ids.get(parent))
                        .filter(|(_, kind)| *kind == Target::Chapter)
                        .map(|(id, _)| *id)
                        .ok_or_else(|| {
                            AppError::Domain(format!(
                                "scene {} must have an explicitly mapped chapter parent",
                                item.source_document_id
                            ))
                        })?;
                    let position = *scene_positions.get(&chapter).unwrap_or(&0);
                    let scene = self.create_scene(chapter, title, position)?;
                    scene_positions.insert(chapter, position.saturating_add(1));
                    let mut metadata = self.get_scene(scene)?.scene;
                    metadata.synopsis = item.synopsis.clone().unwrap_or_default();
                    self.update_scene_metadata(&metadata)?;
                    if let Some(text) = item.content_markdown.as_deref() {
                        let (_, expected) = self.load_scene_text(scene)?;
                        self.save_scene_text(scene, Some(&expected), text)?;
                    }
                    if let Some(notes) = item.notes_markdown.as_deref()
                        && !notes.is_empty()
                    {
                        let bytes = notes.as_bytes().to_vec();
                        let reference = story_reading::ReferenceDocument {
                            title: format!("{} — Scrivener notes", title),
                            media_type: "text/markdown".into(),
                            sha256: format!("{:x}", Sha256::digest(&bytes)),
                            source_bytes: bytes,
                            extracted_text: notes.to_owned(),
                            language_code: None,
                        };
                        let reading = self.import_reading_document(
                            &reference,
                            &format!(
                                "{}#{}:notes",
                                inventory.source_path, item.source_document_id
                            ),
                        )?;
                        destinations.push(("reading", reading.to_string(), "notes"));
                    }
                    imported_scenes.push(scene);
                    destinations.push(("scene", scene.to_string(), "scene"));
                }
                Target::Research => {
                    let source_text = item
                        .content_markdown
                        .as_deref()
                        .or(item.notes_markdown.as_deref())
                        .or(item.synopsis.as_deref())
                        .unwrap_or_default();
                    let bytes = source_text.as_bytes().to_vec();
                    let reference = story_reading::ReferenceDocument {
                        title: title.to_owned(),
                        media_type: "text/markdown".into(),
                        sha256: format!("{:x}", Sha256::digest(&bytes)),
                        source_bytes: bytes,
                        extracted_text: source_text.to_owned(),
                        language_code: None,
                    };
                    let id = self.import_reading_document(
                        &reference,
                        &format!("{}#{}", inventory.source_path, item.source_document_id),
                    )?;
                    destinations.push(("reading", id.to_string(), "research"));
                }
            }

            let source_item = SourceItemId::new();
            let raw_fields = serde_json::to_string(&serde_json::json!({
                "binder_item": item,
                "mapping_decision": decision
            }))
            .map_err(|error| AppError::Domain(error.to_string()))?;
            let fingerprint = format!("{:x}", Sha256::digest(raw_fields.as_bytes()));
            self.store
                .record_source_item(story_storage::ImportSourceItemRecord {
                    id: source_item,
                    run,
                    source_document_id: &item.source_document_id,
                    source_sheet: None,
                    source_row: Some(item.position.saturating_add(1)),
                    fingerprint: &fingerprint,
                    inventory_type: item.type_code.as_deref().unwrap_or("binder-item"),
                    raw_fields: &raw_fields,
                })?;
            for (destination_type, destination_id, mapped_type) in destinations {
                self.store
                    .record_provenance(story_storage::EntityProvenanceRecord {
                        id: ProvenanceId::new(),
                        source_item,
                        destination_type,
                        destination_id: &destination_id,
                        source_field: None,
                        destination_field: Some(mapped_type),
                        mapping: "scrivener-preview-v1",
                    })?;
            }
            unsupported.extend(item.unsupported.iter().cloned());
        }

        // Keep imports deterministic and preserve binder order separately from
        // fictional chronology.
        unsupported.sort();
        unsupported.dedup();
        let status = if unsupported.is_empty() && inventory.warnings.is_empty() {
            "completed"
        } else {
            "incomplete"
        };
        self.store.finish_import_run(
            run,
            status,
            &format!(
                "{} binder items; {} manuscript entries; {} research references; {} skipped; warnings: {}",
                inventory.items.len(),
                preview.manuscript_items,
                preview.research_items,
                preview.skipped_items,
                inventory.warnings.join("; ")
            ),
            &unsupported.join("; "),
        )?;
        Ok((run, imported_scenes))
    }
    pub fn get_import_run_summary(
        &self,
        run: ImportRunId,
    ) -> Result<story_storage::ImportRunSummary> {
        Ok(self.store.get_import_run_summary(run)?)
    }
    pub fn merge_review_text(
        &mut self,
        scene: SceneId,
        base: &str,
        reviewed_text: &str,
    ) -> Result<story_import::ThreeWayMerge> {
        let (current, expected_hash) = self.load_scene_text(scene)?;
        let result = story_import::three_way_merge(base, &current, reviewed_text);
        if let Some(merged) = &result.merged_text {
            self.save_scene_text(scene, Some(&expected_hash), merged)?;
        }
        Ok(result)
    }
    pub fn resolve_review_text(
        &mut self,
        scene: SceneId,
        base: &str,
        reviewed_text: &str,
        choices: &[story_import::MergeConflictChoice],
    ) -> Result<String> {
        let (current, expected_hash) = self.load_scene_text(scene)?;
        let merged = story_import::resolve_three_way_merge(base, &current, reviewed_text, choices)
            .map_err(AppError::Domain)?;
        self.save_scene_text(scene, Some(&expected_hash), &merged)?;
        Ok(merged)
    }
    pub fn merge_docx_review(
        &mut self,
        scene: SceneId,
        base: &str,
        review_file: impl AsRef<Path>,
        accept_changes: bool,
    ) -> Result<story_import::ThreeWayMerge> {
        let reviewed = story_import::docx_review_text(review_file, accept_changes)
            .map_err(|error| AppError::Domain(error.to_string()))?;
        self.merge_review_text(scene, base, &reviewed)
    }
    pub fn inspect_docx_review(
        &self,
        review_file: impl AsRef<Path>,
    ) -> Result<Vec<story_import::DocxReviewItem>> {
        story_import::inspect_docx_review(review_file)
            .map_err(|error| AppError::Domain(error.to_string()))
    }
    pub fn get_reading_document(
        &self,
        id: ReadingId,
    ) -> Result<story_storage::ReadingDocumentRecord> {
        Ok(self.store.get_reading_document(id)?)
    }
    pub fn list_reading_documents(&self) -> Result<Vec<story_storage::ReadingDocumentRecord>> {
        Ok(self.store.list_reading_documents()?)
    }
    pub fn reload_reading_document(
        &mut self,
        id: ReadingId,
        document: &story_reading::ReferenceDocument,
    ) -> Result<()> {
        Ok(self.store.reload_reading_document(id, document)?)
    }
    pub fn search_reading(&self, terms: &str) -> Result<Vec<(ReadingId, String, String)>> {
        Ok(self.store.search_reading(terms)?)
    }
    pub fn create_reading_annotation(
        &mut self,
        id: ReadingId,
        draft: &ReadingAnnotationDraft,
    ) -> Result<AnnotationId> {
        Ok(self.store.create_reading_annotation(id, draft)?)
    }
    pub fn list_reading_annotations(
        &self,
        id: ReadingId,
    ) -> Result<Vec<story_storage::ReadingAnnotationRecord>> {
        Ok(self.store.list_reading_annotations(id)?)
    }
    pub fn create_reading_bookmark(
        &mut self,
        id: ReadingId,
        title: &str,
        offset: u64,
    ) -> Result<BookmarkId> {
        Ok(self.store.create_reading_bookmark(id, title, offset)?)
    }
    pub fn list_reading_bookmarks(
        &self,
        id: ReadingId,
    ) -> Result<Vec<story_storage::ReadingBookmarkRecord>> {
        Ok(self.store.list_reading_bookmarks(id)?)
    }
    pub fn set_reading_checkpoint(&mut self, id: ReadingId, progress: u64) -> Result<()> {
        Ok(self.store.set_reading_checkpoint(id, progress)?)
    }
    pub fn reading_checkpoint(&self, id: ReadingId) -> Result<Option<u64>> {
        Ok(self.store.reading_checkpoint(id)?)
    }
    pub fn create_reading_term(
        &mut self,
        id: ReadingId,
        kind: &str,
        name: &str,
        aliases: &str,
        description: &str,
    ) -> Result<ReadingTermId> {
        Ok(self
            .store
            .create_reading_term(id, kind, name, aliases, description)?)
    }
    pub fn list_reading_terms(
        &self,
        id: ReadingId,
    ) -> Result<Vec<story_storage::ReadingTermRecord>> {
        Ok(self.store.list_reading_terms(id)?)
    }
    pub fn link_reading_entity(
        &mut self,
        doc: ReadingId,
        annotation: Option<AnnotationId>,
        target: EntityRef,
        relation: &str,
    ) -> Result<()> {
        Ok(self
            .store
            .link_reading_entity(doc, annotation, target, relation)?)
    }
    pub fn reading_entity_links(
        &self,
        document: ReadingId,
    ) -> Result<Vec<story_storage::ReadingEntityLinkRecord>> {
        Ok(self.store.reading_entity_links(document)?)
    }
    pub fn backlinks(&self, target: EntityRef) -> Result<Vec<StoryLink>> {
        Ok(self.store.backlinks(target)?)
    }
    pub fn outgoing_links(&self, source: EntityRef) -> Result<Vec<StoryLink>> {
        Ok(self.store.outgoing_links(source)?)
    }
    pub fn expand_scene_transclusions(
        &self,
        scene: SceneId,
    ) -> Result<Vec<story_storage::TranscludedPassage>> {
        self.expand_document_inclusions(EntityRef::Scene(scene))
    }
    pub fn expand_document_inclusions(
        &self,
        root: EntityRef,
    ) -> Result<Vec<story_storage::TranscludedPassage>> {
        Ok(self.store.expand_document_inclusions(root)?)
    }
    pub fn broken_links(&self) -> Result<Vec<story_storage::BrokenLink>> {
        Ok(self.store.broken_links()?)
    }
    pub fn save_scene_query(&mut self, query: &SavedSceneQuery) -> Result<()> {
        Ok(self.store.save_scene_query(query)?)
    }
    pub fn set_scene_query_expression(
        &mut self,
        query: QueryId,
        expression: Option<&QueryExpression>,
    ) -> Result<()> {
        Ok(self.store.set_scene_query_expression(query, expression)?)
    }
    pub fn get_scene_query_expression(&self, query: QueryId) -> Result<Option<QueryExpression>> {
        Ok(self.store.get_scene_query_expression(query)?)
    }
    pub fn list_saved_queries(&self) -> Result<Vec<story_storage::SavedQuerySummary>> {
        Ok(self.store.list_saved_queries()?)
    }
    pub fn run_scene_query(&self, query: QueryId) -> Result<Vec<SceneId>> {
        Ok(self.store.run_scene_query(query)?)
    }
    pub fn explain_scene_query_match(
        &self,
        query: QueryId,
        scene: SceneId,
    ) -> Result<Vec<story_core::QueryEvidence>> {
        Ok(self.store.explain_scene_query_match(query, scene)?)
    }
    pub fn record_character_absence(
        &mut self,
        scene: SceneId,
        character: CharacterId,
        notes: &str,
    ) -> Result<()> {
        Ok(self
            .store
            .record_character_absence(scene, character, notes)?)
    }
    pub fn clear_character_absence(
        &mut self,
        scene: SceneId,
        character: CharacterId,
    ) -> Result<()> {
        Ok(self.store.clear_character_absence(scene, character)?)
    }
    pub fn set_query_assertions(
        &mut self,
        query: QueryId,
        assertions: &[QueryAssertion],
    ) -> Result<()> {
        Ok(self.store.set_query_assertions(query, assertions)?)
    }
    pub fn get_query_assertions(&self, query: QueryId) -> Result<Vec<QueryAssertion>> {
        Ok(self.store.get_query_assertions(query)?)
    }
    pub fn search_scenes(&self, terms: &str) -> Result<Vec<SceneId>> {
        Ok(self.store.search_scenes(terms)?)
    }
    pub fn search_project(
        &self,
        terms: &str,
        limit: u32,
    ) -> Result<Vec<story_storage::ProjectSearchHit>> {
        Ok(self.store.search_project(terms, limit)?)
    }
    pub fn rebuild_project_search_index(&mut self) -> Result<()> {
        Ok(self.store.rebuild_project_search_index()?)
    }
    pub fn define_custom_field(&mut self, def: &CustomFieldDefinition) -> Result<()> {
        Ok(self.store.define_custom_field(def)?)
    }
    pub fn list_custom_field_definitions(&self) -> Result<Vec<CustomFieldDefinition>> {
        Ok(self.store.list_custom_field_definitions()?)
    }
    pub fn set_custom_field(
        &mut self,
        scene: SceneId,
        field: FieldId,
        value: &FieldValue,
    ) -> Result<()> {
        Ok(self.store.set_custom_field(scene, field, value)?)
    }
    pub fn filter_scenes_by_custom_field(
        &self,
        field: FieldId,
        value: &FieldValue,
    ) -> Result<Vec<SceneId>> {
        Ok(self.store.filter_scenes_by_custom_field(field, value)?)
    }
    pub fn save_scene_text(
        &mut self,
        id: SceneId,
        expected_hash: Option<&str>,
        text: &str,
    ) -> Result<String> {
        Ok(self.store.save_text(id, expected_hash, text)?)
    }
    pub fn create_story_object(&mut self, name: &str, description: &str) -> Result<StoryObjectId> {
        Ok(self.store.create_story_object(name, description)?)
    }
    pub fn list_story_objects(&self) -> Result<Vec<story_storage::StoryObjectRecord>> {
        Ok(self.store.list_story_objects()?)
    }
    pub fn list_scene_mentions(
        &self,
        scene: SceneId,
    ) -> Result<Vec<story_storage::DetectedMentionRecord>> {
        Ok(self.store.list_scene_mentions(scene)?)
    }
    pub fn confirmed_entity_mention_appearances(
        &self,
        kind: MentionEntityKind,
        entity_id: &str,
    ) -> Result<Vec<EntityMentionAppearance>> {
        let mut appearances = Vec::new();
        for scene in self.store.list_scenes(false)? {
            for mention in self.store.list_scene_mentions(scene.scene.id)? {
                if mention.state == "confirmed"
                    && mention.entity_kind.as_deref() == Some(kind.as_str())
                    && mention.entity_id.as_deref() == Some(entity_id)
                {
                    appearances.push(EntityMentionAppearance {
                        scene_id: scene.scene.id,
                        scene_title: scene.scene.title.clone(),
                        mention,
                    });
                }
            }
        }
        Ok(appearances)
    }

    pub fn reindex_scene_mentions(&mut self, scene: SceneId, text: &str, hash: &str) -> Result<()> {
        Ok(self.store.reindex_scene_mentions(scene, text, hash)?)
    }
    pub fn confirm_scene_mention(
        &mut self,
        mention: MentionId,
        kind: MentionEntityKind,
        entity_id: &str,
    ) -> Result<()> {
        Ok(self.store.confirm_scene_mention(mention, kind, entity_id)?)
    }
    pub fn dismiss_scene_mention(&mut self, mention: MentionId) -> Result<()> {
        Ok(self.store.dismiss_scene_mention(mention)?)
    }
    pub fn create_personal_comment(
        &mut self,
        scene: SceneId,
        start: u64,
        end: u64,
        quote: &str,
        body: &str,
    ) -> Result<PersonalCommentId> {
        Ok(self
            .store
            .create_personal_comment(scene, start, end, quote, body)?)
    }
    pub fn create_assembly(&mut self, title: &str) -> Result<AssemblyId> {
        Ok(self.store.create_assembly(title)?)
    }

    pub fn create_assembly_draft(&mut self, assembly: AssemblyId) -> Result<AssemblyDraft> {
        Ok(self.store.create_assembly_draft(assembly)?)
    }

    pub fn load_assembly_draft(&self, id: AssemblyDraftId) -> Result<AssemblyDraft> {
        Ok(self.store.load_assembly_draft(id)?)
    }

    pub fn list_assembly_drafts(&self) -> Result<Vec<AssemblyDraft>> {
        Ok(self.store.list_assembly_drafts()?)
    }

    pub fn update_assembly_draft_item(
        &mut self,
        draft: AssemblyDraftId,
        scene: SceneId,
        text: &str,
    ) -> Result<()> {
        Ok(self.store.update_assembly_draft_item(draft, scene, text)?)
    }

    pub fn replace_assembly_draft_items(
        &mut self,
        draft: AssemblyDraftId,
        items: &[(SceneId, String)],
    ) -> Result<()> {
        Ok(self.store.replace_assembly_draft_items(draft, items)?)
    }

    pub fn commit_assembly_draft(&mut self, draft: AssemblyDraftId) -> Result<()> {
        Ok(self.store.commit_assembly_draft(draft)?)
    }

    pub fn create_planning_board(&mut self, title: &str) -> Result<PlanningBoardId> {
        Ok(self.store.create_planning_board(title)?)
    }

    pub fn list_planning_boards(&self) -> Result<Vec<story_storage::PlanningBoardSummary>> {
        Ok(self.store.list_planning_boards()?)
    }

    pub fn save_story_bible_template(&self, template: &StoryBibleTemplate) -> Result<()> {
        Ok(self.store.save_story_bible_template(template)?)
    }

    pub fn list_story_bible_templates(&self) -> Result<Vec<StoryBibleTemplate>> {
        Ok(self.store.list_story_bible_templates()?)
    }

    pub fn load_story_bible_template(
        &self,
        id: StoryBibleTemplateId,
    ) -> Result<StoryBibleTemplate> {
        Ok(self.store.load_story_bible_template(id)?)
    }

    pub fn save_story_bible_profile(&mut self, profile: &StoryBibleProfile) -> Result<()> {
        Ok(self.store.save_story_bible_profile(profile)?)
    }

    pub fn load_story_bible_profile(
        &self,
        kind: StoryBibleEntityKind,
        id: &str,
    ) -> Result<StoryBibleProfile> {
        Ok(self.store.load_story_bible_profile(kind, id)?)
    }

    pub fn add_planning_board_item(
        &mut self,
        board: PlanningBoardId,
        kind: &str,
        entity_id: &str,
        x: f64,
        y: f64,
        note: &str,
    ) -> Result<PlanningBoardItemId> {
        Ok(self
            .store
            .add_planning_board_item(board, kind, entity_id, x, y, note)?)
    }

    pub fn list_planning_board_items(
        &self,
        board: PlanningBoardId,
    ) -> Result<Vec<story_storage::PlanningBoardItemRecord>> {
        Ok(self.store.list_planning_board_items(board)?)
    }

    pub fn list_planning_board_connectors(
        &self,
        board: PlanningBoardId,
    ) -> Result<Vec<story_storage::PlanningBoardConnectorRecord>> {
        Ok(self.store.list_planning_board_connectors(board)?)
    }

    pub fn planning_board_svg(&self, board: PlanningBoardId) -> Result<String> {
        let summary = self
            .list_planning_boards()?
            .into_iter()
            .find(|item| item.id == board)
            .ok_or_else(|| StorageError::NotFound(board.to_string()))?;
        let items = self.list_planning_board_items(board)?;
        let connectors = self.list_planning_board_connectors(board)?;
        if items.iter().any(|item| {
            [item.x, item.y, item.width, item.height]
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1.0e9)
        }) {
            return Err(StorageError::Invalid(
                "planning board coordinates exceed the SVG export range".into(),
            )
            .into());
        }
        let min_x = items.iter().map(|item| item.x).fold(0.0_f64, f64::min) - 12.0;
        let min_y = items.iter().map(|item| item.y).fold(0.0_f64, f64::min) - 12.0;
        let max_x = items
            .iter()
            .map(|item| item.x + item.width)
            .fold(240.0_f64, f64::max)
            + 12.0;
        let max_y = items
            .iter()
            .map(|item| item.y + item.height)
            .fold(160.0_f64, f64::max)
            + 12.0;
        let width = max_x - min_x;
        let height = max_y - min_y;
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(StorageError::Invalid(
                "planning board dimensions cannot be rendered".into(),
            )
            .into());
        }
        let positions = items
            .iter()
            .map(|item| (item.id, item))
            .collect::<std::collections::HashMap<_, _>>();
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{min_x} {min_y} {width} {height}\" role=\"img\" aria-label=\"{}\">\n<title>{}</title>\n<rect x=\"{min_x}\" y=\"{min_y}\" width=\"{width}\" height=\"{height}\" fill=\"white\"/>\n",
            escape_svg_text(&summary.title),
            escape_svg_text(&summary.title)
        );
        for connector in connectors {
            let source = positions
                .get(&connector.source_item_id)
                .ok_or_else(|| StorageError::Invalid("board connector source is missing".into()))?;
            let target = positions
                .get(&connector.target_item_id)
                .ok_or_else(|| StorageError::Invalid("board connector target is missing".into()))?;
            let (sx, sy) = (
                source.x + source.width / 2.0,
                source.y + source.height / 2.0,
            );
            let (tx, ty) = (
                target.x + target.width / 2.0,
                target.y + target.height / 2.0,
            );
            svg.push_str(&format!("<line x1=\"{sx}\" y1=\"{sy}\" x2=\"{tx}\" y2=\"{ty}\" stroke=\"#64748b\" stroke-width=\"0.8\" marker-end=\"url(#arrow)\"/>\n"));
            if !connector.label.is_empty() {
                svg.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" font-size=\"3\" fill=\"#334155\">{}</text>\n",
                    (sx + tx) / 2.0,
                    (sy + ty) / 2.0 - 1.0,
                    escape_svg_text(&connector.label)
                ));
            }
        }
        svg.push_str("<defs><marker id=\"arrow\" markerWidth=\"6\" markerHeight=\"6\" refX=\"5\" refY=\"3\" orient=\"auto\"><path d=\"M0,0 L6,3 L0,6 z\" fill=\"#64748b\"/></marker></defs>\n");
        for item in items {
            svg.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"1.5\" fill=\"#f8fafc\" stroke=\"#334155\" stroke-width=\"0.6\"/>\n", item.x, item.y, item.width, item.height));
            let title = format!("{} · {}", item.entity_kind, item.entity_id);
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"3.2\" font-weight=\"bold\">{}</text>\n",
                item.x + 2.0,
                item.y + 3.8,
                escape_svg_text(&title)
            ));
            for (index, line) in item.note.lines().take(3).enumerate() {
                svg.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" font-size=\"2.8\">{}</text>\n",
                    item.x + 2.0,
                    item.y + 7.5 + index as f64 * 3.4,
                    escape_svg_text(line)
                ));
            }
        }
        svg.push_str("</svg>\n");
        Ok(svg)
    }

    pub fn move_planning_board_item(
        &mut self,
        item: PlanningBoardItemId,
        x: f64,
        y: f64,
    ) -> Result<()> {
        Ok(self.store.move_planning_board_item(item, x, y)?)
    }
    pub fn resize_planning_board_item(
        &mut self,
        item: PlanningBoardItemId,
        width: f64,
        height: f64,
    ) -> Result<()> {
        Ok(self.store.resize_planning_board_item(item, width, height)?)
    }

    pub fn connect_planning_board_items(
        &mut self,
        board: PlanningBoardId,
        source: PlanningBoardItemId,
        target: PlanningBoardItemId,
        label: &str,
    ) -> Result<PlanningBoardConnectorId> {
        Ok(self
            .store
            .connect_planning_board_items(board, source, target, label)?)
    }
    pub fn list_assemblies(&self) -> Result<Vec<story_storage::AssemblySummary>> {
        Ok(self.store.list_assemblies()?)
    }
    pub fn list_assembly_items(
        &self,
        assembly: AssemblyId,
    ) -> Result<Vec<story_storage::AssemblyItemRecord>> {
        Ok(self.store.list_assembly_items(assembly)?)
    }
    pub fn add_assembly_scene(
        &mut self,
        assembly: AssemblyId,
        scene: SceneId,
        position: u32,
    ) -> Result<AssemblyItemId> {
        Ok(self.store.add_assembly_scene(assembly, scene, position)?)
    }
    pub fn add_assembly_content_unit(
        &mut self,
        assembly: AssemblyId,
        content: ContentUnitId,
        position: u32,
    ) -> Result<AssemblyItemId> {
        Ok(self
            .store
            .add_assembly_content_unit(assembly, content, position)?)
    }
    pub fn move_assembly_item(
        &mut self,
        assembly: AssemblyId,
        item: AssemblyItemId,
        position: u32,
    ) -> Result<()> {
        Ok(self.store.move_assembly_item(assembly, item, position)?)
    }
    pub fn set_assembly_item_included(
        &mut self,
        item: AssemblyItemId,
        included: bool,
    ) -> Result<()> {
        Ok(self.store.set_assembly_item_included(item, included)?)
    }
    pub fn compile_assembly_ir(&self, assembly: AssemblyId) -> Result<story_document::CompileIr> {
        let assembly_record = self
            .store
            .list_assemblies()?
            .into_iter()
            .find(|item| item.id == assembly)
            .ok_or_else(|| StorageError::NotFound(assembly.to_string()))?;
        let project = self.get_project()?;
        let mut ir = story_document::CompileIr::new(&assembly_record.title);
        ir.author = (!project.author.trim().is_empty()).then_some(project.author.clone());
        ir.language_code = Some(project.language_code.clone());
        let nodes = self
            .store
            .list_structural_nodes()?
            .into_iter()
            .map(|node| {
                (
                    node.id.to_string(),
                    (
                        node.parent_id.map(|id| id.to_string()),
                        match node.kind.as_str() {
                            "section" => story_document::StructuralKind::Section,
                            "chapter" => story_document::StructuralKind::Chapter,
                            _ => story_document::StructuralKind::Group,
                        },
                        node.title,
                    ),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut inserted_groups = std::collections::BTreeSet::new();
        fn add_ancestors(
            ir: &mut story_document::CompileIr,
            nodes: &std::collections::BTreeMap<
                String,
                (Option<String>, story_document::StructuralKind, String),
            >,
            inserted_groups: &mut std::collections::BTreeSet<String>,
            parent: Option<String>,
        ) -> Option<String> {
            let mut path = Vec::new();
            let mut cursor = parent;
            while let Some(id) = cursor {
                let node = nodes.get(&id)?;
                path.push((id.clone(), node.0.clone(), node.1, node.2.clone()));
                cursor = node.0.clone();
            }
            path.reverse();
            for (id, parent_id, kind, title) in &path {
                if inserted_groups.insert(id.clone()) {
                    ir.push_group(story_document::CompileGroup {
                        id: id.clone(),
                        parent_id: parent_id.clone(),
                        kind: *kind,
                        title: title.clone(),
                    });
                }
            }
            path.last().map(|(id, _, _, _)| id.clone())
        }
        for item in self.store.list_assembly_items(assembly)? {
            if !item.included {
                continue;
            }
            let (kind, title, language_code, source_id, text, group_parent) = if let Some(scene) =
                item.scene_id
            {
                let row = self.store.get_scene(scene)?;
                let (text, _) = self.store.load_text(scene)?;
                let language_code = Some(row.scene.effective_language_code(&project).to_owned());
                (
                    story_document::ContentUnitKind::Scene,
                    row.scene.title,
                    language_code,
                    scene.to_string(),
                    text,
                    row.chapter_id.map(|id| id.to_string()),
                )
            } else if let Some(content) = item.content_unit_id {
                let row = self.store.get_content_unit(content)?;
                let (text, _) = self.store.load_content_unit_text(content)?;
                let kind = match row.kind {
                    ContentUnitKind::FrontMatter => story_document::ContentUnitKind::FrontMatter,
                    ContentUnitKind::SectionBody => story_document::ContentUnitKind::SectionBody,
                    ContentUnitKind::ChapterBody => story_document::ContentUnitKind::ChapterBody,
                    ContentUnitKind::BackMatter => story_document::ContentUnitKind::BackMatter,
                };
                let language_code = Some(row.effective_language_code(&project).to_owned());
                (
                    kind,
                    row.title,
                    language_code,
                    content.to_string(),
                    text,
                    row.parent_id.map(|id| id.to_string()),
                )
            } else {
                return Err(AppError::Domain(
                    "assembly item has no source entity".into(),
                ));
            };
            let group_parent = add_ancestors(&mut ir, &nodes, &mut inserted_groups, group_parent);
            ir.push(story_document::CompileUnit {
                id: item.id.to_string(),
                parent_id: group_parent,
                kind,
                title,
                language_code,
                segments: vec![story_document::CompileSegment::new(source_id, text)],
            });
        }
        if !ir.validate() {
            return Err(AppError::Domain(
                "assembly produced invalid Compile IR".into(),
            ));
        }
        Ok(ir)
    }

    pub fn compile_assembly_markdown(&self, assembly: AssemblyId) -> Result<String> {
        let ir = self.compile_assembly_ir(assembly)?;
        let title = ir.title.clone();
        let language = ir.language_code.clone();
        let (content, _) = ir.render_markdown();
        if let Some(language) = language {
            let metadata = format!("---\nlang: {language}\n---\n\n");
            let body = content.strip_prefix(&metadata).unwrap_or(&content);
            Ok(format!("{metadata}# {title}\n\n{body}"))
        } else {
            Ok(format!("# {title}\n\n{content}"))
        }
    }
    pub fn list_scene_comments(
        &self,
        scene: SceneId,
    ) -> Result<Vec<story_storage::PersonalCommentRecord>> {
        Ok(self.store.list_scene_comments(scene)?)
    }
    pub fn set_personal_comment_state(
        &mut self,
        comment: PersonalCommentId,
        state: &str,
    ) -> Result<()> {
        Ok(self.store.set_personal_comment_state(comment, state)?)
    }
    pub fn create_scratchpad_note(&mut self, title: &str, body: &str) -> Result<ScratchpadId> {
        Ok(self.store.create_scratchpad_note(title, body)?)
    }
    pub fn list_scratchpad_notes(&self, include_archived: bool) -> Result<Vec<ScratchpadNote>> {
        Ok(self.store.list_scratchpad_notes(include_archived)?)
    }
    pub fn update_scratchpad_note(
        &mut self,
        id: ScratchpadId,
        title: &str,
        body: &str,
        pinned: bool,
    ) -> Result<()> {
        Ok(self.store.update_scratchpad_note(id, title, body, pinned)?)
    }
    pub fn promote_scratchpad_note(
        &mut self,
        id: ScratchpadId,
        destination_kind: &str,
        destination_id: &str,
    ) -> Result<()> {
        Ok(self
            .store
            .promote_scratchpad_note(id, destination_kind, destination_id)?)
    }
    pub fn load_scene_text(&self, id: SceneId) -> Result<(String, String)> {
        Ok(self.store.load_text(id)?)
    }
    pub fn validate_project(&self) -> Result<()> {
        Ok(self.store.validate()?)
    }
    pub fn recover_pending_writes(&mut self) -> Result<usize> {
        Ok(self.store.recover_pending_writes()?.len())
    }
    pub fn pending_conflicts(&self) -> Result<u32> {
        Ok(self.store.pending_conflicts()?)
    }
    pub fn list_recovery_conflicts(&self) -> Result<Vec<story_storage::RecoveryConflict>> {
        Ok(self.store.list_recovery_conflicts()?)
    }
    pub fn list_content_unit_recovery_conflicts(
        &self,
    ) -> Result<Vec<story_storage::ContentUnitRecoveryConflict>> {
        Ok(self.store.list_content_unit_recovery_conflicts()?)
    }
    pub fn backup_project(&mut self, destination: impl AsRef<Path>) -> Result<()> {
        Ok(self.store.backup_to(destination)?)
    }
    pub fn backup_project_archive(&mut self, destination: impl AsRef<Path>) -> Result<()> {
        Ok(self.store.backup_archive_to(destination)?)
    }
    pub fn restore_project_archive(
        archive: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<()> {
        Ok(ProjectStore::restore_archive_to(archive, destination)?)
    }
    pub fn restore_git_snapshot(
        project: impl AsRef<Path>,
        commit: &str,
        destination: impl AsRef<Path>,
    ) -> Result<Vec<std::path::PathBuf>> {
        let destination = destination.as_ref();
        if destination.exists() {
            return Err(AppError::Domain(
                "Git restore destination already exists".into(),
            ));
        }
        let stage =
            destination.with_extension(format!("writers-ide-stage-{}", uuid::Uuid::new_v4()));
        let result = (|| {
            let files = story_git::GitHistory::open_existing(project)?
                .restore_files_to_new_directory(commit, &stage)?;
            ProjectStore::restore_metadata_snapshot(&stage)?;
            std::fs::rename(&stage, destination).map_err(story_storage::StorageError::from)?;
            if let Some(parent) = destination.parent() {
                std::fs::File::open(parent)
                    .and_then(|directory| directory.sync_all())
                    .map_err(story_storage::StorageError::from)?;
            }
            Ok(files)
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&stage);
        }
        result
    }

    /// Builds an isolated, validated restore candidate and reports the tracked
    /// file changes without mutating the current project or Git history.
    pub fn preview_git_restore(&self, commit: &str) -> Result<story_git::GitRestorePreview> {
        let history = story_git::GitHistory::open_existing(self.store.project_root())?;
        let preview = history.restore_preview(commit)?;
        let temporary = tempfile::tempdir().map_err(|error| {
            AppError::Domain(format!("create Git restore preview workspace: {error}"))
        })?;
        let restored_path = temporary.path().join("restored-project");
        Self::restore_git_snapshot(
            self.store.project_root(),
            &preview.commit_id,
            &restored_path,
        )?;
        let restored = Self::open_project(&restored_path)?;
        if restored.get_project()?.id != self.get_project()?.id {
            return Err(AppError::Domain(
                "selected Git snapshot belongs to a different project".into(),
            ));
        }
        Ok(preview)
    }

    /// Restores committed project files in place with an optimistic, recoverable
    /// file-set transaction. A current-state safety snapshot is taken first.
    pub fn restore_git_snapshot_in_place(
        &mut self,
        commit: &str,
    ) -> Result<story_git::GitRestorePreview> {
        let project_id = self.get_project()?.id;
        let preview = self.preview_git_restore(commit)?;
        self.git_snapshot(&format!("Safety snapshot before restore {commit}"))?;
        let history = story_git::GitHistory::open_existing(self.store.project_root())?;
        let temporary = tempfile::tempdir().map_err(|error| {
            AppError::Domain(format!("create Git restore staging workspace: {error}"))
        })?;
        let restored_path = temporary.path().join("restored-project");
        Self::restore_git_snapshot(
            self.store.project_root(),
            &preview.commit_id,
            &restored_path,
        )?;
        let restored = Self::open_project(&restored_path)?;
        if restored.get_project()?.id != project_id {
            return Err(AppError::Domain(
                "selected Git snapshot belongs to a different project".into(),
            ));
        }
        let historical_paths = history.tracked_paths(&preview.commit_id)?;
        let current_head = history
            .history(1)?
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Domain("project has no Git history to restore".into()))?;
        let current_paths = history.tracked_paths(&current_head.id)?;
        let mut paths = std::collections::BTreeSet::new();
        paths.extend(historical_paths.iter().cloned());
        paths.extend(current_paths);
        let mut changes = Vec::new();
        for relative in paths {
            let contents = if historical_paths.binary_search(&relative).is_ok() {
                Some(
                    std::fs::read(restored_path.join(&relative)).map_err(|error| {
                        AppError::Domain(format!(
                            "read staged Git restore file {relative:?}: {error}"
                        ))
                    })?,
                )
            } else {
                None
            };
            changes.push(story_storage::GitRestoreFile {
                relative_path: relative,
                contents,
            });
        }
        changes.push(story_storage::GitRestoreFile {
            relative_path: "story.db".into(),
            contents: Some(
                std::fs::read(restored_path.join("story.db")).map_err(|error| {
                    AppError::Domain(format!("read staged historical SQLite database: {error}"))
                })?,
            ),
        });
        self.store.restore_git_files(&changes)?;
        self.validate_project()?;
        if self.get_project()?.id != project_id {
            return Err(AppError::Domain(
                "restored project identity changed unexpectedly".into(),
            ));
        }
        Ok(preview)
    }

    /// Recreate an archived or deleted scene from a committed project
    /// snapshot. A fresh ID prevents stale links or ID collisions from being
    /// silently reattached to the restored document.
    pub fn restore_deleted_scene_as_new(
        &mut self,
        commit: &str,
        historical_scene: SceneId,
        destination_chapter: NodeId,
        position: u32,
    ) -> Result<SceneId> {
        if self
            .store
            .list_scenes(false)?
            .iter()
            .any(|record| record.scene.id == historical_scene)
        {
            return Err(AppError::Domain("an active scene with this identity still exists; use the existing scene restore workflow".into()));
        }
        if !self
            .store
            .list_structural_nodes()?
            .iter()
            .any(|node| node.id == destination_chapter && node.kind == "chapter")
        {
            return Err(AppError::Domain(
                "deleted-scene restore destination must be an existing chapter".into(),
            ));
        }
        let temporary = tempfile::tempdir().map_err(|error| {
            AppError::Domain(format!("create historical restore workspace: {error}"))
        })?;
        let historical_path = temporary.path().join("historical-project");
        Self::restore_git_snapshot(self.store.project_root(), commit, &historical_path)?;
        let historical = Self::open_project(&historical_path)?;
        let source = historical.get_scene(historical_scene)?;
        let (text, _) = historical.load_scene_text(historical_scene)?;
        self.git_snapshot(&format!(
            "Before recovering deleted scene {}",
            source.scene.title
        ))?;
        let restored = self.create_scene(destination_chapter, &source.scene.title, position)?;
        let mut metadata = source.scene;
        metadata.id = restored;
        metadata.archived = false;
        if metadata
            .pov_character
            .is_some_and(|id| self.get_character(id).is_err())
        {
            metadata.pov_character = None;
        }
        if metadata
            .start_location
            .is_some_and(|id| self.get_location(id).is_err())
        {
            metadata.start_location = None;
        }
        if metadata
            .end_location
            .is_some_and(|id| self.get_location(id).is_err())
        {
            metadata.end_location = None;
        }
        let result = (|| {
            self.update_scene_metadata(&metadata)?;
            let (_, expected) = self.load_scene_text(restored)?;
            self.save_scene_text(restored, Some(&expected), &text)?;
            Ok::<(), AppError>(())
        })();
        if let Err(error) = result {
            let _ = self.archive_scene(restored);
            return Err(error);
        }
        Ok(restored)
    }

    /// Reactivates an archived scene under its original UUID when the historical
    /// chapter still exists. Prose is restored through the normal durable write
    /// journal and current state is snapshotted first.
    pub fn restore_archived_scene_from_git(
        &mut self,
        commit: &str,
        scene_id: SceneId,
    ) -> Result<()> {
        let current = self.get_scene(scene_id)?;
        if !current.scene.archived {
            return Err(AppError::Domain(
                "selected scene is not archived; use the scene revision restore workflow".into(),
            ));
        }
        let project_id = self.get_project()?.id;
        let temporary = tempfile::tempdir().map_err(|error| {
            AppError::Domain(format!("create historical restore workspace: {error}"))
        })?;
        let historical_path = temporary.path().join("historical-project");
        Self::restore_git_snapshot(self.store.project_root(), commit, &historical_path)?;
        let historical = Self::open_project(&historical_path)?;
        if historical.get_project()?.id != project_id {
            return Err(AppError::Domain(
                "historical snapshot belongs to a different project".into(),
            ));
        }
        let source = historical.get_scene(scene_id)?;
        if source.scene.archived {
            return Err(AppError::Domain(
                "selected snapshot contains the scene only in archived state".into(),
            ));
        }
        let chapter = source.chapter_id.ok_or_else(|| {
            AppError::Domain("historical scene has no chapter placement to restore".into())
        })?;
        let position = source.position.ok_or_else(|| {
            AppError::Domain("historical scene has no manuscript position to restore".into())
        })?;
        if !self
            .list_structural_nodes()?
            .iter()
            .any(|node| node.id == chapter && node.kind == "chapter")
        {
            return Err(AppError::Domain(
                "historical chapter no longer exists; use Recover Deleted Scene as New and choose a chapter".into(),
            ));
        }
        let (text, _) = historical.load_scene_text(scene_id)?;
        let (current_text, current_hash) = self.load_scene_text(scene_id)?;
        self.git_snapshot(&format!(
            "Before restoring archived scene {}",
            source.scene.title
        ))?;
        let restored_hash = self.save_scene_text(scene_id, Some(&current_hash), &text)?;
        let result = (|| {
            self.move_scene(scene_id, chapter, position)?;
            let mut metadata = source.scene;
            metadata.archived = false;
            self.update_scene_metadata(&metadata)?;
            Ok::<(), AppError>(())
        })();
        if let Err(error) = result {
            if current_text != text
                && let Err(rollback) =
                    self.save_scene_text(scene_id, Some(&restored_hash), &current_text)
            {
                return Err(AppError::Domain(format!(
                    "scene restore failed ({error}); prose rollback also needs attention ({rollback})"
                )));
            }
            if let (Some(old_chapter), Some(old_position)) = (current.chapter_id, current.position)
                && let Err(rollback) = self.move_scene(scene_id, old_chapter, old_position)
            {
                return Err(AppError::Domain(format!(
                    "scene restore failed ({error}); placement rollback also needs attention ({rollback})"
                )));
            }
            if let Err(rollback) = self.update_scene_metadata(&current.scene) {
                return Err(AppError::Domain(format!(
                    "scene restore failed ({error}); metadata rollback also needs attention ({rollback})"
                )));
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn git_snapshot(&mut self, label: &str) -> Result<Option<String>> {
        let author = self.get_project()?.author;
        Ok(self.store.with_git_snapshot_barrier(|root| {
            story_git::GitHistory::open(root)
                .and_then(|git| git.snapshot(label, &author))
                .map_err(|e| e.to_string())
        })?)
    }

    pub fn add_git_remote(&self, name: &str, url: &str) -> Result<()> {
        story_git::GitHistory::open_existing(self.store.project_root())?.add_remote(name, url)?;
        Ok(())
    }

    pub fn git_remotes(&self) -> Result<Vec<story_git::GitRemote>> {
        Ok(story_git::GitHistory::open_existing(self.store.project_root())?.remotes()?)
    }

    /// Snapshot current canonical files, then push the selected branch using
    /// Git's normal non-forced push semantics. Existing credentials are
    /// delegated to Git; credentials in remote URLs are rejected by story-git.
    pub fn back_up_git_branch(&mut self, remote: &str, branch: &str) -> Result<String> {
        self.git_snapshot("Before explicit remote backup")?;
        let history = story_git::GitHistory::open_existing(self.store.project_root())?;
        if history.current_branch()? != branch {
            return Err(AppError::Domain(format!(
                "current branch is not {branch}; refusing to push a different branch"
            )));
        }
        let commit = history.head_commit()?;
        history.push_branch(remote, branch)?;
        Ok(commit)
    }

    /// Fetch and apply a remote branch only when it is a fast-forward of the
    /// current branch. Divergent histories are left fetched but unapplied for
    /// explicit conflict inspection. The local safety snapshot is committed
    /// before contacting the remote.
    pub fn retrieve_git_branch(&mut self, remote: &str, branch: &str) -> Result<String> {
        self.git_snapshot("Safety snapshot before remote retrieval")?;
        let history = story_git::GitHistory::open_existing(self.store.project_root())?;
        let current_branch = history.current_branch()?;
        if current_branch != branch {
            return Err(AppError::Domain(format!(
                "current branch is {current_branch}; switch to {branch} before retrieving"
            )));
        }
        let current = history.head_commit()?;
        let fetched = history.fetch_branch(remote, branch)?;
        if fetched == current {
            return Ok(fetched);
        }
        if !history.is_ancestor(&current, &fetched)? {
            return Err(AppError::Domain(format!(
                "remote branch {remote}/{branch} diverges from local history; it was fetched but not applied"
            )));
        }

        self.restore_git_snapshot_in_place(&fetched)?;
        history.fast_forward_ref_to(&fetched, &current)?;
        Ok(fetched)
    }

    pub fn set_git_auto_snapshot_interval(&self, minutes: Option<u32>) -> Result<()> {
        let mut policy = story_git::load_auto_snapshot_policy(self.store.project_root())?;
        policy.set_interval(minutes)?;
        story_git::save_auto_snapshot_policy(self.store.project_root(), &policy)?;
        Ok(())
    }
    pub fn set_git_snapshot_on_exit(&self, enabled: bool) -> Result<()> {
        let mut policy = story_git::load_auto_snapshot_policy(self.store.project_root())?;
        policy.snapshot_on_exit = enabled;
        story_git::save_auto_snapshot_policy(self.store.project_root(), &policy)?;
        Ok(())
    }
    /// Create an interval snapshot when due. `now_unix` is injectable for deterministic tests.
    pub fn git_auto_snapshot_if_due(&mut self, now_unix: i64) -> Result<Option<String>> {
        let mut policy = story_git::load_auto_snapshot_policy(self.store.project_root())?;
        if !policy.due(now_unix) {
            return Ok(None);
        }
        let result = self.git_snapshot("Automatic project snapshot")?;
        policy.record_check(now_unix);
        story_git::save_auto_snapshot_policy(self.store.project_root(), &policy)?;
        Ok(result)
    }
    /// Snapshot persisted project state on orderly application exit when enabled.
    pub fn git_auto_snapshot_on_exit(&mut self) -> Result<Option<String>> {
        let policy = story_git::load_auto_snapshot_policy(self.store.project_root())?;
        if !policy.snapshot_on_exit {
            return Ok(None);
        }
        self.git_snapshot("Automatic project snapshot on exit")
    }
    /// Restore one scene's prose from a local Git commit through the normal
    /// optimistic write journal. Current project state is snapshotted first.
    pub fn restore_scene_from_git(&mut self, commit: &str, scene: SceneId) -> Result<String> {
        let relative_path = self.store.scene_content_relative_path(scene)?;
        let relative_path = relative_path
            .to_str()
            .ok_or_else(|| AppError::Domain("scene manuscript path is not UTF-8".into()))?;
        let historical = story_git::GitHistory::open_existing(self.store.project_root())?
            .read_file_at(commit, relative_path)?;
        let historical = String::from_utf8(historical)
            .map_err(|_| AppError::Domain("historical scene text is not UTF-8".into()))?;
        let (current, expected_hash) = self.load_scene_text(scene)?;
        if current == historical {
            return Ok(expected_hash);
        }
        let title = self.get_scene(scene)?.scene.title;
        self.git_snapshot(&format!("Before restoring scene {title} from {commit}"))?;
        self.save_scene_text(scene, Some(&expected_hash), &historical)
    }
    /// Restore one chapter/front/section content unit from a local Git commit
    /// through its optimistic write journal after snapshotting current state.
    pub fn restore_content_unit_from_git(
        &mut self,
        commit: &str,
        content_unit: ContentUnitId,
    ) -> Result<String> {
        let relative_path = self
            .store
            .content_unit_content_relative_path(content_unit)?;
        let relative_path = relative_path
            .to_str()
            .ok_or_else(|| AppError::Domain("content-unit path is not UTF-8".into()))?;
        let historical = story_git::GitHistory::open_existing(self.store.project_root())?
            .read_file_at(commit, relative_path)?;
        let historical = String::from_utf8(historical)
            .map_err(|_| AppError::Domain("historical content-unit text is not UTF-8".into()))?;
        let (current, expected_hash) = self.load_content_unit_text(content_unit)?;
        if current == historical {
            return Ok(expected_hash);
        }
        let title = self.get_content_unit(content_unit)?.title;
        self.git_snapshot(&format!(
            "Before restoring content unit {title} from {commit}"
        ))?;
        self.save_content_unit_text(content_unit, Some(&expected_hash), &historical)
    }
    pub fn git_history(&self, limit: usize) -> Result<Vec<story_git::GitCommit>> {
        Ok(story_git::GitHistory::open_existing(self.store.project_root())?.history(limit)?)
    }
    pub fn git_diff(&self, from: &str, to: &str, path: Option<&str>) -> Result<String> {
        Ok(
            story_git::GitHistory::open_existing(self.store.project_root())?
                .diff(from, to, path)?,
        )
    }
    pub fn git_search_history(
        &self,
        text: &str,
        path: Option<&str>,
        limit: usize,
    ) -> Result<Vec<story_git::HistoricalTextChange>> {
        Ok(
            story_git::GitHistory::open_existing(self.store.project_root())?
                .search_text_history(text, path, limit)?,
        )
    }
    pub fn git_blame(
        &self,
        path: &str,
        first_line: usize,
        last_line: usize,
    ) -> Result<Vec<story_git::BlamedLine>> {
        Ok(
            story_git::GitHistory::open_existing(self.store.project_root())?
                .blame(path, first_line, last_line)?,
        )
    }
    pub fn resolve_conflict(&mut self, txid: &str, resolution: ConflictResolution) -> Result<()> {
        Ok(self.store.resolve_write_conflict(txid, resolution)?)
    }
    pub fn resolve_content_unit_conflict(&mut self, txid: &str, use_prepared: bool) -> Result<()> {
        Ok(self
            .store
            .resolve_content_unit_conflict(txid, use_prepared)?)
    }
}

#[allow(dead_code)]
fn _storage_result(_: StorageResult<()>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn chronology_is_disabled_by_default_and_preferences_survive_reopen_and_relocation() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        assert_eq!(
            app.chronology_preferences().unwrap(),
            ChronologyPreferences::default()
        );
        let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
        let scene_id = app.create_scene(chapter, "Opening", 0).unwrap();
        assert!(!app.should_prompt_for_missing_scene_time(scene_id).unwrap());
        let scene_text = app.load_scene_text(scene_id).unwrap();
        let prompt_preferences = ChronologyPreferences {
            enabled: true,
            prompt_for_missing_scene_time: true,
            missing_scene_time_prompt_dismissed: false,
        };
        app.set_chronology_preferences(prompt_preferences).unwrap();
        assert!(app.should_prompt_for_missing_scene_time(scene_id).unwrap());
        app.update_scene_story_time(scene_id, StoryTime::exact(60), StoryTime::exact(120))
            .unwrap();
        assert!(!app.should_prompt_for_missing_scene_time(scene_id).unwrap());
        assert_eq!(app.load_scene_text(scene_id).unwrap(), scene_text);
        drop(app);

        let app = WriterApp::open_project(&project).unwrap();
        assert_eq!(app.chronology_preferences().unwrap(), prompt_preferences);
        drop(app);

        let relocated = dir.path().join("Relocated Novel");
        fs::rename(&project, &relocated).unwrap();
        let reopened = WriterApp::open_project(&relocated).unwrap();
        assert_eq!(
            reopened.chronology_preferences().unwrap(),
            prompt_preferences
        );
    }

    #[test]
    fn emacs_tags_export_points_to_stable_symbol_index_entries_and_keeps_utf8_offsets() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
        let scene = app.create_scene(chapter, "Mára arrives", 0).unwrap();
        let character = app.create_character("Mára Venn").unwrap();
        app.add_character_alias(character, "Mára").unwrap();

        let definitions = app.list_project_symbols().unwrap();
        assert!(definitions.iter().any(|symbol| {
            symbol.entity_kind == "scene"
                && symbol.entity_id == scene.to_string()
                && symbol
                    .source_path
                    .as_ref()
                    .is_some_and(|path| path.starts_with("manuscript"))
        }));
        let export = app.build_emacs_tags_export().unwrap();
        assert!(
            export
                .symbols_markdown
                .contains("## character: Mára Venn [")
        );
        assert!(export.symbols_markdown.contains("## scene: Mára arrives ["));
        assert!(export.tags_file.starts_with("\u{c}\nsymbols.md,"));
        assert!(export.tags_file.contains("\u{7f}Mára\u{1}"));
        assert!(export.tags_file.contains("\u{7f}Mára Venn\u{1}"));

        let record = export
            .tags_file
            .split('\n')
            .find(|line| line.contains("\u{7f}Mára\u{1}"))
            .unwrap();
        let (_, coordinates) = record.split_once('\u{1}').unwrap();
        let (line, offset) = coordinates.split_once(',').unwrap();
        let line: usize = line.parse().unwrap();
        let offset: usize = offset.parse().unwrap();
        assert!(line > 1);
        assert!(export.symbols_markdown.is_char_boundary(offset));
        assert!(export.symbols_markdown[offset..].starts_with("## character: Mára Venn ["));

        let path = app.export_emacs_tags().unwrap();
        assert!(path.ends_with(".writers-ide/tags/TAGS"));
        assert_eq!(fs::read_to_string(path).unwrap(), export.tags_file);
    }

    #[test]
    fn declarative_nonfiction_template_creates_direct_prose_and_round_trips() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("Reference Book");
        let template = BuiltinTemplate::NonfictionBook.definition();
        let mut app =
            WriterApp::create_project_from_definition(&path, "Reference Book", &template).unwrap();
        let project = app.get_project().unwrap();
        assert_eq!(project.book_type, BookType::Nonfiction);
        assert_eq!(project.template_id.as_deref(), Some("nonfiction-book"));
        let chapters = app.list_structural_nodes().unwrap();
        assert_eq!(chapters.len(), 1);
        let bodies = app.list_content_units(false).unwrap();
        assert_eq!(bodies.len(), 1);
        let body = bodies[0].id;
        let (_, hash) = app.load_content_unit_text(body).unwrap();
        app.save_content_unit_text(body, Some(&hash), "Direct chapter prose.")
            .unwrap();
        drop(app);
        let reopened = WriterApp::open_project(&path).unwrap();
        assert_eq!(
            reopened.load_content_unit_text(body).unwrap().0,
            "Direct chapter prose."
        );
        assert!(reopened.list_scenes(false).unwrap().is_empty());
        let compiled = reopened
            .compile_artifact(story_document::CompileFormat::Markdown, false)
            .unwrap();
        let compiled = String::from_utf8(compiled.bytes).unwrap();
        assert!(compiled.contains("Direct chapter prose."));
    }

    #[test]
    fn saving_project_as_template_preserves_structure_but_never_copies_prose() {
        let directory = tempfile::tempdir().unwrap();
        let source_path = directory.path().join("Source");
        let mut source = WriterApp::create_project_with_type(
            &source_path,
            "Private Source Book",
            BookType::Nonfiction,
        )
        .unwrap();
        let part = source.create_section("Part One", 0).unwrap();
        let subsection = source.create_section("Background", 1).unwrap();
        source
            .move_structural_node(subsection, Some(part), 0)
            .unwrap();
        let chapter = source
            .create_chapter(Some(subsection), "Chapter One", 0)
            .unwrap();
        let section_body = source
            .create_content_unit(
                ContentUnitKind::SectionBody,
                "Section introduction",
                Some(subsection),
                0,
                Some(450),
            )
            .unwrap();
        let (_, section_hash) = source.load_content_unit_text(section_body).unwrap();
        source
            .save_content_unit_text(
                section_body,
                Some(&section_hash),
                "UNIQUE PRIVATE SECTION PROSE",
            )
            .unwrap();
        let chapter_body = source
            .create_content_unit(
                ContentUnitKind::ChapterBody,
                "Chapter One prose",
                Some(chapter),
                0,
                Some(1200),
            )
            .unwrap();
        let (_, chapter_hash) = source.load_content_unit_text(chapter_body).unwrap();
        source
            .save_content_unit_text(
                chapter_body,
                Some(&chapter_hash),
                "UNIQUE PRIVATE CHAPTER PROSE",
            )
            .unwrap();
        source
            .create_content_unit(ContentUnitKind::FrontMatter, "Dedication", None, 0, None)
            .unwrap();
        source
            .define_custom_field(&CustomFieldDefinition {
                id: FieldId::new(),
                name: "Review state".into(),
                field_type: FieldType::Enum,
                required: false,
                default_value: Some(FieldValue::Enum("draft".into())),
                enum_values: vec!["draft".into(), "review".into()],
                reference_kind: None,
            })
            .unwrap();

        let template = source
            .project_template_from_current("field-guide", "Field Guide")
            .unwrap();
        let encoded = toml::to_string_pretty(&template).unwrap();
        assert!(!encoded.contains("UNIQUE PRIVATE"));
        assert!(!encoded.contains("Private Source Book"));
        assert_eq!(
            toml::from_str::<ProjectTemplate>(&encoded).unwrap(),
            template
        );
        assert_eq!(template.book_type, BookType::Nonfiction);
        assert_eq!(template.nodes.len(), 3);
        assert_eq!(template.content_units.len(), 3);
        assert_eq!(template.custom_fields.len(), 1);
        assert_eq!(template.custom_fields[0].name, "Review state");
        assert_eq!(
            template.custom_fields[0].default_value,
            Some(FieldValue::Enum("draft".into()))
        );
        assert!(template.scenes.is_empty());
        assert!(template.nodes.iter().any(|node| node.parent_key.is_some()));

        let restored_path = directory.path().join("FromTemplate");
        let restored =
            WriterApp::create_project_from_definition(&restored_path, "New Field Guide", &template)
                .unwrap();
        assert_eq!(restored.list_scenes(false).unwrap().len(), 0);
        assert_eq!(restored.list_structural_nodes().unwrap().len(), 3);
        let restored_units = restored.list_content_units(false).unwrap();
        assert_eq!(restored_units.len(), 3);
        let restored_fields = restored.list_custom_field_definitions().unwrap();
        assert_eq!(restored_fields.len(), 1);
        assert_eq!(restored_fields[0].name, "Review state");
        for unit in restored_units {
            assert!(
                restored
                    .load_content_unit_text(unit.id)
                    .unwrap()
                    .0
                    .is_empty()
            );
        }
    }

    #[test]
    fn custom_template_deserialization_rejects_unmapped_keys() {
        let source = r#"
schema_version = 1
template_revision = 1
id = "custom"
name = "Custom"
book_type = "nonfiction"
unexpected_prose = "must not be silently discarded"
"#;
        assert!(toml::from_str::<ProjectTemplate>(source).is_err());
    }

    #[test]
    fn user_writing_language_is_independent_and_seeds_new_projects() {
        let dir = tempfile::tempdir().unwrap();
        let preferences_path = dir.path().join("config/preferences.toml");
        let preferences = UserPreferences {
            ui_language: "en-GB".into(),
            default_writing_language: "it-IT".into(),
        };
        write_user_preferences_at(&preferences_path, &preferences).unwrap();
        assert_eq!(
            read_user_preferences_at(&preferences_path).unwrap(),
            preferences
        );
        let app = WriterApp::create_project_with_preferences(
            dir.path().join("Novel"),
            "Novel",
            &preferences,
        )
        .unwrap();
        assert_eq!(app.get_project().unwrap().language_code, "it-IT");
        assert_eq!(preferences.ui_language, "en-GB");
    }

    #[test]
    fn invalid_user_language_preferences_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.toml");
        fs::write(
            &path,
            "ui_language = 'en'\ndefault_writing_language = 'not a tag'\n",
        )
        .unwrap();
        assert!(read_user_preferences_at(&path).is_err());
    }

    #[test]
    fn event_dependency_review_uses_uncertain_story_intervals() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Events"), "Events").unwrap();
        let first = EventId::new();
        let second = EventId::new();
        app.add_story_event(&StoryEvent {
            id: first,
            title: "Message sent".into(),
            description: String::new(),
            start: StoryTime::exact(10),
            end: StoryTime::exact(12),
            source_time_text: "day 1".into(),
        })
        .unwrap();
        let mut second_event = StoryEvent {
            id: second,
            title: "Reply received".into(),
            description: String::new(),
            start: StoryTime::new(Some(20), Some(15), Some(25), TimePrecision::Bounded).unwrap(),
            end: StoryTime::unknown(),
            source_time_text: "sometime later".into(),
        };
        app.add_story_event(&second_event).unwrap();
        app.add_event_dependency(&StoryEventDependency {
            predecessor: first,
            successor: second,
            kind: "precedes".into(),
            notes: "Communication order".into(),
        })
        .unwrap();
        let findings = app.inspect_event_dependencies().unwrap();
        assert_eq!(findings[0].1.status, story_analysis::InspectionStatus::Pass);
        second_event.start =
            StoryTime::new(Some(12), Some(11), Some(13), TimePrecision::Bounded).unwrap();
        app.update_story_event(&second_event).unwrap();
        assert_eq!(
            app.inspect_event_dependencies().unwrap()[0].1.status,
            story_analysis::InspectionStatus::Unknown
        );
    }
    #[test]
    fn application_vertical_slice_survives_reopen_and_relocation() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("Novel");
        let mut app = WriterApp::create_project(&p, "Novel").unwrap();
        let first = app.create_chapter(None, "First", 0).unwrap();
        let second = app.create_chapter(None, "Second", 1).unwrap();
        let scene = app.create_scene(first, "Arrival", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "The gate opened. Mara stepped through.")
            .unwrap();
        let mut row = app.get_scene(scene).unwrap().scene;
        row.synopsis = "Arrival and choice".into();
        row.status = SceneStatus::DraftComplete;
        row.target_words = Some(14);
        app.update_scene_metadata(&row).unwrap();
        app.move_scene(scene, second, 0).unwrap();
        drop(app);
        let moved = d.path().join("Relocated");
        fs::rename(&p, &moved).unwrap();
        let app = WriterApp::open_project(&moved).unwrap();
        app.validate_project().unwrap();
        let got = app.get_scene(scene).unwrap();
        assert_eq!(got.chapter_id, Some(second));
        assert_eq!(got.scene.synopsis, "Arrival and choice");
        assert_eq!(got.scene.status, SceneStatus::DraftComplete);
        assert_eq!(
            app.load_scene_text(scene).unwrap().0,
            "The gate opened. Mara stepped through."
        );
        let pipeline = app.processing_input().unwrap();
        assert_eq!(pipeline.scenes[0].id, scene.to_string());
        assert_eq!(
            pipeline.scenes[0].text,
            "The gate opened. Mara stepped through."
        );
        assert!(
            app.run_processing()
                .unwrap()
                .iter()
                .all(|stage| stage.status == story_pipeline::StageStatus::Complete)
        );
        assert!(
            app.run_processing()
                .unwrap()
                .iter()
                .all(|stage| stage.status == story_pipeline::StageStatus::Cached)
        );
    }

    #[test]
    fn nonfiction_chapter_prose_is_a_content_unit_not_a_scene() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Field Guide");
        let mut app = WriterApp::create_project_from_template(
            &path,
            "Field Guide",
            BuiltinTemplate::BlankNonfiction,
        )
        .unwrap();
        let chapter = app.create_chapter(None, "Introduction", 0).unwrap();
        let body = app
            .create_content_unit(
                ContentUnitKind::ChapterBody,
                "Introduction",
                Some(chapter),
                0,
                None,
            )
            .unwrap();
        let (_, hash) = app.load_content_unit_text(body).unwrap();
        app.save_content_unit_text(body, Some(&hash), "This is direct chapter prose.")
            .unwrap();
        assert!(app.list_scenes(false).unwrap().is_empty());
        assert_eq!(
            app.get_project().unwrap().template_id.as_deref(),
            Some("blank-nonfiction")
        );
        let input = app.processing_input().unwrap();
        assert!(input.scenes.is_empty());
        assert_eq!(input.content_units.len(), 1);
        assert!(input.content_units[0].text.contains("direct chapter prose"));
        let (compiled, _) = story_pipeline::compile_ir(&input).render_markdown();
        assert_eq!(
            compiled,
            "---\nlang: en-GB\n---\n\n## Introduction\n\nThis is direct chapter prose.\n\n"
        );
    }

    #[test]
    fn git_snapshots_pair_manuscript_and_metadata_and_skip_unchanged_projects() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "First draft.")
            .unwrap();
        let first = app.git_snapshot("First draft").unwrap().unwrap();
        assert!(app.git_snapshot("No changes").unwrap().is_none());
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Revised draft.")
            .unwrap();
        let second = app.git_snapshot("Revision").unwrap().unwrap();
        assert_ne!(first, second);
        assert_eq!(app.git_history(10).unwrap().len(), 2);
        assert!(
            app.git_diff(&first, &second, Some(&format!("manuscript/{scene}.md")))
                .unwrap()
                .contains("Revised draft")
        );
        assert!(project.join("snapshots/database.sql").exists());
        assert!(app.git_diff(&first, &second, Some("story.db")).is_err());
    }

    #[test]
    fn git_remote_backup_and_fast_forward_retrieval_reconcile_manuscript_and_metadata() {
        use std::process::Command;

        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let bare = dir.path().join("backup.git");
        let remote_work = dir.path().join("RemoteWork");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let (_, expected) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&expected), "Local draft.")
            .unwrap();
        app.git_snapshot("Initial shared version").unwrap();
        let branch = story_git::GitHistory::open_existing(&project)
            .unwrap()
            .current_branch()
            .unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--bare", bare.to_str().unwrap()])
                .output()
                .unwrap()
                .status
                .success()
        );
        app.add_git_remote("backup", bare.to_str().unwrap())
            .unwrap();
        app.back_up_git_branch("backup", &branch).unwrap();
        drop(app);

        let cloned = Command::new("git")
            .args([
                "clone",
                "--quiet",
                "--branch",
                &branch,
                bare.to_str().unwrap(),
                remote_work.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            cloned.status.success(),
            "{}",
            String::from_utf8_lossy(&cloned.stderr)
        );
        ProjectStore::restore_metadata_snapshot(&remote_work).unwrap();
        let mut remote_app = WriterApp::open_project(&remote_work).unwrap();
        let (_, expected) = remote_app.load_scene_text(scene).unwrap();
        remote_app
            .save_scene_text(scene, Some(&expected), "Remote revision.")
            .unwrap();
        remote_app.back_up_git_branch("origin", &branch).unwrap();
        drop(remote_app);

        let mut local_app = WriterApp::open_project(&project).unwrap();
        let retrieved = local_app.retrieve_git_branch("backup", &branch).unwrap();
        assert_eq!(
            local_app.load_scene_text(scene).unwrap().0,
            "Remote revision."
        );
        assert_eq!(local_app.git_history(1).unwrap()[0].id, retrieved);
        assert_eq!(local_app.get_scene(scene).unwrap().scene.title, "Opening");
        local_app.validate_project().unwrap();
    }

    #[test]
    fn git_remote_retrieval_reports_divergence_without_replacing_local_content() {
        use std::process::Command;

        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let bare = dir.path().join("backup.git");
        let remote_work = dir.path().join("RemoteWork");
        let mut local = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = local.create_chapter(None, "Chapter", 0).unwrap();
        let scene = local.create_scene(chapter, "Opening", 0).unwrap();
        let (_, expected) = local.load_scene_text(scene).unwrap();
        local
            .save_scene_text(scene, Some(&expected), "Shared base.")
            .unwrap();
        local.git_snapshot("Shared base").unwrap();
        let branch = story_git::GitHistory::open_existing(&project)
            .unwrap()
            .current_branch()
            .unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--bare", bare.to_str().unwrap()])
                .output()
                .unwrap()
                .status
                .success()
        );
        local
            .add_git_remote("backup", bare.to_str().unwrap())
            .unwrap();
        local.back_up_git_branch("backup", &branch).unwrap();

        let cloned = Command::new("git")
            .args([
                "clone",
                "--quiet",
                "--branch",
                &branch,
                bare.to_str().unwrap(),
                remote_work.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            cloned.status.success(),
            "{}",
            String::from_utf8_lossy(&cloned.stderr)
        );
        ProjectStore::restore_metadata_snapshot(&remote_work).unwrap();
        let mut remote = WriterApp::open_project(&remote_work).unwrap();
        let (_, expected) = remote.load_scene_text(scene).unwrap();
        remote
            .save_scene_text(scene, Some(&expected), "Remote branch.")
            .unwrap();
        remote.back_up_git_branch("origin", &branch).unwrap();
        drop(remote);

        let (_, expected) = local.load_scene_text(scene).unwrap();
        local
            .save_scene_text(scene, Some(&expected), "Local branch.")
            .unwrap();
        local.git_snapshot("Local branch").unwrap();
        let error = local.retrieve_git_branch("backup", &branch).unwrap_err();
        assert!(error.to_string().contains("diverges from local history"));
        assert_eq!(local.load_scene_text(scene).unwrap().0, "Local branch.");
        local.validate_project().unwrap();
    }

    #[test]
    fn automatic_git_snapshots_run_only_when_due_and_skip_unchanged_projects() {
        let root = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(root.path().join("Novel"), "Novel").unwrap();
        app.set_git_auto_snapshot_interval(Some(1)).unwrap();
        assert!(app.git_auto_snapshot_if_due(1_000).unwrap().is_some());
        assert!(app.git_auto_snapshot_if_due(1_059).unwrap().is_none());
        assert!(app.git_auto_snapshot_if_due(1_060).unwrap().is_none());
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Changed prose.")
            .unwrap();
        assert!(app.git_auto_snapshot_if_due(1_119).unwrap().is_none());
        assert!(app.git_auto_snapshot_if_due(1_120).unwrap().is_some());
        app.set_git_auto_snapshot_interval(None).unwrap();
        assert!(app.git_auto_snapshot_if_due(10_000).unwrap().is_none());
    }

    #[test]
    fn exit_snapshots_are_opt_in_and_interval_changes_preserve_the_setting() {
        let root = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(root.path().join("Novel"), "Novel").unwrap();
        assert!(app.git_auto_snapshot_on_exit().unwrap().is_none());
        app.set_git_snapshot_on_exit(true).unwrap();
        assert!(app.git_auto_snapshot_on_exit().unwrap().is_some());
        assert!(app.git_auto_snapshot_on_exit().unwrap().is_none());
        app.set_git_auto_snapshot_interval(Some(30)).unwrap();
        assert!(app.git_auto_snapshot_on_exit().unwrap().is_none());
        app.set_git_snapshot_on_exit(false).unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        app.create_scene(chapter, "Scene", 0).unwrap();
        assert!(app.git_auto_snapshot_on_exit().unwrap().is_none());
    }

    #[test]
    fn git_restores_one_scene_through_journal_and_snapshots_current_prose() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Original version.")
            .unwrap();
        let original = app.git_snapshot("Original prose").unwrap().unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Current revision.")
            .unwrap();

        app.restore_scene_from_git(&original, scene).unwrap();
        assert_eq!(app.load_scene_text(scene).unwrap().0, "Original version.");
        let history = app.git_history(10).unwrap();
        assert_eq!(history.len(), 2);
        assert!(
            history[0]
                .subject
                .contains("Before restoring scene Opening")
        );
        assert!(
            app.git_diff(
                &history[1].id,
                &history[0].id,
                Some(&format!("manuscript/{scene}.md"))
            )
            .unwrap()
            .contains("Current revision.")
        );
        assert!(app.restore_scene_from_git("not-a-commit", scene).is_err());
    }

    #[test]
    fn git_recovers_archived_scene_as_new_identity_with_core_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
        let original = app.create_scene(chapter, "Lost scene", 0).unwrap();
        let mut metadata = app.get_scene(original).unwrap().scene;
        metadata.synopsis = "An important turn".into();
        metadata.purpose = "Reveal the map".into();
        metadata.goal = "Find the route".into();
        metadata.outcome = "The route is found".into();
        app.update_scene_metadata(&metadata).unwrap();
        let (_, expected) = app.load_scene_text(original).unwrap();
        app.save_scene_text(original, Some(&expected), "The map opened.")
            .unwrap();
        let commit = app
            .git_snapshot("Before accidental deletion")
            .unwrap()
            .unwrap();
        app.archive_scene(original).unwrap();

        let restored = app
            .restore_deleted_scene_as_new(&commit, original, chapter, 0)
            .unwrap();
        assert_ne!(restored, original);
        let restored_record = app.get_scene(restored).unwrap();
        assert_eq!(restored_record.scene.title, "Lost scene");
        assert_eq!(restored_record.scene.synopsis, "An important turn");
        assert_eq!(restored_record.scene.purpose, "Reveal the map");
        assert_eq!(restored_record.chapter_id, Some(chapter));
        assert_eq!(app.load_scene_text(restored).unwrap().0, "The map opened.");
        assert!(
            app.list_scenes(false)
                .unwrap()
                .iter()
                .any(|item| item.scene.id == restored)
        );
    }

    #[test]
    fn git_reactivates_archived_scene_with_original_identity_placement_metadata_and_links() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let original_chapter = app.create_chapter(None, "Original Chapter", 0).unwrap();
        let later_chapter = app.create_chapter(None, "Later Chapter", 1).unwrap();
        let scene = app.create_scene(original_chapter, "Lost Scene", 0).unwrap();
        let character = app.create_character("Mara").unwrap();
        app.link_character(scene, character, ParticipationRole::Major)
            .unwrap();
        let plotline = app.create_plotline("The Map").unwrap();
        app.set_scene_plotlines(
            scene,
            &[story_core::ScenePlotline {
                plotline_id: plotline,
                role: "advances".into(),
                share_basis_points: None,
            }],
            WordAttributionPolicy::Overlap,
        )
        .unwrap();
        let event = app.add_event("The map is found", "day 1").unwrap();
        app.link_event(scene, event, "depicts").unwrap();

        let mut metadata = app.get_scene(scene).unwrap().scene;
        metadata.synopsis = "Historical synopsis".into();
        metadata.purpose = "Reveal the map".into();
        metadata.goal = "Find the route".into();
        metadata.outcome = "The route is found".into();
        metadata.target_words = Some(850);
        app.update_scene_metadata(&metadata).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "The original map passage.")
            .unwrap();
        let snapshot = app
            .git_snapshot("Before accidental archive")
            .unwrap()
            .unwrap();

        app.archive_scene(scene).unwrap();
        app.move_scene(scene, later_chapter, 0).unwrap();
        let mut changed = app.get_scene(scene).unwrap().scene;
        changed.synopsis = "Changed after archive".into();
        changed.purpose = "Changed purpose".into();
        app.update_scene_metadata(&changed).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Changed prose after archive.")
            .unwrap();

        app.restore_archived_scene_from_git(&snapshot, scene)
            .unwrap();

        let restored = app.get_scene(scene).unwrap();
        assert_eq!(restored.scene.id, scene);
        assert!(!restored.scene.archived);
        assert_eq!(restored.scene.synopsis, "Historical synopsis");
        assert_eq!(restored.scene.purpose, "Reveal the map");
        assert_eq!(restored.scene.goal, "Find the route");
        assert_eq!(restored.scene.outcome, "The route is found");
        assert_eq!(restored.scene.target_words, Some(850));
        assert_eq!(restored.chapter_id, Some(original_chapter));
        assert_eq!(restored.position, Some(0));
        assert_eq!(
            app.load_scene_text(scene).unwrap().0,
            "The original map passage."
        );
        assert_eq!(
            app.store.scene_character_analytics(scene).unwrap(),
            vec![(character.to_string(), "Mara".into(), "major".into())]
        );
        assert_eq!(app.scene_plotline_links(scene).unwrap().len(), 1);
        assert_eq!(
            app.scene_plotline_links(scene).unwrap()[0].plotline_id,
            plotline
        );
        assert_eq!(
            app.scene_event_links(scene).unwrap(),
            vec![(event, "depicts".into())]
        );
        app.validate_project().unwrap();
        let history = app.git_history(10).unwrap();
        assert!(
            history[0]
                .subject
                .contains("Before restoring archived scene")
        );
        drop(app);
        let reopened = WriterApp::open_project(project).unwrap();
        assert_eq!(
            reopened.get_scene(scene).unwrap().chapter_id,
            Some(original_chapter)
        );
        assert_eq!(
            reopened.load_scene_text(scene).unwrap().0,
            "The original map passage."
        );
    }

    #[test]
    fn git_restores_one_nonfiction_content_unit_through_its_journal() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Book");
        let mut app =
            WriterApp::create_project_with_type(&project, "Field Guide", BookType::Nonfiction)
                .unwrap();
        let chapter = app.create_chapter(None, "Introduction", 0).unwrap();
        let body = app
            .create_content_unit(
                ContentUnitKind::ChapterBody,
                "Introduction prose",
                Some(chapter),
                0,
                None,
            )
            .unwrap();
        let (_, hash) = app.load_content_unit_text(body).unwrap();
        app.save_content_unit_text(body, Some(&hash), "Original chapter text.")
            .unwrap();
        let original = app.git_snapshot("Original chapter body").unwrap().unwrap();
        let (_, hash) = app.load_content_unit_text(body).unwrap();
        app.save_content_unit_text(body, Some(&hash), "Current chapter revision.")
            .unwrap();

        app.restore_content_unit_from_git(&original, body).unwrap();
        assert_eq!(
            app.load_content_unit_text(body).unwrap().0,
            "Original chapter text."
        );
        let history = app.git_history(10).unwrap();
        assert_eq!(history.len(), 2);
        assert!(history[0].subject.contains("Before restoring content unit"));
        assert!(
            app.restore_content_unit_from_git("not-a-commit", body)
                .is_err()
        );
    }

    #[test]
    fn git_snapshot_restores_complete_project_to_new_relocatable_directory() {
        let root = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(root.path().join("Original"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "A restored passage.")
            .unwrap();
        let commit = app.git_snapshot("Initial complete state").unwrap().unwrap();

        let restored_path = root.path().join("Restored");
        let files =
            WriterApp::restore_git_snapshot(app.store.project_root(), &commit, &restored_path)
                .unwrap();
        assert!(files.iter().any(|path| path == Path::new("project.toml")));
        let restored = WriterApp::open_project(&restored_path).unwrap();
        assert_eq!(restored.get_project().unwrap().title, "Novel");
        assert_eq!(
            restored.load_scene_text(scene).unwrap().0,
            "A restored passage."
        );

        let duplicate =
            WriterApp::restore_git_snapshot(app.store.project_root(), &commit, &restored_path);
        assert!(duplicate.is_err());
        assert_eq!(
            restored.load_scene_text(scene).unwrap().0,
            "A restored passage."
        );
    }

    #[test]
    fn git_restore_preview_is_read_only_and_in_place_restore_recovers_project_state() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let mut metadata = app.get_scene(scene).unwrap().scene;
        metadata.synopsis = "Historical synopsis".into();
        metadata.purpose = "Establish the mystery".into();
        app.update_scene_metadata(&metadata).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "First committed prose.")
            .unwrap();
        let first = app.git_snapshot("First draft").unwrap().unwrap();

        let second_scene = app.create_scene(chapter, "Later Scene", 1).unwrap();
        let (_, hash) = app.load_scene_text(second_scene).unwrap();
        app.save_scene_text(second_scene, Some(&hash), "Second scene prose.")
            .unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Changed prose.")
            .unwrap();
        app.git_snapshot("Changed draft").unwrap().unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Unsnapshotted current prose.")
            .unwrap();
        std::fs::write(project.join(".gitignore"), "author-specific ignore rules\n").unwrap();

        let preview = app.preview_git_restore(&first).unwrap();
        assert!(
            preview
                .changes
                .iter()
                .any(|change| { change.status == "removed" && change.path.ends_with(".md") })
        );
        assert_eq!(app.list_scenes(false).unwrap().len(), 2);
        assert_eq!(
            app.load_scene_text(scene).unwrap().0,
            "Unsnapshotted current prose."
        );
        assert!(preview.working_tree_changes.iter().any(|change| {
            change.status == "changed" && change.path == format!("manuscript/{scene}.md")
        }));
        assert_eq!(
            std::fs::read_to_string(project.join(".gitignore")).unwrap(),
            "author-specific ignore rules\n"
        );

        let applied = app.restore_git_snapshot_in_place(&first).unwrap();
        assert_eq!(applied.commit_id, first);
        let active = app.list_scenes(false).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].scene.id, scene);
        assert_eq!(active[0].scene.synopsis, "Historical synopsis");
        assert_eq!(active[0].scene.purpose, "Establish the mystery");
        assert_eq!(active[0].chapter_id, Some(chapter));
        assert_eq!(
            app.load_scene_text(scene).unwrap().0,
            "First committed prose."
        );
        assert!(
            app.git_history(10)
                .unwrap()
                .iter()
                .any(|commit| commit.subject.contains("Safety snapshot before restore"))
        );
        app.validate_project().unwrap();
        drop(app);
        let reopened = WriterApp::open_project(&project).unwrap();
        assert_eq!(reopened.list_scenes(false).unwrap().len(), 1);
        assert_eq!(
            reopened.load_scene_text(scene).unwrap().0,
            "First committed prose."
        );
        reopened.validate_project().unwrap();
    }

    #[test]
    fn compile_artifact_exposes_portable_markdown_and_source_map() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "A Novel").unwrap();
        app.set_project_language("it-IT").unwrap();
        let chapter = app.create_chapter(None, "Opening", 0).unwrap();
        let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
        app.set_scene_language(scene, Some("fr-FR")).unwrap();
        let chapter_prose = app
            .create_content_unit(
                ContentUnitKind::ChapterBody,
                "Chapter prose",
                Some(chapter),
                0,
                None,
            )
            .unwrap();
        app.set_content_unit_language(chapter_prose, Some("ko-KR"))
            .unwrap();
        let (_, prose_hash) = app.load_content_unit_text(chapter_prose).unwrap();
        app.save_content_unit_text(chapter_prose, Some(&prose_hash), "한국어")
            .unwrap();
        let (_, expected) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&expected), "Élodie arrives.")
            .unwrap();
        app.tag_passage_language(scene, 8, 15, "it-IT").unwrap();
        let artifact = app
            .compile_artifact(story_document::CompileFormat::Markdown, false)
            .unwrap();
        let markdown = String::from_utf8(artifact.bytes).unwrap();
        assert!(markdown.starts_with("---\nlang: it-IT\n---"));
        assert!(markdown.contains("::: {lang=fr-FR}"));
        assert!(markdown.contains("::: {lang=ko-KR}"));
        assert!(markdown.contains("## Opening"));
        assert!(markdown.contains("### Arrival"));
        assert!(markdown.contains("Élodie [arrives]"));
        assert!(markdown.contains("[arrives]{lang=it-IT}"));
        assert_eq!(artifact.source_map.len(), 2);
        assert!(
            artifact
                .source_map
                .iter()
                .any(|entry| entry.source_entity_id == scene.to_string())
        );
        let plain = app
            .compile_artifact(story_document::CompileFormat::PlainText, false)
            .unwrap();
        let plain = String::from_utf8(plain.bytes).unwrap();
        assert!(plain.contains("Élodie arrives."));
        assert!(!plain.contains("{lang=it-IT}"));
    }

    #[test]
    fn excluded_scenes_remain_editable_but_are_absent_from_default_compile() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(directory.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let included = app.create_scene(chapter, "Included", 0).unwrap();
        let excluded = app.create_scene(chapter, "Set Aside", 1).unwrap();
        for (scene, text) in [
            (included, "This stays in the book."),
            (excluded, "This draft is set aside."),
        ] {
            let (_, hash) = app.load_scene_text(scene).unwrap();
            app.save_scene_text(scene, Some(&hash), text).unwrap();
        }

        app.set_scene_included(excluded, false).unwrap();
        assert!(!app.scene_is_included(excluded).unwrap());
        assert_eq!(
            app.load_scene_text(excluded).unwrap().0,
            "This draft is set aside."
        );
        let compiled = app
            .compile_artifact(story_document::CompileFormat::Markdown, false)
            .unwrap();
        let compiled = String::from_utf8(compiled.bytes).unwrap();
        assert!(compiled.contains("This stays in the book."));
        assert!(!compiled.contains("This draft is set aside."));

        app.set_scene_included(excluded, true).unwrap();
        let compiled = app
            .compile_artifact(story_document::CompileFormat::Markdown, false)
            .unwrap();
        assert!(
            String::from_utf8(compiled.bytes)
                .unwrap()
                .contains("This draft is set aside.")
        );
    }

    #[test]
    fn unplaced_scene_is_persistent_but_omitted_until_explicitly_placed() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let scene = app.create_unplaced_scene("Unplaced idea").unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Drafted before choosing a chapter.")
            .unwrap();
        assert_eq!(app.get_scene(scene).unwrap().chapter_id, None);

        let compiled = app
            .compile_artifact(story_document::CompileFormat::Markdown, false)
            .unwrap();
        assert!(
            !String::from_utf8(compiled.bytes)
                .unwrap()
                .contains("Drafted before choosing a chapter.")
        );

        let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
        app.move_scene(scene, chapter, 0).unwrap();
        let compiled = app
            .compile_artifact(story_document::CompileFormat::Markdown, false)
            .unwrap();
        assert!(
            String::from_utf8(compiled.bytes)
                .unwrap()
                .contains("Drafted before choosing a chapter.")
        );
        drop(app);

        let reopened = WriterApp::open_project(project).unwrap();
        let record = reopened.get_scene(scene).unwrap();
        assert_eq!(record.scene.title, "Unplaced idea");
        assert_eq!(record.chapter_id, Some(chapter));
        assert_eq!(
            reopened.load_scene_text(scene).unwrap().0,
            "Drafted before choosing a chapter."
        );
    }

    #[test]
    fn failed_one_off_renderer_attempt_is_recorded_without_an_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let app = WriterApp::create_project(dir.path().join("Novel"), "A Novel").unwrap();
        let result = app.render_artifact_recording_failure(
            "One-off Compile",
            story_document::CompileFormat::Epub,
            || Err(AppError::Domain("synthetic renderer failure".into())),
        );
        assert!(
            matches!(result, Err(AppError::Domain(message)) if message == "synthetic renderer failure")
        );

        let runs = app.list_compile_results().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].configuration, "One-off Compile");
        assert_eq!(runs[0].format.as_deref(), Some("EPUB"));
        assert!(!runs[0].succeeded);
        assert!(runs[0].outputs.is_empty());
        assert!(uuid::Uuid::parse_str(&runs[0].run_id).is_ok());
        assert_eq!(
            runs[0].error.as_deref(),
            Some("invalid domain input: synthetic renderer failure")
        );
        assert!(runs[0].source_fingerprint.is_some());
    }

    #[test]
    fn named_compile_configuration_tracks_only_managed_outputs_and_preserves_sources() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Compile project");
        let mut app = WriterApp::create_project(&project, "Compile project").unwrap();
        let chapter = app.create_chapter(None, "Opening", 0).unwrap();
        let scene = app.create_scene(chapter, "First scene", 0).unwrap();
        let (_, source_hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(
            scene,
            Some(&source_hash),
            "Canonical prose remains here. TODO",
        )
        .unwrap();
        let (_, expected_hash) = app.load_scene_text(scene).unwrap();
        let recipe = story_document::EditionRecipe {
            version: 1,
            name: "Plain Manuscript".into(),
            format: story_document::CompileFormat::Markdown,
            assembly_id: None,
            output: "exports/compile/plain/manuscript.md".into(),
            metadata_title: None,
            metadata_author: None,
            cover_image: None,
            design: story_document::BookDesign::Manuscript,
            expand_transclusions: false,
            spellcheck: false,
            language: None,
            dictionary: None,
            spelling_engine: story_document::SpellcheckEngine::default(),
            diagnostics: story_document::DiagnosticPolicy::default(),
        };
        app.create_compile_configuration("plain", &recipe).unwrap();
        assert!(app.create_compile_configuration("plain", &recipe).is_err());
        assert_eq!(app.list_compile_configurations().unwrap(), vec!["plain"]);

        let output = project.join(&recipe.output);
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(output.parent().unwrap().join("author-note.txt"), "keep me").unwrap();
        fs::write(&output, "unmanaged content").unwrap();
        assert!(app.compile_configuration("plain", false).is_err());
        assert_eq!(fs::read_to_string(&output).unwrap(), "unmanaged content");
        let failed = app.list_compile_results().unwrap();
        assert_eq!(failed.len(), 1);
        assert!(!failed[0].succeeded);
        assert_eq!(failed[0].format.as_deref(), Some("MD"));
        assert!(
            failed[0]
                .error
                .as_deref()
                .is_some_and(|message| message.contains("not managed"))
        );

        fs::remove_file(&output).unwrap();
        let result = app.compile_configuration("plain", false).unwrap();
        assert_eq!(result.configuration, "plain");
        assert!(result.outputs.contains(&recipe.output));
        assert!(output.exists());
        assert!(Path::new(&format!("{}.source-map.json", output.display())).exists());
        assert!(app.compile_configuration("plain", false).is_ok());
        assert!(app.list_compile_results().unwrap().iter().any(|record| {
            record.succeeded
                && record.outputs.contains(&recipe.output)
                && record.version == 6
                && record.format.as_deref() == Some("MD")
                && record.diagnostics.iter().any(|item| {
                    item.code == "PUB004"
                        && item.source_entity_id.as_deref() == Some(&scene.to_string())
                })
        }));
        assert!(
            app.list_compile_results()
                .unwrap()
                .iter()
                .all(|record| !record.stale)
        );
        let (compiled_text, compiled_hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&compiled_hash), "Changed after compile.")
            .unwrap();
        assert!(
            app.list_compile_results()
                .unwrap()
                .iter()
                .all(|record| record.stale)
        );
        let (_, changed_hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&changed_hash), &compiled_text)
            .unwrap();
        assert!(
            app.list_compile_results()
                .unwrap()
                .iter()
                .all(|record| !record.stale)
        );

        fs::write(&output, "author edited generated output").unwrap();
        assert!(app.clean_compile_configuration("plain").is_err());
        assert_eq!(
            fs::read_to_string(&output).unwrap(),
            "author edited generated output"
        );
        assert!(app.compile_configuration("plain", false).is_err());
        app.compile_configuration("plain", true).unwrap();

        let cleaned = app.clean_compile_configuration("plain").unwrap();
        assert!(cleaned.contains(&recipe.output));
        assert!(!output.exists());
        assert!(output.parent().unwrap().join("author-note.txt").exists());
        let (source, actual_hash) = app.load_scene_text(scene).unwrap();
        assert_eq!(source, "Canonical prose remains here. TODO");
        assert_eq!(actual_hash, expected_hash);
    }

    #[test]
    fn compile_diagnostics_include_revision_mapped_proofreading_and_werror_blocks_output() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let (_, original_hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&original_hash), "Mara said said.")
            .unwrap();

        let diagnostics = app.compile_diagnostics().unwrap();
        let repeated = diagnostics
            .iter()
            .find(|item| item.code == "repeated-word")
            .unwrap();
        assert_eq!(repeated.category, "mechanical-proofreading");
        assert_eq!(
            repeated.source_entity_id.as_deref(),
            Some(scene.to_string().as_str())
        );
        let range = repeated.source_range.as_ref().unwrap();
        assert_eq!("Mara said said.".get(range.start..range.end), Some("said"));
        assert_eq!(
            repeated.severity,
            story_document::DiagnosticSeverity::Warning
        );

        app.create_builtin_compile_configuration("strict", "plain")
            .unwrap();
        let config_path = project.join(".writers-ide/compile-configurations/strict.toml");
        let mut recipe = story_document::EditionRecipe::from_toml(
            &std::fs::read_to_string(&config_path).unwrap(),
        )
        .unwrap();
        recipe.diagnostics.warnings_as_errors = true;
        std::fs::write(&config_path, recipe.to_toml().unwrap()).unwrap();
        let failed = app.compile_configuration("strict", false).unwrap();
        assert_eq!(failed.status, "failed");
        assert!(failed.outputs.is_empty());
        let repeated = failed
            .diagnostics
            .iter()
            .find(|item| item.code == "repeated-word")
            .unwrap();
        assert_eq!(
            repeated.severity,
            story_document::DiagnosticSeverity::Warning
        );
        assert_eq!(
            repeated.effective_level,
            Some(story_document::DiagnosticLevel::Error)
        );
        assert!(
            !project
                .join("exports/compile/strict/manuscript.txt")
                .exists()
        );

        app.create_builtin_compile_configuration("quiet", "plain")
            .unwrap();
        let config_path = project.join(".writers-ide/compile-configurations/quiet.toml");
        let mut recipe = story_document::EditionRecipe::from_toml(
            &std::fs::read_to_string(&config_path).unwrap(),
        )
        .unwrap();
        recipe.diagnostics.preset = story_document::DiagnosticPreset::Quiet;
        std::fs::write(config_path, recipe.to_toml().unwrap()).unwrap();
        let quiet = app.compile_configuration("quiet", false).unwrap();
        assert_eq!(quiet.status, "success");
        assert!(quiet.diagnostics.is_empty());
    }

    #[test]
    fn finding_suppressions_are_audited_expiring_and_revalidated_against_current_findings() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Suppressions");
        let mut app = WriterApp::create_project(&project, "Suppressions").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "TODO: resolve this clue.")
            .unwrap();
        let finding = app
            .compile_diagnostics()
            .unwrap()
            .into_iter()
            .find(|diagnostic| diagnostic.code == "PUB004")
            .unwrap();
        let id = app
            .add_diagnostic_suppression(
                &finding,
                "finding",
                "Intentional placeholder during draft",
                None,
            )
            .unwrap();
        let current = app.compile_diagnostics().unwrap();
        let finding = current.iter().find(|item| item.code == "PUB004").unwrap();
        assert!(finding.suppressed);
        assert_eq!(finding.suppressed_by.as_deref(), Some(id.as_str()));
        let record = app.diagnostic_suppressions().unwrap().remove(0);
        assert_eq!(record.status, "active");
        assert_eq!(record.reason, "Intentional placeholder during draft");
        assert!(record.triggering_fingerprint.is_some());
        app.create_builtin_compile_configuration("strict", "plain")
            .unwrap();
        let config_path = project.join(".writers-ide/compile-configurations/strict.toml");
        let mut recipe =
            story_document::EditionRecipe::from_toml(&fs::read_to_string(&config_path).unwrap())
                .unwrap();
        recipe.diagnostics.warnings_as_errors = true;
        fs::write(&config_path, recipe.to_toml().unwrap()).unwrap();
        let build = app.compile_configuration("strict", false).unwrap();
        assert_eq!(build.status, "success");
        let logged_finding = build
            .diagnostics
            .iter()
            .find(|item| item.code == "PUB004")
            .unwrap();
        assert!(logged_finding.suppressed);
        assert_eq!(
            logged_finding.effective_level,
            Some(story_document::DiagnosticLevel::Error)
        );

        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "The clue remains unresolved.")
            .unwrap();
        assert!(
            app.compile_diagnostics()
                .unwrap()
                .iter()
                .all(|item| item.code != "PUB004")
        );
        assert_eq!(app.diagnostic_suppressions().unwrap()[0].status, "stale");

        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "TODO: resolve this clue.")
            .unwrap();
        let restored = app
            .compile_diagnostics()
            .unwrap()
            .into_iter()
            .find(|item| item.code == "PUB004")
            .unwrap();
        assert!(restored.suppressed);
        assert_eq!(app.diagnostic_suppressions().unwrap()[0].status, "active");
        drop(app);
        let reopened = WriterApp::open_project(&project).unwrap();
        assert_eq!(reopened.diagnostic_suppressions().unwrap()[0].id, id);
    }

    #[test]
    fn compile_diagnostics_add_thresholded_plot_gaps_and_gate_timeline_checks_by_preference() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(directory.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let first = app.create_scene(chapter, "First", 0).unwrap();
        let middle = app.create_scene(chapter, "Middle", 1).unwrap();
        let last = app.create_scene(chapter, "Last", 2).unwrap();
        let character = app.create_character("Mara").unwrap();
        app.link_character(first, character, ParticipationRole::Major)
            .unwrap();
        app.link_character(last, character, ParticipationRole::Major)
            .unwrap();
        app.update_scene_story_interval(
            first,
            StoryInterval::closed(StoryTime::exact(0), StoryTime::exact(100)),
        )
        .unwrap();
        app.update_scene_story_interval(
            last,
            StoryInterval::closed(StoryTime::exact(50), StoryTime::exact(75)),
        )
        .unwrap();
        let plotline = app.create_plotline("The Compass").unwrap();
        for scene in [first, last] {
            app.set_scene_plotlines(
                scene,
                &[ScenePlotline {
                    plotline_id: plotline,
                    role: "advances".into(),
                    share_basis_points: None,
                }],
                WordAttributionPolicy::Overlap,
            )
            .unwrap();
        }
        assert_eq!(app.get_scene(middle).unwrap().scene.title, "Middle");

        let policy = story_document::DiagnosticPolicy {
            minimum_plotline_gap_scenes: Some(1),
            ..story_document::DiagnosticPolicy::default()
        };
        let disabled = app.compile_diagnostics_with_policy(&policy).unwrap();
        assert!(disabled.iter().any(|finding| finding.code == "PLOT001"));
        assert!(!disabled.iter().any(|finding| finding.code == "CHR001"));

        app.set_chronology_preferences(ChronologyPreferences {
            enabled: true,
            ..ChronologyPreferences::default()
        })
        .unwrap();
        let bridge = app.create_location("Bridge").unwrap();
        let vault = app.create_location("Vault").unwrap();
        for (scene_id, location) in [(first, bridge), (last, vault)] {
            let mut scene = app.get_scene(scene_id).unwrap().scene;
            scene.start_location = Some(location);
            app.update_scene_metadata(&scene).unwrap();
        }
        app.add_location_incompatibility_rule(&LocationIncompatibilityRule {
            id: ContinuityRuleId::new(),
            first_location: bridge,
            second_location: vault,
            description: "Mara cannot be in both places".into(),
            enabled: true,
        })
        .unwrap();
        let predecessor = EventId::new();
        let successor = EventId::new();
        app.add_story_event(&StoryEvent {
            id: predecessor,
            title: "Arrival".into(),
            description: String::new(),
            start: StoryTime::exact(80),
            end: StoryTime::exact(90),
            source_time_text: "later".into(),
        })
        .unwrap();
        app.add_story_event(&StoryEvent {
            id: successor,
            title: "Departure".into(),
            description: String::new(),
            start: StoryTime::exact(10),
            end: StoryTime::exact(20),
            source_time_text: "earlier".into(),
        })
        .unwrap();
        app.link_event(first, predecessor, "depicts").unwrap();
        app.link_event(last, successor, "depicts").unwrap();
        app.add_event_dependency(&StoryEventDependency {
            predecessor,
            successor,
            kind: "precedes".into(),
            notes: "Arrival must precede departure".into(),
        })
        .unwrap();
        let enabled = app.compile_diagnostics_with_policy(&policy).unwrap();
        let overlap = enabled
            .iter()
            .find(|finding| finding.code == "CHR001")
            .unwrap();
        assert_eq!(
            overlap.source_entity_id.as_deref(),
            Some(first.to_string().as_str())
        );
        assert_eq!(
            overlap.severity,
            story_document::DiagnosticSeverity::Warning
        );
        assert!(enabled.iter().any(|finding| finding.code == "PLOT001"));
        assert!(enabled.iter().any(|finding| finding.code == "LOC001"));
        assert!(enabled.iter().any(|finding| finding.code == "EVT001"));
        let extended = app
            .compile_diagnostics_with_policy(&story_document::DiagnosticPolicy {
                preset: story_document::DiagnosticPreset::Extended,
                ..story_document::DiagnosticPolicy::default()
            })
            .unwrap();
        assert!(!extended.iter().any(|finding| finding.code == "PLOT001"));
        let pedantic = app
            .compile_diagnostics_with_policy(&story_document::DiagnosticPolicy {
                preset: story_document::DiagnosticPreset::Pedantic,
                ..story_document::DiagnosticPolicy::default()
            })
            .unwrap();
        assert!(pedantic.iter().any(|finding| finding.code == "PLOT001"));
    }

    #[test]
    fn diagnostic_suppression_rejects_operational_errors_and_expires_without_hiding_a_finding() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Expiry"), "Expiry").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "TODO: review.")
            .unwrap();
        let finding = app
            .compile_diagnostics()
            .unwrap()
            .into_iter()
            .find(|diagnostic| diagnostic.code == "PUB004")
            .unwrap();
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
            + 1;
        app.add_diagnostic_suppression(&finding, "rule", "Temporary review", Some(expiry))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        let current = app
            .compile_diagnostics()
            .unwrap()
            .into_iter()
            .find(|item| item.code == "PUB004")
            .unwrap();
        assert!(!current.suppressed);
        assert_eq!(app.diagnostic_suppressions().unwrap()[0].status, "expired");

        let renderer_error = story_document::CompileDiagnostic {
            code: "TEX_FATAL".into(),
            category: "renderer".into(),
            severity: story_document::DiagnosticSeverity::Error,
            effective_level: Some(story_document::DiagnosticLevel::Error),
            message: "renderer failed".into(),
            remediation_hint: None,
            source_entity_id: None,
            source_range: None,
            generated_file: None,
            generated_line: None,
            suppressed_by: None,
            suppressed: false,
        };
        assert!(
            app.add_diagnostic_suppression(&renderer_error, "rule", "hide fatal", None)
                .is_err()
        );
    }

    #[test]
    fn compile_configuration_paths_cannot_escape_managed_output_or_follow_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let app = WriterApp::create_project(&project, "Novel").unwrap();
        let mut recipe = story_document::EditionRecipe {
            version: 1,
            name: "Unsafe".into(),
            format: story_document::CompileFormat::Markdown,
            assembly_id: None,
            output: "exports/compile/unsafe/../../manuscript/scene.md".into(),
            metadata_title: None,
            metadata_author: None,
            cover_image: None,
            design: story_document::BookDesign::default(),
            expand_transclusions: false,
            spellcheck: false,
            language: None,
            dictionary: None,
            spelling_engine: story_document::SpellcheckEngine::default(),
            diagnostics: story_document::DiagnosticPolicy::default(),
        };
        assert!(app.create_compile_configuration("unsafe", &recipe).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let outside = dir.path().join("outside");
            fs::create_dir(&outside).unwrap();
            let config_project = dir.path().join("Config symlink");
            let config_app = WriterApp::create_project(&config_project, "Config symlink").unwrap();
            symlink(
                &outside,
                config_project.join(".writers-ide/compile-configurations"),
            )
            .unwrap();
            let safe_recipe = story_document::EditionRecipe {
                output: "exports/compile/unsafe/manuscript.md".into(),
                ..recipe.clone()
            };
            assert!(
                config_app
                    .create_compile_configuration("unsafe", &safe_recipe)
                    .is_err()
            );
            assert!(fs::read_dir(&outside).unwrap().next().is_none());

            let exports = project.join("exports");
            fs::create_dir_all(&exports).unwrap();
            symlink(&outside, exports.join("compile")).unwrap();
            recipe.output = "exports/compile/unsafe/manuscript.md".into();
            app.create_compile_configuration("unsafe", &recipe).unwrap();
            assert!(app.compile_configuration("unsafe", false).is_err());
            assert!(fs::read_dir(&outside).unwrap().next().is_none());
        }
    }

    #[test]
    fn one_off_compile_publication_protects_sources_and_requires_explicit_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let (_, source_hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&source_hash), "Canonical source text.")
            .unwrap();
        let (source, expected_hash) = app.load_scene_text(scene).unwrap();
        let mut artifact = app
            .compile_artifact(story_document::CompileFormat::Markdown, false)
            .unwrap();
        artifact
            .diagnostics
            .push(story_document::CompileDiagnostic {
                code: "TESTGEN".into(),
                category: "renderer".into(),
                effective_level: None,
                severity: story_document::DiagnosticSeverity::Warning,
                message: "Generated TeX diagnostic".into(),
                remediation_hint: None,
                source_entity_id: None,
                source_range: None,
                generated_file: Some("lualatex.log".into()),
                generated_line: Some(2),
                suppressed_by: None,
                suppressed: false,
            });
        let source_path = app
            .list_project_symbols()
            .unwrap()
            .into_iter()
            .find(|symbol| symbol.entity_id == scene.to_string())
            .unwrap()
            .source_path
            .unwrap();
        let canonical = project.join(source_path);
        assert!(
            app.publish_compile_artifact(&canonical, &artifact, true)
                .is_err()
        );
        assert_eq!(
            app.load_scene_text(scene).unwrap(),
            (source.clone(), expected_hash.clone())
        );

        let output = project.join("exports/one-off.md");
        let files = app
            .publish_compile_artifact(&output, &artifact, false)
            .unwrap();
        assert!(app.list_compile_results().unwrap().iter().any(|record| {
            record.succeeded
                && record.version == 6
                && record.diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == "TESTGEN"
                        && diagnostic.generated_file.as_deref() == Some("lualatex.log")
                        && diagnostic.generated_line == Some(2)
                })
        }));
        assert_eq!(files.len(), 2);
        let original = fs::read(&output).unwrap();
        assert!(
            app.publish_compile_artifact(&output, &artifact, false)
                .is_err()
        );
        assert_eq!(fs::read(&output).unwrap(), original);
        assert!(
            app.publish_compile_artifact(&output, &artifact, true)
                .is_ok()
        );
        assert_eq!(app.load_scene_text(scene).unwrap(), (source, expected_hash));

        let edition_output = project.join("exports/edition.md");
        let sidecar_name = "edition.md.spellcheck.json".to_owned();
        let sidecar_bytes = br#"[{"scene":"reviewed"}]"#.to_vec();
        let published = app
            .publish_compile_artifact_with_sidecars(
                &edition_output,
                &artifact,
                false,
                &[(sidecar_name.clone(), sidecar_bytes.clone())],
            )
            .unwrap();
        assert_eq!(published.len(), 3);
        let sidecar = project.join("exports").join(&sidecar_name);
        assert_eq!(fs::read(&sidecar).unwrap(), sidecar_bytes);
        assert!(
            app.publish_compile_artifact_with_sidecars(
                &edition_output,
                &artifact,
                false,
                &[(sidecar_name, b"replacement".to_vec())],
            )
            .is_err()
        );
        assert_eq!(fs::read(&sidecar).unwrap(), br#"[{"scene":"reviewed"}]"#);
        assert!(
            app.publish_compile_artifact_with_sidecars(
                project.join("exports/unsafe.md"),
                &artifact,
                false,
                &[("../outside.json".into(), b"escape".to_vec())],
            )
            .is_err()
        );
        let results = app.list_compile_results().unwrap();
        assert!(results.iter().any(|record| {
            record.configuration == "One-off Compile"
                && record.format.as_deref() == Some("MD")
                && record.succeeded
                && record
                    .outputs
                    .iter()
                    .any(|path| path.ends_with("one-off.md"))
        }));
        assert!(results.iter().any(|record| {
            record.configuration == "One-off Compile"
                && record.format.as_deref() == Some("MD")
                && !record.succeeded
                && record
                    .error
                    .as_deref()
                    .is_some_and(|message| message.contains("already exists"))
        }));
    }

    #[test]
    fn compile_publication_journal_rolls_back_after_crash_and_project_relocation() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Journal project");
        let app = WriterApp::create_project(&project, "Journal project").unwrap();
        let project_id = app.get_project().unwrap().id;
        let first = project.join("exports/book.epub");
        let second = project.join("exports/book.epub.source-map.json");
        fs::create_dir_all(first.parent().unwrap()).unwrap();
        fs::write(&first, b"previous epub").unwrap();
        let files = vec![
            (first.clone(), b"proposed epub".to_vec()),
            (second.clone(), b"proposed source map".to_vec()),
        ];
        assert!(
            publish_compile_file_set_with_failpoint(
                &project,
                project_id,
                &files,
                Some(CompilePublishFailpoint::OutputPublished(0)),
            )
            .is_err()
        );
        assert_eq!(fs::read(&first).unwrap(), b"proposed epub");
        assert!(!second.exists());
        drop(app);

        let relocated = directory.path().join("Moved project");
        fs::rename(&project, &relocated).unwrap();
        let reopened = WriterApp::open_project(&relocated).unwrap();
        assert_eq!(
            fs::read(relocated.join("exports/book.epub")).unwrap(),
            b"previous epub"
        );
        assert!(!relocated.join("exports/book.epub.source-map.json").exists());
        assert_eq!(reopened.compile_recovery_notice(), None);
        assert!(!compile_publication_journal_path(&relocated).exists());
        assert!(
            fs::read_dir(relocated.join("exports"))
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains("writers-ide-compile-"))
        );
    }

    #[test]
    fn compile_publication_journal_finishes_committed_cleanup_and_preserves_conflicts() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Committed project");
        let app = WriterApp::create_project(&project, "Committed project").unwrap();
        let project_id = app.get_project().unwrap().id;
        let output = project.join("exports/book.epub");
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(&output, b"previous").unwrap();
        let files = vec![(output.clone(), b"committed".to_vec())];
        assert!(
            publish_compile_file_set_with_failpoint(
                &project,
                project_id,
                &files,
                Some(CompilePublishFailpoint::CommitWritten),
            )
            .is_err()
        );
        drop(app);
        let reopened = WriterApp::open_project(&project).unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"committed");
        assert_eq!(reopened.compile_recovery_notice(), None);
        assert!(!compile_publication_journal_path(&project).exists());
        drop(reopened);

        let project = directory.path().join("Conflict project");
        let app = WriterApp::create_project(&project, "Conflict project").unwrap();
        let project_id = app.get_project().unwrap().id;
        let output = project.join("exports/book.epub");
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(&output, b"previous").unwrap();
        let files = vec![(output.clone(), b"proposed".to_vec())];
        assert!(
            publish_compile_file_set_with_failpoint(
                &project,
                project_id,
                &files,
                Some(CompilePublishFailpoint::IntentWritten),
            )
            .is_err()
        );
        fs::write(&output, b"external edit").unwrap();
        drop(app);
        let app = WriterApp::open_project(&project).unwrap();
        let notice = app.compile_recovery_notice().unwrap();
        assert!(notice.contains("conflicting versions"));
        assert_eq!(fs::read(&output).unwrap(), b"external edit");
        let recovery_files = fs::read_dir(output.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(recovery_files.iter().any(|name| name.ends_with(".old")));
        assert!(compile_publication_journal_path(&project).exists());
        let artifact = app
            .compile_artifact(story_document::CompileFormat::Markdown, false)
            .unwrap();
        assert!(
            app.publish_compile_artifact(&output, &artifact, true)
                .unwrap_err()
                .to_string()
                .contains("conflicting versions")
        );
        assert_eq!(fs::read(&output).unwrap(), b"external edit");
        fs::write(&output, b"previous").unwrap();
        assert_eq!(app.retry_compile_publication_recovery().unwrap(), None);
        assert_eq!(fs::read(&output).unwrap(), b"previous");
        assert!(!compile_publication_journal_path(&project).exists());
    }

    #[cfg(unix)]
    #[test]
    fn one_off_compile_publication_rejects_symlinked_output_directories() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let artifact = app
            .compile_artifact(story_document::CompileFormat::Markdown, false)
            .unwrap();
        let outside = dir.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::remove_dir(project.join("exports")).unwrap();
        symlink(&outside, project.join("exports")).unwrap();
        assert!(
            app.publish_compile_artifact(project.join("exports/escaped.md"), &artifact, false)
                .is_err()
        );
        assert!(fs::read_dir(outside).unwrap().next().is_none());
        assert!(app.load_scene_text(scene).unwrap().0.is_empty());
    }

    #[test]
    fn staged_manuscript_import_creates_scenes_and_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Imported"), "Imported").unwrap();
        let staging = story_import::ManuscriptStaging {
            source_type: story_import::SourceType::Rtf,
            source_path: "/fixtures/source.rtf".into(),
            source_fingerprint: "a".repeat(64),
            title: "Source book".into(),
            units: vec![story_import::ManuscriptUnit {
                source_document_id: "source.rtf:unit:0".into(),
                title: "Opening scene".into(),
                heading_level: None,
                body_markdown: "Imported prose.".into(),
                unsupported: vec!["Formatting needs review".into()],
            }],
            warnings: Vec::new(),
            unsupported: vec!["Some formatting is unsupported".into()],
        };
        let (run, scenes) = app.import_staged_manuscript(&staging).unwrap();
        assert_eq!(scenes.len(), 1);
        assert_eq!(app.load_scene_text(scenes[0]).unwrap().0, "Imported prose.");
        let summary = app.get_import_run_summary(run).unwrap();
        assert_eq!(summary.source_items, 1);
        assert_eq!(summary.provenance_records, 2);
        assert_eq!(summary.status, "incomplete");
    }

    #[test]
    fn text_and_org_staging_import_into_scenes_without_touching_source_files() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("outside.txt");
        let org = dir.path().join("outline.org");
        fs::write(&plain, "Outside reference prose.\n\nSecond paragraph.").unwrap();
        fs::write(&org, "* Opening\n\nAn /important/ beat.").unwrap();
        let plain_before = fs::read(&plain).unwrap();
        let org_before = fs::read(&org).unwrap();
        let plain_staging = story_import::inspect_manuscript(&plain).unwrap();
        let org_staging = story_import::inspect_manuscript(&org).unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Imported"), "Imported").unwrap();

        let (plain_run, plain_scenes) = app.import_staged_manuscript(&plain_staging).unwrap();
        let (org_run, org_scenes) = app.import_staged_manuscript(&org_staging).unwrap();

        assert_eq!(plain_scenes.len(), 1);
        assert_eq!(org_scenes.len(), 1);
        assert_eq!(
            app.load_scene_text(plain_scenes[0]).unwrap().0,
            "Outside reference prose.\n\nSecond paragraph."
        );
        assert!(
            app.load_scene_text(org_scenes[0])
                .unwrap()
                .0
                .contains("*important*")
        );
        assert_eq!(
            app.get_import_run_summary(plain_run).unwrap().status,
            "completed"
        );
        assert_eq!(
            app.get_import_run_summary(org_run).unwrap().status,
            "completed"
        );
        assert_eq!(fs::read(plain).unwrap(), plain_before);
        assert_eq!(fs::read(org).unwrap(), org_before);
    }

    #[test]
    fn org_heading_import_maps_level_one_to_chapters_and_level_two_to_scenes() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("outline.org");
        fs::write(
            &source,
            "* Act One\nIntroduction prose.\n** Opening\nThe door opens.\n** Decision\nMara chooses.",
        )
        .unwrap();
        let preview = story_import::inspect_text_document(
            &source,
            story_import::OrgImportMode::HeadingHierarchy,
        )
        .unwrap();
        let staging = story_import::ManuscriptStaging {
            source_type: story_import::SourceType::Org,
            source_path: preview.source_path,
            source_fingerprint: preview.source_fingerprint,
            title: preview.title,
            units: preview
                .units
                .into_iter()
                .map(|unit| story_import::ManuscriptUnit {
                    source_document_id: unit.source_document_id,
                    title: unit.title,
                    heading_level: unit.heading_level,
                    body_markdown: unit.body_markdown,
                    unsupported: Vec::new(),
                })
                .collect(),
            warnings: preview.warnings,
            unsupported: preview.unsupported,
        };
        let mut app = WriterApp::create_project(dir.path().join("Imported"), "Imported").unwrap();
        let (_, scenes) = app.import_staged_manuscript(&staging).unwrap();
        let chapters = app
            .list_structural_nodes()
            .unwrap()
            .into_iter()
            .filter(|node| node.kind == "chapter")
            .collect::<Vec<_>>();
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].title, "Act One");
        assert_eq!(scenes.len(), 3);
        assert!(
            scenes
                .iter()
                .all(|scene| app.get_scene(*scene).unwrap().chapter_id == Some(chapters[0].id))
        );
        assert!(
            scenes
                .iter()
                .any(|scene| { app.get_scene(*scene).unwrap().scene.title == "Opening" })
        );
        assert!(
            scenes
                .iter()
                .any(|scene| { app.get_scene(*scene).unwrap().scene.title == "Decision" })
        );
    }

    #[test]
    fn scrivener_preview_import_preserves_hierarchy_prose_research_and_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("SeriesBook.scriv");
        let scene_files = source.join("Files/Data/scene-1");
        fs::create_dir_all(&scene_files).unwrap();
        fs::write(
            source.join("SeriesBook.scrivx"),
            r#"<ScrivenerProject Version="2.0"><Binder>
              <BinderItem UUID="chapter-1" Type="Folder"><Title>Chapter One</Title><Children>
                <BinderItem UUID="scene-1" Type="Text"><Title>Opening</Title><IncludeInCompile>Yes</IncludeInCompile><Children/></BinderItem>
              </Children></BinderItem>
              <BinderItem UUID="research-1" Type="Folder"><Title>Research</Title><Children/></BinderItem>
            </Binder></ScrivenerProject>"#,
        )
        .unwrap();
        fs::write(
            scene_files.join("content.rtf"),
            r"{\rtf1\ansi Élodie entered.}",
        )
        .unwrap();
        fs::write(scene_files.join("synopsis.txt"), "The door opens").unwrap();
        let source_before = story_import::inspect_scrivener_project(&source).unwrap();
        let source_fingerprint = source_before.source_fingerprint.clone();
        let mut app = WriterApp::create_project(dir.path().join("Imported"), "Imported").unwrap();
        let inventory = story_import::inspect_scrivener_project(&source).unwrap();
        let mut plan = story_import::ScrivenerImportPlan::proposed(&inventory);
        plan.decisions
            .iter_mut()
            .find(|decision| decision.source_document_id == "chapter-1")
            .unwrap()
            .destination_kind = story_import::ScrivenerDestinationKind::Chapter;
        assert!(
            plan.preview(&inventory)
                .unwrap()
                .unresolved_source_ids
                .is_empty()
        );

        let (run, scenes) = app
            .import_scrivener_inventory(&inventory, &plan, story_import::ScrivenerImportMode::Both)
            .unwrap();
        assert_eq!(scenes.len(), 1);
        assert_eq!(app.load_scene_text(scenes[0]).unwrap().0, "Élodie entered.");
        let scene = app.get_scene(scenes[0]).unwrap();
        assert_eq!(scene.scene.synopsis, "The door opens");
        assert_eq!(app.list_structural_nodes().unwrap().len(), 1);
        assert_eq!(app.list_reading_documents().unwrap().len(), 1);
        assert_eq!(
            story_import::inspect_scrivener_project(&source)
                .unwrap()
                .source_fingerprint,
            source_fingerprint
        );
        let snapshot_root = app.project_root().join("research/imported_sources");
        let snapshots = fs::read_dir(snapshot_root).unwrap().count();
        assert_eq!(snapshots, 1);
        let summary = app.get_import_run_summary(run).unwrap();
        assert_eq!(summary.source_items, 3);
        assert_eq!(summary.provenance_records, 3);
        assert_eq!(summary.status, "incomplete");
        assert!(
            summary
                .source_identifier
                .starts_with("research/imported_sources/")
        );
    }

    #[test]
    fn editorial_merge_saves_disjoint_changes_and_preserves_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let base = "One.\nTwo.\nThree.\n";
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "One.\nTwo local.\nThree.\n")
            .unwrap();
        let merged = app
            .merge_review_text(scene, base, "One.\nTwo.\nThree external.\n")
            .unwrap();
        assert_eq!(
            merged.merged_text.as_deref(),
            Some("One.\nTwo local.\nThree external.\n")
        );
        let conflict = app
            .merge_review_text(scene, base, "One.\nTwo reviewed.\nThree.\n")
            .unwrap();
        assert!(conflict.merged_text.is_none());
        assert_eq!(
            app.load_scene_text(scene).unwrap().0,
            "One.\nTwo local.\nThree external.\n"
        );
        let resolved = app
            .resolve_review_text(
                scene,
                base,
                "One.\nTwo reviewed.\nThree.\n",
                &[story_import::MergeConflictChoice::External],
            )
            .unwrap();
        assert_eq!(resolved, "One.\nTwo reviewed.\nThree.\n");
    }

    #[test]
    fn docx_revision_disposition_merges_without_modifying_the_review_file() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let base = "The old door.\n";
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), base).unwrap();
        let docx = dir.path().join("review.docx");
        let file = std::fs::File::create(&docx).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        archive
            .start_file("word/document.xml", zip::write::FileOptions::default())
            .unwrap();
        archive
            .write_all(b"<w:document><w:body><w:p><w:r><w:t>The </w:t></w:r><w:del w:id=\"1\"><w:r><w:delText>old</w:delText></w:r></w:del><w:ins w:id=\"2\"><w:r><w:t>new</w:t></w:r></w:ins><w:r><w:t> door.</w:t></w:r></w:p></w:body></w:document>")
            .unwrap();
        archive.finish().unwrap();
        let source = std::fs::read(&docx).unwrap();

        let merged = app.merge_docx_review(scene, base, &docx, true).unwrap();
        assert_eq!(merged.merged_text.as_deref(), Some("The new door."));
        assert_eq!(app.load_scene_text(scene).unwrap().0, "The new door.");
        assert_eq!(std::fs::read(&docx).unwrap(), source);
    }

    #[test]
    fn diagnostics_config_is_project_local_typed_and_used_by_application() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Novel");
        let app = WriterApp::create_project(&project, "Novel").unwrap();
        let config_path = app.write_default_proofreading_config().unwrap();
        assert!(config_path.starts_with(&project));
        let drafting = std::fs::read_to_string(&config_path).unwrap();
        assert!(drafting.contains("severity = \"information\""));
        assert!(app.write_default_proofreading_config().is_err());
        let default_findings = app.proofread_text("Mara was followed.").unwrap();
        assert!(default_findings.is_empty());
        std::fs::write(
            &config_path,
            "version=1\n[[rules]]\nid='passive'\nenabled=true\nkind='regex'\npattern='\\bwas\\s+\\w+ed\\b'\nseverity='warning'\nmessage='Review passive phrasing'\n",
        ).unwrap();
        let configured = app.proofread_text("Mara was followed.").unwrap();
        assert_eq!(configured.len(), 1);
        assert_eq!(configured[0].category, "passive");
        assert!(configured[0].byte_range.end <= "Mara was followed.".len());

        let copyedit_project = directory.path().join("Copyedit");
        let copyedit = WriterApp::create_project(&copyedit_project, "Copyedit").unwrap();
        let copyedit_path = copyedit
            .write_proofreading_config(story_analysis::ProofreadingPreset::Copyedit)
            .unwrap();
        let copyedit_source = std::fs::read_to_string(copyedit_path).unwrap();
        assert!(copyedit_source.contains("severity = \"warning\""));
        assert_eq!(
            copyedit.proofread_text("Mara  followed !").unwrap().len(),
            2
        );
    }

    #[test]
    fn optional_processing_transclusion_expands_a_derived_compile_view() {
        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("Novel");
        let mut app = WriterApp::create_project(&project_path, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let source = app.create_scene(chapter, "Source", 0).unwrap();
        let target = app.create_scene(chapter, "Target", 1).unwrap();
        let (_, source_hash) = app.load_scene_text(source).unwrap();
        app.save_scene_text(source, Some(&source_hash), "Source passage.")
            .unwrap();
        let (_, target_hash) = app.load_scene_text(target).unwrap();
        app.save_scene_text(target, Some(&target_hash), "Included passage.")
            .unwrap();
        app.create_story_link(&StoryLink {
            id: LinkId::new(),
            source: EntityRef::Scene(source),
            source_anchor: None,
            target: EntityRef::Scene(target),
            target_anchor: None,
            kind: LinkKind::Transcludes,
            label: String::new(),
        })
        .unwrap();
        let research = app
            .create_research_item(
                "Background document",
                "Document body included in the compilation.",
                "text/markdown",
                None,
                None,
            )
            .unwrap();
        app.create_story_link(&StoryLink {
            id: LinkId::new(),
            source: EntityRef::Scene(target),
            source_anchor: None,
            target: EntityRef::Research(research),
            target_anchor: None,
            kind: LinkKind::Includes,
            label: "appendix excerpt".into(),
        })
        .unwrap();
        let source_bytes = b"Reference body is read-only.".to_vec();
        let reference = story_reading::ReferenceDocument {
            title: "Imported reference".into(),
            media_type: "text/plain".into(),
            sha256: format!("{:x}", Sha256::digest(&source_bytes)),
            extracted_text: "Reference body is read-only.".into(),
            source_bytes,
            language_code: None,
        };
        let reading = app
            .import_reading_document(&reference, "fixture:read-only")
            .unwrap();
        app.create_story_link(&StoryLink {
            id: LinkId::new(),
            source: EntityRef::Scene(target),
            source_anchor: None,
            target: EntityRef::Reading(reading),
            target_anchor: None,
            kind: LinkKind::Includes,
            label: "quoted source".into(),
        })
        .unwrap();

        app.run_processing().unwrap();
        let ordinary = fs::read_to_string(project_path.join("exports/compiled.md")).unwrap();
        assert_eq!(ordinary.matches("Included passage.").count(), 1);

        let reports = app.run_processing_with_transclusions().unwrap();
        assert!(reports[0].message.contains("inclusions expanded"));
        let expanded = fs::read_to_string(project_path.join("exports/compiled.md")).unwrap();
        assert_eq!(expanded.matches("Included passage.").count(), 2);
        assert!(expanded.contains("Source passage.\n\n---\n\nIncluded passage."));
        assert!(expanded.contains("Document body included in the compilation."));
        assert!(expanded.contains("Reference body is read-only."));
        let reading_record = app.get_reading_document(reading).unwrap();
        assert_eq!(
            fs::read(project_path.join(reading_record.relative_path)).unwrap(),
            reference.source_bytes
        );
        assert_eq!(app.load_scene_text(source).unwrap().0, "Source passage.");
        assert_eq!(app.load_scene_text(target).unwrap().0, "Included passage.");
    }

    #[test]
    fn analytics_reads_relationships_without_mutating_story_data() {
        let d = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(d.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "One", 0).unwrap();
        let first = app.create_scene(chapter, "First", 0).unwrap();
        let _second = app.create_scene(chapter, "Second", 1).unwrap();
        let character = app.create_character("Mara").unwrap();
        app.link_character(first, character, ParticipationRole::Pov)
            .unwrap();
        let plot = app.create_plotline("A thread").unwrap();
        app.set_scene_plotlines(
            first,
            &[ScenePlotline {
                plotline_id: plot,
                role: "primary".into(),
                share_basis_points: None,
            }],
            WordAttributionPolicy::Overlap,
        )
        .unwrap();
        let location = app.create_location("Harbor").unwrap();
        let mut scene_metadata = app.get_scene(first).unwrap().scene;
        scene_metadata.start_location = Some(location);
        app.update_scene_metadata(&scene_metadata).unwrap();
        let event = app.add_event("The signal", "day 1").unwrap();
        app.link_event(first, event, "depicts").unwrap();
        let before = app
            .list_scenes(false)
            .unwrap()
            .iter()
            .map(|s| s.scene.id)
            .collect::<Vec<_>>();
        let data = app.analytics_dataset().unwrap();
        let after = app
            .list_scenes(false)
            .unwrap()
            .iter()
            .map(|s| s.scene.id)
            .collect::<Vec<_>>();
        assert_eq!(before, after);
        assert_eq!(data[0].characters[0].name, "Mara");
        assert_eq!(data[0].plotlines[0].name, "A thread");
        assert_eq!(data[0].locations[0].name, "Harbor");
        assert_eq!(data[0].events[0].name, "The signal");
    }

    #[test]
    fn location_incompatibility_rules_are_persisted_and_evaluated_read_only() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "One", 0).unwrap();
        let first = app.create_scene(chapter, "At the bridge", 0).unwrap();
        let second = app.create_scene(chapter, "In the vault", 1).unwrap();
        let character = app.create_character("Mara").unwrap();
        app.link_character(first, character, ParticipationRole::Major)
            .unwrap();
        app.link_character(second, character, ParticipationRole::Major)
            .unwrap();
        let bridge = app.create_location("Bridge").unwrap();
        let vault = app.create_location("Vault").unwrap();
        for (scene_id, location, start, end) in [(first, bridge, 0, 10), (second, vault, 5, 15)] {
            let mut scene = app.get_scene(scene_id).unwrap().scene;
            scene.start_location = Some(location);
            scene.start_time = StoryTime::exact(start);
            scene.end_time = StoryTime::exact(end);
            app.update_scene_metadata(&scene).unwrap();
        }
        let rule = LocationIncompatibilityRule {
            id: ContinuityRuleId::new(),
            first_location: bridge,
            second_location: vault,
            description: "Mara cannot be at both locations at once".into(),
            enabled: true,
        };
        app.add_location_incompatibility_rule(&rule).unwrap();
        assert_eq!(
            app.inspect_location_incompatibilities().unwrap()[0].status,
            story_analysis::LocationRuleFindingStatus::DefiniteConflict
        );
        let mut disabled = rule.clone();
        disabled.enabled = false;
        app.update_location_incompatibility_rule(&disabled).unwrap();
        assert!(app.inspect_location_incompatibilities().unwrap().is_empty());
        app.update_location_incompatibility_rule(&rule).unwrap();
        drop(app);
        let reopened = WriterApp::open_project(project).unwrap();
        let reopened_rules = reopened.list_location_incompatibility_rules().unwrap();
        assert_eq!(reopened_rules.len(), 1);
        assert_eq!(reopened_rules[0].id, rule.id);
        assert!(
            (reopened_rules[0].first_location == bridge
                && reopened_rules[0].second_location == vault)
                || (reopened_rules[0].first_location == vault
                    && reopened_rules[0].second_location == bridge)
        );
        assert_eq!(
            reopened.inspect_location_incompatibilities().unwrap().len(),
            1
        );
    }

    #[test]
    fn extracted_claims_require_current_exact_evidence_before_authoritative_promotion() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "One", 0).unwrap();
        let scene = app.create_scene(chapter, "Discovery", 0).unwrap();
        let source = "Mara knows the ward opens at dawn.";
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), source).unwrap();
        let proposal = story_ollama::attach_provenance(
            "scene",
            &scene.to_string(),
            source,
            "fixture-model",
            story_ollama::ExtractionResult {
                claims: vec![story_ollama::CandidateClaim {
                    kind: "character_knowledge".into(),
                    name: "Mara".into(),
                    claim: "Mara knows the ward opens at dawn.".into(),
                    evidence_quote: "Mara knows the ward opens at dawn.".into(),
                    approximate_story_time: Some("day 1 morning".into()),
                }],
            },
        );
        let mut invalid_evidence = proposal.clone();
        invalid_evidence.claims[0].evidence_start_byte = Some(1);
        assert!(app.approve_extraction_claim(&invalid_evidence, 0).is_err());
        let id = app.approve_extraction_claim(&proposal, 0).unwrap();
        assert_eq!(app.approve_extraction_claim(&proposal, 0).unwrap(), id);
        let approved = app.list_approved_knowledge_claims().unwrap();
        assert_eq!(approved.len(), 1);
        assert_eq!(approved[0].id, id);
        assert_eq!(
            approved[0].evidence_quote,
            "Mara knows the ward opens at dawn."
        );
        assert_eq!(approved[0].source_id, scene.to_string());
        assert_eq!(approved[0].model, "fixture-model");
        assert!(
            app.search_project("character_knowledge dawn", 20)
                .unwrap()
                .iter()
                .any(|hit| hit.entity_kind == "knowledge_claim" && hit.entity_id == id.to_string())
        );

        let (_, latest_hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&latest_hash), "Mara learned this yesterday.")
            .unwrap();
        assert!(app.approve_extraction_claim(&proposal, 0).is_err());
        assert_eq!(app.list_approved_knowledge_claims().unwrap().len(), 1);

        let reading_path = dir.path().join("reference.txt");
        std::fs::write(&reading_path, "The bell sounds at noon.").unwrap();
        let reference = story_reading::import_reference(&reading_path).unwrap();
        let reading_id = app
            .import_reading_document(&reference, reading_path.to_str().unwrap())
            .unwrap();
        let reading_proposal = story_ollama::attach_provenance(
            "reading",
            &reading_id.to_string(),
            &reference.extracted_text,
            "fixture-model",
            story_ollama::ExtractionResult {
                claims: vec![story_ollama::CandidateClaim {
                    kind: "event".into(),
                    name: "Noon bell".into(),
                    claim: "A bell sounds at noon.".into(),
                    evidence_quote: "The bell sounds at noon.".into(),
                    approximate_story_time: Some("noon".into()),
                }],
            },
        );
        let reading_claim = app.approve_extraction_claim(&reading_proposal, 0).unwrap();
        assert!(
            app.list_approved_knowledge_claims()
                .unwrap()
                .iter()
                .any(|record| record.id == reading_claim && record.source_kind == "reading")
        );
        assert_eq!(
            std::fs::read(&reading_path).unwrap(),
            reference.source_bytes
        );
        drop(app);
        let app = WriterApp::open_project(dir.path().join("Novel")).unwrap();
        let persisted = app.list_approved_knowledge_claims().unwrap();
        assert_eq!(persisted.len(), 2);
        assert_eq!(persisted[1].source_kind, "reading");
    }

    #[test]
    fn provisional_claims_can_be_reviewed_and_dismissed_without_losing_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "One", 0).unwrap();
        let scene = app.create_scene(chapter, "Discovery", 0).unwrap();
        let source = "Mara found the silver key.";
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), source).unwrap();
        let mut proposal = story_ollama::attach_provenance(
            "scene",
            &scene.to_string(),
            source,
            "fixture-model",
            story_ollama::ExtractionResult {
                claims: vec![story_ollama::CandidateClaim {
                    kind: "object".into(),
                    name: "Silver key".into(),
                    claim: "Mara found the key.".into(),
                    evidence_quote: "Mara found the silver key.".into(),
                    approximate_story_time: None,
                }],
            },
        );
        let filename = "review.json";
        let path = app
            .save_extraction_proposal(filename, &serde_json::to_vec(&proposal).unwrap())
            .unwrap();
        let (original, fingerprint) = app.load_extraction_proposal_file(&path).unwrap();
        let review: story_ollama::ProvisionalExtraction =
            serde_json::from_slice(&original).unwrap();
        assert!(!review.claims[0].approved);
        assert!(!review.claims[0].dismissed);
        assert!(review.claims[0].evidence_start_byte.is_some());

        app.dismiss_extraction_claim(&mut proposal, 0).unwrap();
        assert!(proposal.claims[0].dismissed);
        app.update_extraction_proposal_file(
            &path,
            &fingerprint,
            &serde_json::to_vec(&proposal).unwrap(),
        )
        .unwrap();
        let (updated, _) = app.load_extraction_proposal_file(&path).unwrap();
        let persisted: story_ollama::ProvisionalExtraction =
            serde_json::from_slice(&updated).unwrap();
        assert!(persisted.claims[0].dismissed);
        assert_eq!(
            persisted.claims[0].claim.evidence_quote,
            "Mara found the silver key."
        );
        assert_eq!(app.load_scene_text(scene).unwrap().0, source);
        assert!(app.approve_extraction_claim(&persisted, 0).is_err());
        assert!(
            app.list_extraction_proposal_files()
                .unwrap()
                .contains(&path)
        );

        proposal.claims[0].approved = true;
        assert!(app.dismiss_extraction_claim(&mut proposal, 0).is_err());
    }

    #[test]
    fn character_profile_use_cases_validate_and_persist_fields_and_aliases() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let id = app.create_character("Mara Venn").unwrap();
        app.set_character_field(id, "preferred_name", "Mara")
            .unwrap();
        app.set_character_field(id, "goals", "Find the atlas")
            .unwrap();
        app.add_character_alias(id, "The Cartographer").unwrap();
        assert!(app.add_character_alias(id, "the cartographer").is_err());
        app.remove_character_alias(id, "THE CARTOGRAPHER").unwrap();
        assert!(app.remove_character_alias(id, "unknown").is_err());
        assert!(app.set_character_field(id, "unknown", "value").is_err());
        let character = app.get_character(id).unwrap();
        assert_eq!(character.preferred_name, "Mara");
        assert_eq!(character.goals, "Find the atlas");
        assert!(character.aliases.is_empty());
        drop(app);
        let app = WriterApp::open_project(dir.path().join("Novel")).unwrap();
        assert_eq!(app.get_character(id).unwrap(), character);
    }

    #[test]
    fn planning_board_svg_exports_cards_connectors_and_escaped_text() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Board"), "Board").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let first_scene = app.create_scene(chapter, "First", 0).unwrap();
        let second_scene = app.create_scene(chapter, "Second", 1).unwrap();
        let board = app.create_planning_board("<Idea & map>\u{1}").unwrap();
        let first = app
            .add_planning_board_item(board, "scene", &first_scene.to_string(), 0.0, 0.0, "A & B")
            .unwrap();
        let second = app
            .add_planning_board_item(
                board,
                "scene",
                &second_scene.to_string(),
                42.0,
                0.0,
                "Ending",
            )
            .unwrap();
        app.connect_planning_board_items(board, first, second, "<leads>")
            .unwrap();
        let svg = app.planning_board_svg(board).unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("&lt;Idea &amp; map&gt;\u{fffd}"));
        assert!(!svg.contains('\u{1}'));
        assert!(svg.contains("A &amp; B"));
        assert!(svg.contains("&lt;leads&gt;"));
        assert!(svg.contains("marker-end=\"url(#arrow)\""));
    }

    #[test]
    fn confirmed_mention_appearances_are_cross_scene_and_do_not_create_presence() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Mentions"), "Mentions").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let first = app.create_scene(chapter, "First", 0).unwrap();
        let second = app.create_scene(chapter, "Second", 1).unwrap();
        let object = app
            .create_story_object("Silver Key", "A small key.")
            .unwrap();
        for (scene, text) in [
            (first, "Mara found the Silver Key."),
            (second, "The Silver Key was missing."),
        ] {
            let (_, hash) = app.load_scene_text(scene).unwrap();
            app.save_scene_text(scene, Some(&hash), text).unwrap();
            let records = app.list_scene_mentions(scene).unwrap();
            let mention = records
                .iter()
                .find(|mention| mention.quote == "Silver Key")
                .unwrap();
            app.confirm_scene_mention(mention.id, MentionEntityKind::Object, &object.to_string())
                .unwrap();
        }
        let appearances = app
            .confirmed_entity_mention_appearances(MentionEntityKind::Object, &object.to_string())
            .unwrap();
        assert_eq!(
            appearances
                .iter()
                .map(|item| item.scene_id)
                .collect::<Vec<_>>(),
            [first, second]
        );
        let first_analysis = app
            .analytics_dataset()
            .unwrap()
            .into_iter()
            .find(|row| row.scene_id == first.to_string())
            .unwrap();
        assert!(first_analysis.characters.is_empty());
    }

    #[test]
    fn assembly_compile_uses_alternate_order_and_exclusion_without_mutating_canonical_order() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let first = app.create_scene(chapter, "First", 0).unwrap();
        let second = app.create_scene(chapter, "Second", 1).unwrap();
        for (scene, content) in [
            (first, "First canonical prose."),
            (second, "Second canonical prose."),
        ] {
            let (_, hash) = app.load_scene_text(scene).unwrap();
            app.save_scene_text(scene, Some(&hash), content).unwrap();
        }
        let assembly = app.create_assembly("Audio Edition").unwrap();
        let front_matter = app
            .create_content_unit(ContentUnitKind::FrontMatter, "Dedication", None, 0, None)
            .unwrap();
        let (_, front_hash) = app.load_content_unit_text(front_matter).unwrap();
        app.save_content_unit_text(front_matter, Some(&front_hash), "For the readers.")
            .unwrap();
        app.add_assembly_content_unit(assembly, front_matter, 0)
            .unwrap();
        let first_item = app.add_assembly_scene(assembly, first, 1).unwrap();
        let second_item = app.add_assembly_scene(assembly, second, 2).unwrap();
        app.move_assembly_item(assembly, second_item, 1).unwrap();
        app.set_assembly_item_included(first_item, false).unwrap();
        let ir = app.compile_assembly_ir(assembly).unwrap();
        let chapter_id = chapter.to_string();
        assert_eq!(ir.groups.len(), 1);
        assert_eq!(ir.groups[0].id, chapter_id);
        assert!(ir.units.iter().any(|unit| {
            unit.id == second_item.to_string()
                && unit.parent_id.as_deref() == Some(chapter_id.as_str())
        }));
        assert!(!ir.units.iter().any(|unit| {
            unit.segments
                .iter()
                .any(|segment| segment.document.source.contains("First canonical prose."))
        }));
        let compiled = app.compile_assembly_markdown(assembly).unwrap();
        assert!(compiled.contains("Audio Edition"));
        assert!(compiled.contains("For the readers."));
        assert!(compiled.contains("Second canonical prose."));
        assert!(!compiled.contains("First canonical prose."));
        assert!(
            compiled.find("For the readers.").unwrap()
                < compiled.find("Second canonical prose.").unwrap()
        );
        assert_eq!(
            app.list_scenes(false)
                .unwrap()
                .into_iter()
                .map(|scene| scene.scene.id)
                .collect::<Vec<_>>(),
            [first, second]
        );

        let recipe = story_document::EditionRecipe {
            version: 1,
            name: "Audio Edition".into(),
            format: story_document::CompileFormat::Markdown,
            assembly_id: Some(assembly.to_string()),
            output: "exports/compile/audio/manuscript.md".into(),
            metadata_title: None,
            metadata_author: None,
            cover_image: None,
            design: story_document::BookDesign::Manuscript,
            expand_transclusions: false,
            spellcheck: false,
            language: None,
            dictionary: None,
            spelling_engine: story_document::SpellcheckEngine::default(),
            diagnostics: story_document::DiagnosticPolicy::default(),
        };
        app.create_compile_configuration("audio", &recipe).unwrap();
        app.compile_configuration("audio", false).unwrap();
        let artifact =
            fs::read_to_string(dir.path().join("Novel/exports/compile/audio/manuscript.md"))
                .unwrap();
        assert!(artifact.contains("Second canonical prose."));
        assert!(!artifact.contains("First canonical prose."));
    }
}
