//! Versioned, application-wide onboarding preferences.
//!
//! Tutorial state intentionally lives outside project databases so opening a
//! new book does not restart first-run onboarding and so it cannot mutate a
//! manuscript project.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, io, path::Path};

pub const PREFERENCES_VERSION: u16 = 1;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub version: u16,
    pub welcome_seen: bool,
    pub tutorial_dismissed: bool,
    pub tutorial_completed: bool,
    pub active_tutorial: Option<TutorialProgress>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TutorialProgress {
    pub track: TutorialTrack,
    pub step: usize,
    pub demo_project: String,
    pub return_project: String,
    /// Stable sample scene restored when a resumed lesson requires the editor.
    #[serde(default)]
    pub demo_scene: Option<String>,
    /// Verified user actions for the current tutorial route. Older preference
    /// files deserialize with an empty set so progress remains resumable.
    #[serde(default)]
    pub verified_actions: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TutorialTrack {
    GuidedTour,
    QuickStart,
}

#[derive(Debug)]
pub enum PreferenceError {
    Io(io::Error),
    Json(serde_json::Error),
    UnsupportedVersion(u16),
}

impl std::fmt::Display for PreferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported tutorial-preferences version {version}")
            }
        }
    }
}

impl std::error::Error for PreferenceError {}

impl From<io::Error> for PreferenceError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for PreferenceError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

pub fn load(path: &Path) -> Result<Preferences, PreferenceError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Preferences {
                version: PREFERENCES_VERSION,
                ..Preferences::default()
            });
        }
        Err(error) => return Err(error.into()),
    };
    let mut preferences: Preferences = serde_json::from_slice(&bytes)?;
    if preferences.version != PREFERENCES_VERSION {
        return Err(PreferenceError::UnsupportedVersion(preferences.version));
    }
    preferences.version = PREFERENCES_VERSION;
    Ok(preferences)
}

/// Persist preferences with a same-directory staged replacement and directory
/// sync. The user's manuscript project is never involved in this write.
pub fn store(path: &Path, preferences: &Preferences) -> Result<(), PreferenceError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let bytes = serde_json::to_vec_pretty(preferences)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    use io::Write;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_launch_defaults_to_unseen_and_can_be_persisted() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tutorial-state.json");
        let mut preferences = load(&path).unwrap();
        assert!(!preferences.welcome_seen);
        assert_eq!(preferences.version, PREFERENCES_VERSION);
        preferences.welcome_seen = true;
        preferences.active_tutorial = Some(TutorialProgress {
            track: TutorialTrack::QuickStart,
            step: 1,
            demo_project: "demo".into(),
            return_project: "book".into(),
            demo_scene: None,
            verified_actions: BTreeSet::new(),
        });
        store(&path, &preferences).unwrap();
        assert_eq!(load(&path).unwrap(), preferences);
    }

    #[test]
    fn unsupported_preferences_version_is_not_silently_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tutorial-state.json");
        fs::write(&path, r#"{"version":99}"#).unwrap();
        assert!(matches!(
            load(&path),
            Err(PreferenceError::UnsupportedVersion(99))
        ));
    }

    #[test]
    fn tutorial_progress_is_global_and_serializable() {
        let mut preferences = Preferences {
            version: PREFERENCES_VERSION,
            welcome_seen: true,
            tutorial_dismissed: false,
            tutorial_completed: false,
            active_tutorial: Some(TutorialProgress {
                track: TutorialTrack::GuidedTour,
                step: 3,
                demo_project: "/tutorial/demo".into(),
                return_project: "/books/novel".into(),
                demo_scene: None,
                verified_actions: BTreeSet::new(),
            }),
        };
        preferences
            .active_tutorial
            .as_mut()
            .unwrap()
            .verified_actions
            .insert("return".into());
        let json = serde_json::to_string(&preferences).unwrap();
        assert!(json.contains("guided_tour"));
        assert!(json.contains("/books/novel"));
        assert!(load_progress_from_json(&json));
    }

    fn load_progress_from_json(json: &str) -> bool {
        serde_json::from_str::<Preferences>(json)
            .ok()
            .and_then(|preferences| preferences.active_tutorial)
            .is_some_and(|progress| progress.verified_actions.contains("return"))
    }

    #[test]
    fn older_tutorial_preferences_default_to_no_verified_actions() {
        let json = r#"{"version":1,"welcome_seen":true,"active_tutorial":{"track":"guided_tour","step":0,"demo_project":"demo","return_project":"book"}}"#;
        let preferences: Preferences = serde_json::from_str(json).unwrap();
        assert!(
            preferences
                .active_tutorial
                .unwrap()
                .verified_actions
                .is_empty()
        );
    }
}
