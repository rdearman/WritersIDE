# Writer's IDE — Engineering Design v0.5

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.4  
**Migration level:** 4

This revision adds scene snapshots and revision operations. It retains the Phase 1 write journal as the only path for changing canonical manuscript files.

## Snapshot storage and contracts

Migration 0004 adds `scene_snapshot` metadata. Snapshot bytes live in `.writers-ide/snapshots/<scene-uuid>/<snapshot-uuid>.md`; every path is project-relative. A snapshot is a SHA-256-verified copy of the scene text at a point in time. Snapshot creation first loads through the normal external-change check, then writes and synchronizes a sibling temporary before inserting the database record. An orphaned snapshot file after an interrupted metadata insertion is harmless; authoritative manuscript content is not modified.

The snapshot listing is per scene and ordered newest-first. `load_snapshot` verifies the stored hash and rejects a tampered copy. Diff returns Equal/Added/Removed lines using a bounded LCS calculation; very large line products fall back to a valid, coarser prefix-and-replacement diff to bound memory.

Full restoration checks that the snapshot belongs to the requested scene, creates an automatic pre-restore snapshot, and saves the replacement through the existing expected-hash manuscript journal. A stale expected hash or external edit prevents replacement. Selective restore accepts explicit current and snapshot line ranges, validates both, snapshots the current state, merges those lines, and saves through the same journal. This is an author-directed hunk operation; the UI must present the diff and ranges before invoking it.

Revision files are ordinary UTF-8 Markdown under `.writers-ide/`, suitable for project backups and local history. Snapshot metadata stays in SQLite; manuscript saves remain in `manuscript/`. Restore does not rename or duplicate a scene and preserves stable scene IDs, relationships, and chronology.

## Application/API surface

The storage/application API provides create/list/load snapshot, compare snapshot, full restore, and selective line restore. The CLI exposes snapshot/history/diff/restore. The terminal Revisions workspace lists snapshots and previews a selected diff; command-palette operations create snapshots and restore by UUID. Git remains usable for the project files; snapshots are application-managed history rather than a Git wrapper.

## Failure handling

- A failed or stale save leaves existing scene text unchanged and retains the pre-restore snapshot.
- A tampered snapshot produces an explicit conflict and cannot be restored.
- A snapshot from another scene is rejected.
- Invalid line ranges and stale current hashes reject selective restore.
- A crash during restore is recovered by the existing write journal; snapshot creation itself cannot corrupt the canonical manuscript.
- Large diffs may show a broad replacement hunk rather than a minimal edit script, without changing either version.

## Acceptance tests

1. Snapshot text and metadata survive close/reopen and relocation.
2. A modified scene produces Added/Removed diff lines against its snapshot.
3. Selective line restore replaces only the chosen range and produces a pre-restore snapshot.
4. Full restore replaces scene content, creates a safety snapshot, and advances through the manuscript write journal.
5. Wrong-scene IDs, tampered snapshot hashes, invalid ranges and stale expected hashes are rejected.
6. Existing crash-injection and recovery tests continue to pass during restores.

## Remaining decisions

- Snapshot retention and deletion policy.
- Richer diff algorithms for very large scenes and formatting-aware Markdown diffs.
- A visual hunk-selection flow for selective restore.
- Whether periodic or pre-save snapshots should be configurable.
- Git change/snapshot cross-referencing without making Git mandatory.
