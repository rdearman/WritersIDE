# Writer's IDE — Engineering Design v0.13

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.12

## Selective prose restoration from Git history

`WriterApp::restore_scene_from_git(commit, scene_id)` restores only the stable scene's Markdown text. The scene's current relative content path is resolved from SQLite, then read through the project-local `story-git` API, which validates the commit and confines reads to application-owned project paths. Invalid UTF-8 and absent historical files are rejected.

If the historical prose differs from the current prose, the application first creates a consistent Git snapshot of the current project, then saves the historical text using the current SHA-256 as the optimistic expected hash. The existing durable manuscript journal performs replacement and metadata/hash/index updates. Concurrent or external changes therefore follow the existing conflict path, and the pre-restore Git snapshot retains the prior version. Identical historical prose is a no-op.

CLI `git-restore-scene PROJECT COMMIT SCENE-UUID` and the terminal command registry expose the operation. The terminal saves its active buffer first and reloads the restored scene if it is open. This operation restores prose only; it does not rewrite scene metadata, structure, or relationships. Whole-project restore remains restore-to-new-directory. Selective restoration of historical structural/database state and automatic interval snapshots remain open Issue #11 work.
