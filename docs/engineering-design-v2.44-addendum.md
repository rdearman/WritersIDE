# Engineering Design v2.42 Addendum — Staged Projects and Portable Archives

## Backup archive contract

Portable project archives must preserve the project directory structure as well
as regular-file content. The version-2 archive manifest records portable
relative file hashes and explicit relative directory entries, including empty
directories. Non-UTF-8 Unix names and literal backslashes are reversibly
encoded as path components instead of being converted lossily into colliding
names. Restore validates both file and directory sets before publishing the
staged project. It rejects unsafe paths, duplicate directory records, special
files and directory/file type mismatches. Existing version-1 archives remain
readable without interpreting the version-2 escape convention.

## Persistent temporary working projects

New projects and standalone project imports are created in a unique
application-managed staging directory. The project uses the ordinary SQLite,
manuscript, asset, journal and recovery layout; it is not an in-memory or
temporary-only model. On Unix, WIDE creates private staging directories with
mode `0700`. The root follows platform application-data conventions (XDG data
home on Linux, Application Support on macOS and Local AppData on Windows).

Project creation and import open the normal editing workspace without asking
for a permanent path. Compile and autosave continue to use the staged project.
The first explicit Save or Save As invokes the normal destination browser and
creates a complete independent copy. The copy is built and validated in a
temporary directory on the destination filesystem, then published by rename;
the staged source remains active if any step fails.

Before copying, WIDE atomically records a promotion intent containing the
destination and immutable project identity. On restart, if the destination
exists and has the same project identity, WIDE treats the promotion as
completed and removes the redundant stage. If the destination is absent, it
clears the intent and resumes the stage. If identities differ or either copy
cannot be verified, it preserves both and reports a conflict. Abandoned,
unmarked creation/import transaction directories are cleaned only when they
match WIDE's generated staging names; other directories are left untouched.

This applies to Save As and whole-project restore because both use the same
verified archive mechanism. No external paths or symlinks are followed.

## Verification

- Archive/restore tests preserve empty and nested directories under attachments,
  templates, exports and research, including Unix names that cannot be
  represented as UTF-8 and path components containing a literal backslash.
- The terminal Save As workflow verifies empty author-managed directories and
  existing asset files in the independent copy.
- Recovery tests cover interrupted promotion, project identity conflicts and
  staged-project recovery after an abrupt PTY termination.
