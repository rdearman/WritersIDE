use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
use story_core::{BookId, ProjectId, SeriesId};
use tempfile::NamedTempFile;
use thiserror::Error;

const SERIES_SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Error)]
pub enum SeriesError {
    #[error("series I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("series database: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("series configuration: {0}")]
    Config(String),
    #[error("series identity mismatch: manifest {manifest}, database {database}")]
    IdentityMismatch {
        manifest: SeriesId,
        database: SeriesId,
    },
    #[error("unsupported series schema version {0}")]
    UnsupportedSchema(i64),
    #[error("book directory must be located inside the series folder")]
    BookOutsideSeries,
    #[error("invalid project-relative book path: {0}")]
    InvalidBookPath(String),
    #[error("book membership metadata conflicts with the series database")]
    MembershipConflict,
    #[error("book project identity does not match the supplied stable project ID")]
    BookIdentityMismatch,
}

pub type Result<T> = std::result::Result<T, SeriesError>;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SeriesManifest {
    pub format_version: u32,
    pub id: SeriesId,
    pub title: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct BookMembership {
    pub format_version: u32,
    pub series_id: SeriesId,
    pub book_id: BookId,
    pub series_root_relative_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SeriesBook {
    pub id: BookId,
    pub project_id: ProjectId,
    pub title: String,
    pub relative_path: String,
    pub position: u32,
    pub source_fingerprint: Option<String>,
}

pub struct SeriesStore {
    root: PathBuf,
    manifest: SeriesManifest,
    conn: Connection,
}

impl SeriesStore {
    pub fn create(root: impl AsRef<Path>, title: &str) -> Result<Self> {
        let root = root.as_ref();
        let title = title.trim();
        if title.is_empty() {
            return Err(SeriesError::Config("series title cannot be empty".into()));
        }
        if root.exists() {
            return Err(SeriesError::Config(format!(
                "series destination already exists: {}",
                root.display()
            )));
        }
        let parent = root
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let staging = tempfile::tempdir_in(parent)?;
        let staged_root = staging.path().join("series");
        fs::create_dir(&staged_root)?;
        let manifest = SeriesManifest {
            format_version: 1,
            id: SeriesId::new(),
            title: title.to_owned(),
        };
        write_toml_atomically(&staged_root.join("series.toml"), &manifest)?;
        let conn = Connection::open(staged_root.join("series.db"))?;
        configure(&conn)?;
        conn.execute_batch(
            "BEGIN IMMEDIATE;
                 CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);
                 CREATE TABLE series_metadata(
                    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
                    id TEXT NOT NULL UNIQUE,
                    title TEXT NOT NULL,
                    schema_version INTEGER NOT NULL
                 );
                 CREATE TABLE series_book(
                    id TEXT PRIMARY KEY,
                    project_id TEXT NOT NULL UNIQUE,
                    title TEXT NOT NULL,
                    relative_path TEXT NOT NULL UNIQUE,
                    position INTEGER NOT NULL UNIQUE CHECK(position>=0),
                    source_fingerprint TEXT,
                    created_at INTEGER NOT NULL
                 );
                 CREATE UNIQUE INDEX series_book_title_unique
                    ON series_book(title COLLATE NOCASE);
                 INSERT INTO schema_migrations(version,applied_at)
                    VALUES(1,unixepoch()),(2,unixepoch());",
        )?;
        conn.execute(
            "INSERT INTO series_metadata(singleton,id,title,schema_version) VALUES(1,?1,?2,?3)",
            params![
                manifest.id.to_string(),
                manifest.title,
                SERIES_SCHEMA_VERSION
            ],
        )?;
        conn.execute_batch("COMMIT;")?;
        drop(conn);
        fs::File::open(&staged_root)?.sync_all()?;
        fs::rename(&staged_root, root)?;
        fs::File::open(parent)?.sync_all()?;
        drop(staging);
        Self::open(root)
    }

    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        let manifest_text = fs::read_to_string(root.join("series.toml"))?;
        let manifest: SeriesManifest = toml::from_str(&manifest_text)
            .map_err(|error| SeriesError::Config(error.to_string()))?;
        if manifest.format_version != 1 {
            return Err(SeriesError::UnsupportedSchema(i64::from(
                manifest.format_version,
            )));
        }
        let mut conn = Connection::open(root.join("series.db"))?;
        configure(&conn)?;
        let (database_id, database_title, schema_version): (String, String, i64) = conn.query_row(
            "SELECT id,title,schema_version FROM series_metadata WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if schema_version > SERIES_SCHEMA_VERSION {
            return Err(SeriesError::UnsupportedSchema(schema_version));
        }
        if schema_version < 1 {
            return Err(SeriesError::UnsupportedSchema(schema_version));
        }
        if schema_version < SERIES_SCHEMA_VERSION {
            migrate_series_schema(&mut conn, schema_version)?;
        }
        let database_id = parse_id::<SeriesId>(&database_id)?;
        if database_id != manifest.id {
            return Err(SeriesError::IdentityMismatch {
                manifest: manifest.id,
                database: database_id,
            });
        }
        if database_title != manifest.title {
            return Err(SeriesError::Config(
                "series title differs between manifest and database".into(),
            ));
        }
        Ok(Self {
            root: root.to_path_buf(),
            manifest,
            conn,
        })
    }

    pub fn id(&self) -> SeriesId {
        self.manifest.id
    }
    pub fn title(&self) -> &str {
        &self.manifest.title
    }
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn add_book(
        &mut self,
        project_id: ProjectId,
        book_path: impl AsRef<Path>,
        title: &str,
        source_fingerprint: Option<&str>,
    ) -> Result<SeriesBook> {
        let title = title.trim();
        if title.is_empty() {
            return Err(SeriesError::Config("book title cannot be empty".into()));
        }
        let project = story_storage::ProjectStore::open(book_path.as_ref())
            .map_err(|error| SeriesError::Config(format!("invalid book project: {error}")))?;
        if project
            .get_project()
            .map_err(|error| SeriesError::Config(format!("cannot read book project: {error}")))?
            .id
            != project_id
        {
            return Err(SeriesError::BookIdentityMismatch);
        }
        drop(project);
        let relative_path = self.relative_book_path(book_path.as_ref())?;
        if let Some(fingerprint) = source_fingerprint
            && (fingerprint.len() != 64
                || !fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err(SeriesError::Config(
                "source fingerprint must be a SHA-256 hex digest".into(),
            ));
        }
        if let Some(existing) = self.book_for_project(project_id)? {
            if existing.relative_path != relative_path {
                return Err(SeriesError::Config(format!(
                    "project {project_id} is already a member at {}",
                    existing.relative_path
                )));
            }
            self.write_membership(&existing)?;
            return Ok(existing);
        }
        if self
            .list_books()?
            .iter()
            .any(|book| book.title.eq_ignore_ascii_case(title))
        {
            return Err(SeriesError::Config(
                "a book with that name already exists in this series".into(),
            ));
        }
        let position: u32 = self
            .conn
            .query_row("SELECT count(*) FROM series_book", [], |row| row.get(0))?;
        let book = SeriesBook {
            id: BookId::new(),
            project_id,
            title: title.to_owned(),
            relative_path,
            position,
            source_fingerprint: source_fingerprint.map(str::to_owned),
        };
        let transaction = self.conn.transaction()?;
        transaction.execute(
            "INSERT INTO series_book(id,project_id,title,relative_path,position,source_fingerprint,created_at) VALUES(?1,?2,?3,?4,?5,?6,unixepoch())",
            params![book.id.to_string(), book.project_id.to_string(), book.title, book.relative_path, book.position, book.source_fingerprint],
        )?;
        transaction.commit()?;
        // The database is authoritative. If a crash occurs before this sidecar
        // is written, retrying add_book or repair_memberships recreates it.
        self.write_membership(&book)?;
        Ok(book)
    }

    pub fn list_books(&self) -> Result<Vec<SeriesBook>> {
        let mut statement = self.conn.prepare(
            "SELECT id,project_id,title,relative_path,position,source_fingerprint FROM series_book ORDER BY position,id"
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;
        rows.map(|row| {
            let (id, project, title, relative_path, position, source_fingerprint) = row?;
            Ok::<SeriesBook, rusqlite::Error>(SeriesBook {
                id: parse_id(&id).map_err(to_sql_conversion_error)?,
                project_id: parse_id(&project).map_err(to_sql_conversion_error)?,
                title,
                relative_path,
                position,
                source_fingerprint,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(SeriesError::from)
    }

    pub fn book_for_project(&self, project_id: ProjectId) -> Result<Option<SeriesBook>> {
        let mut statement = self.conn.prepare(
            "SELECT id,project_id,title,relative_path,position,source_fingerprint FROM series_book WHERE project_id=?1"
        )?;
        statement
            .query_row([project_id.to_string()], |row| {
                let id: String = row.get(0)?;
                let project: String = row.get(1)?;
                Ok(SeriesBook {
                    id: parse_id(&id).map_err(to_sql_conversion_error)?,
                    project_id: parse_id(&project).map_err(to_sql_conversion_error)?,
                    title: row.get(2)?,
                    relative_path: row.get(3)?,
                    position: row.get(4)?,
                    source_fingerprint: row.get(5)?,
                })
            })
            .optional()
            .map_err(SeriesError::from)
    }

    pub fn repair_memberships(&self) -> Result<Vec<PathBuf>> {
        let mut repaired = Vec::new();
        for book in self.list_books()? {
            let path = self.root.join(&book.relative_path);
            if path.is_dir() {
                self.write_membership(&book)?;
                repaired.push(path);
            }
        }
        Ok(repaired)
    }

    pub fn open_for_book(book_root: impl AsRef<Path>) -> Result<(Self, SeriesBook)> {
        let book_root = book_root.as_ref();
        let membership_path = book_root.join(".writers-ide/series-membership.toml");
        let membership: BookMembership = toml::from_str(&fs::read_to_string(&membership_path)?)
            .map_err(|error| SeriesError::Config(error.to_string()))?;
        if membership.format_version != 1 {
            return Err(SeriesError::UnsupportedSchema(i64::from(
                membership.format_version,
            )));
        }
        let relative_root = Path::new(&membership.series_root_relative_path);
        if relative_root.is_absolute()
            || relative_root
                .components()
                .any(|component| !matches!(component, Component::Normal(_) | Component::ParentDir))
        {
            return Err(SeriesError::InvalidBookPath(
                membership.series_root_relative_path,
            ));
        }
        let series_root = book_root.join(relative_root);
        let series = Self::open(&series_root)?;
        if series.id() != membership.series_id {
            return Err(SeriesError::IdentityMismatch {
                manifest: membership.series_id,
                database: series.id(),
            });
        }
        let project = story_storage::ProjectStore::open(book_root)
            .map_err(|error| SeriesError::Config(format!("invalid book project: {error}")))?;
        let project_id = project
            .get_project()
            .map_err(|error| SeriesError::Config(format!("cannot read book project: {error}")))?
            .id;
        drop(project);
        let book = series
            .list_books()?
            .into_iter()
            .find(|book| book.id == membership.book_id && book.project_id == project_id)
            .ok_or(SeriesError::MembershipConflict)?;
        if series.relative_book_path(book_root)? != book.relative_path {
            return Err(SeriesError::MembershipConflict);
        }
        Ok((series, book))
    }

    fn relative_book_path(&self, book_path: &Path) -> Result<String> {
        let root = self.root.canonicalize()?;
        let book = book_path.canonicalize()?;
        let relative = book
            .strip_prefix(&root)
            .map_err(|_| SeriesError::BookOutsideSeries)?;
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(SeriesError::InvalidBookPath(relative.display().to_string()));
        }
        Ok(relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"))
    }

    fn write_membership(&self, book: &SeriesBook) -> Result<()> {
        let project_root = self.root.join(&book.relative_path);
        let control = project_root.join(".writers-ide");
        if !control.is_dir() {
            return Err(SeriesError::Config(format!(
                "book project metadata is missing: {}",
                control.display()
            )));
        }
        let path = control.join("series-membership.toml");
        let membership = BookMembership {
            format_version: 1,
            series_id: self.manifest.id,
            book_id: book.id,
            series_root_relative_path: Path::new(&book.relative_path)
                .components()
                .map(|_| "..")
                .collect::<Vec<_>>()
                .join("/"),
        };
        if path.exists() {
            let current: BookMembership = toml::from_str(&fs::read_to_string(&path)?)
                .map_err(|error| SeriesError::Config(error.to_string()))?;
            if current != membership {
                return Err(SeriesError::MembershipConflict);
            }
            return Ok(());
        }
        write_toml_atomically(&path, &membership)
    }
}

fn migrate_series_schema(conn: &mut Connection, current_version: i64) -> Result<()> {
    if current_version != 1 {
        return Err(SeriesError::UnsupportedSchema(current_version));
    }
    let transaction = conn.transaction()?;
    let duplicate: Option<String> = transaction
        .query_row(
            "SELECT title FROM series_book GROUP BY title COLLATE NOCASE HAVING count(*) > 1 LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if duplicate.is_some() {
        return Err(SeriesError::Config(
            "existing series contains duplicate book names; resolve them before opening".into(),
        ));
    }
    transaction.execute_batch(
        "CREATE UNIQUE INDEX series_book_title_unique ON series_book(title COLLATE NOCASE);
         INSERT INTO schema_migrations(version,applied_at) VALUES(2,unixepoch());
         UPDATE series_metadata SET schema_version=2 WHERE singleton=1;",
    )?;
    transaction.commit()?;
    Ok(())
}

fn configure(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "FULL")?;
    Ok(())
}

fn write_toml_atomically<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| SeriesError::Config("manifest has no parent directory".into()))?;
    let text =
        toml::to_string_pretty(value).map_err(|error| SeriesError::Config(error.to_string()))?;
    let mut file = NamedTempFile::new_in(parent)?;
    use std::io::Write;
    file.write_all(text.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(path)
        .map_err(|error| SeriesError::Io(error.error))?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn parse_id<T>(text: &str) -> Result<T>
where
    T: From<uuid::Uuid>,
{
    let id = uuid::Uuid::parse_str(text).map_err(|error| SeriesError::Config(error.to_string()))?;
    Ok(id.into())
}

fn to_sql_conversion_error(error: SeriesError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book_folder(series_root: &Path, name: &str) -> (PathBuf, ProjectId) {
        let book = series_root.join(name);
        let store = story_storage::ProjectStore::create(&book, name).unwrap();
        let id = store.get_project().unwrap().id;
        drop(store);
        (book, id)
    }

    #[test]
    fn series_identity_books_and_membership_survive_relocation() {
        let temp = tempfile::tempdir().unwrap();
        let series_root = temp.path().join("Chronicles");
        let mut series = SeriesStore::create(&series_root, "The Chronicles").unwrap();
        let series_id = series.id();
        let (path, project_id) = book_folder(&series_root, "Book 1");
        let book = series
            .add_book(project_id, &path, "Book One", Some("a".repeat(64).as_str()))
            .unwrap();
        assert_eq!(book.position, 0);
        assert_eq!(book.relative_path, "Book 1");
        let membership: BookMembership = toml::from_str(
            &fs::read_to_string(path.join(".writers-ide/series-membership.toml")).unwrap(),
        )
        .unwrap();
        assert_eq!(membership.series_id, series_id);
        assert_eq!(membership.book_id, book.id);
        assert_eq!(membership.series_root_relative_path, "..");
        let (by_book, current_book) = SeriesStore::open_for_book(&path).unwrap();
        assert_eq!(by_book.id(), series_id);
        assert_eq!(current_book.id, book.id);
        assert_eq!(
            series.book_for_project(project_id).unwrap(),
            Some(book.clone())
        );
        drop(series);

        let relocated = temp.path().join("On USB/Chronicles");
        fs::create_dir_all(relocated.parent().unwrap()).unwrap();
        fs::rename(&series_root, &relocated).unwrap();
        let series = SeriesStore::open(&relocated).unwrap();
        assert_eq!(series.id(), series_id);
        assert_eq!(series.list_books().unwrap(), vec![book]);
        assert!(
            relocated
                .join("Book 1/.writers-ide/series-membership.toml")
                .is_file()
        );
        assert!(SeriesStore::open_for_book(relocated.join("Book 1")).is_ok());
    }

    #[test]
    fn duplicate_book_names_are_rejected_by_services_and_sql_constraints() {
        let temp = tempfile::tempdir().unwrap();
        let series_root = temp.path().join("Series");
        let mut series = SeriesStore::create(&series_root, "Series").unwrap();
        let (first_path, first_id) = book_folder(&series_root, "Book 1");
        series
            .add_book(first_id, &first_path, "The First Book", None)
            .unwrap();
        let (second_path, second_id) = book_folder(&series_root, "Book 2");
        assert!(matches!(
            series.add_book(second_id, &second_path, "the first book", None),
            Err(SeriesError::Config(message)) if message.contains("already exists")
        ));

        let connection = Connection::open(series_root.join("series.db")).unwrap();
        let direct_insert = connection.execute(
            "INSERT INTO series_book(id,project_id,title,relative_path,position,source_fingerprint,created_at)
             VALUES(?1,?2,?3,?4,?5,NULL,unixepoch())",
            params![
                uuid::Uuid::new_v4().to_string(),
                uuid::Uuid::new_v4().to_string(),
                "THE FIRST BOOK",
                "Book 2",
                1
            ],
        );
        assert!(direct_insert.is_err());
    }

    #[test]
    fn version_one_series_database_migrates_title_uniqueness() {
        let temp = tempfile::tempdir().unwrap();
        let series_root = temp.path().join("Series");
        drop(SeriesStore::create(&series_root, "Series").unwrap());
        let database_path = series_root.join("series.db");
        let connection = Connection::open(&database_path).unwrap();
        connection
            .execute_batch(
                "DROP INDEX series_book_title_unique;
                 DELETE FROM schema_migrations WHERE version=2;
                 UPDATE series_metadata SET schema_version=1;",
            )
            .unwrap();
        drop(connection);

        let series = SeriesStore::open(&series_root).unwrap();
        let schema_version: i64 = series
            .conn
            .query_row(
                "SELECT schema_version FROM series_metadata WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(schema_version, 2);
        let migrations: i64 = series
            .conn
            .query_row(
                "SELECT count(*) FROM schema_migrations WHERE version=2",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(migrations, 1);
    }

    #[test]
    fn version_one_series_with_duplicate_names_is_left_unmodified() {
        let temp = tempfile::tempdir().unwrap();
        let series_root = temp.path().join("Series");
        drop(SeriesStore::create(&series_root, "Series").unwrap());
        let database_path = series_root.join("series.db");
        let connection = Connection::open(&database_path).unwrap();
        connection
            .execute_batch(
                "DROP INDEX series_book_title_unique;
                 DELETE FROM schema_migrations WHERE version=2;
                 UPDATE series_metadata SET schema_version=1;",
            )
            .unwrap();
        for (id, project_id, path) in [
            ("book-1", "project-1", "Book 1"),
            ("book-2", "project-2", "Book 2"),
        ] {
            connection
                .execute(
                    "INSERT INTO series_book(id,project_id,title,relative_path,position,source_fingerprint,created_at)
                     VALUES(?1,?2,'Same Name',?3,?4,NULL,unixepoch())",
                    params![id, project_id, path, if id == "book-1" { 0 } else { 1 }],
                )
                .unwrap();
        }
        drop(connection);

        assert!(matches!(
            SeriesStore::open(&series_root),
            Err(SeriesError::Config(message)) if message.contains("duplicate book names")
        ));
        let connection = Connection::open(&database_path).unwrap();
        let schema_version: i64 = connection
            .query_row(
                "SELECT schema_version FROM series_metadata WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(schema_version, 1);
        let migration_applied: i64 = connection
            .query_row(
                "SELECT count(*) FROM schema_migrations WHERE version=2",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(migration_applied, 0);
    }

    #[test]
    fn series_rejects_outside_books_bad_fingerprints_and_identity_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        let series_root = temp.path().join("Series");
        let mut series = SeriesStore::create(&series_root, "Series").unwrap();
        let outside = temp.path().join("Outside");
        let outside_store = story_storage::ProjectStore::create(&outside, "Outside").unwrap();
        let outside_id = outside_store.get_project().unwrap().id;
        drop(outside_store);
        assert!(matches!(
            series.add_book(outside_id, outside, "Book", None),
            Err(SeriesError::BookOutsideSeries)
        ));
        let (inside, inside_id) = book_folder(&series_root, "Book");
        assert!(matches!(
            series.add_book(inside_id, &inside, "Book", Some("not-a-hash")),
            Err(SeriesError::Config(_))
        ));
        drop(series);

        let manifest_path = series_root.join("series.toml");
        let mut manifest: SeriesManifest =
            toml::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest.id = SeriesId::new();
        fs::write(&manifest_path, toml::to_string(&manifest).unwrap()).unwrap();
        assert!(matches!(
            SeriesStore::open(&series_root),
            Err(SeriesError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn repeat_association_repairs_missing_membership_and_rejects_conflicting_membership() {
        let temp = tempfile::tempdir().unwrap();
        let series_root = temp.path().join("Series");
        let mut series = SeriesStore::create(&series_root, "Series").unwrap();
        let (book_path, project_id) = book_folder(&series_root, "Book");
        let book = series
            .add_book(project_id, &book_path, "Book", None)
            .unwrap();
        let membership_path = book_path.join(".writers-ide/series-membership.toml");
        fs::remove_file(&membership_path).unwrap();
        assert_eq!(
            series
                .add_book(project_id, &book_path, "Book", None)
                .unwrap(),
            book
        );
        assert!(membership_path.is_file());
        fs::write(&membership_path, "format_version=1\n").unwrap();
        assert!(matches!(
            series.repair_memberships(),
            Err(SeriesError::Config(_))
        ));
    }
}
