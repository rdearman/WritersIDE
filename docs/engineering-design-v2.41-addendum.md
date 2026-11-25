# Engineering Design v2.41 Addendum — Preserve Empty Project Directories

## Backup archive contract

Portable project archives must preserve the project directory structure as well
as regular-file content. The archive manifest records normalized relative file
hashes and explicit relative directory entries, including empty directories.
Restore validates both sets before publishing the staged project. It rejects
unsafe paths, duplicate directory records, special files and directory/file
type mismatches. Older format-version-1 archives without a `directories` field
remain readable; the field defaults to an empty list and parent directories are
created as needed for their files.

This applies to Save As and whole-project restore because both use the same
verified archive mechanism. No external paths or symlinks are followed.

## Verification

- Archive/restore tests preserve empty and nested directories under attachments,
  templates, exports and research.
- The terminal Save As workflow verifies empty author-managed directories and
  existing asset files in the independent copy.
- Recovery must continue to stage and validate the entire restored project
  before activation.
