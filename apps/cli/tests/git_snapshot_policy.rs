use std::{fs, process::Command};

#[test]
fn cli_configures_opt_in_exit_snapshot_policy() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("Novel");
    let executable = env!("CARGO_BIN_EXE_writers-ide");
    let created = Command::new(executable)
        .args(["create", project.to_str().unwrap(), "Novel"])
        .output()
        .unwrap();
    assert!(created.status.success());

    for (setting, enabled) in [("on", true), ("off", false)] {
        let output = Command::new(executable)
            .args(["git-auto-snapshot-exit", project.to_str().unwrap(), setting])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let policy: toml::Value = toml::from_str(
            &fs::read_to_string(project.join(".writers-ide/git-snapshot.toml")).unwrap(),
        )
        .unwrap();
        assert_eq!(policy["snapshot_on_exit"].as_bool(), Some(enabled));
    }
}
