# Scrivener Starter fixture

These are unmodified real Scrivener projects from the `robinsonkw/scrivener-test`
repository, upstream commit `768a925c774221cdf6a7809fb728ffdb9239edf1`:

<https://github.com/robinsonkw/scrivener-test/tree/768a925c774221cdf6a7809fb728ffdb9239edf1/test.scriv>

The upstream repository provides the project under Apache License 2.0. The
license and upstream NOTICE are retained alongside the fixture. Upstream
metadata identifies the project as created with Scrivener for Mac 3.1.4
(`Creator="SCRMAC-3.1.4-12105"`).

The project is a small fixture with a Draft folder, two RTF text documents, a
Research folder and Trash. Its expected document text, after paragraph
normalisation, is:

1. `Test` / `This is an addition to the old file.`
2. `And this is a new file test`

`test Backup.scriv` is a separate project whose metadata identifies Scrivener
for Mac 2.5 (`Creator="SCRMAC-2.5-25239"`, manifest revision `1.0`). It has
Draft, Research, and Trash binder folders but no document RTF files; it is used
to verify version reporting and explicit unsupported-content handling, not to
claim Scrivener 2 import support.

`test.scriv` supplies real-format end-to-end import evidence for this specific
Scrivener 3.1.4 Mac project shape. It does not establish broad Scrivener
compatibility or cover synopsis, notes, Unicode prose, inline emphasis,
excluded binder items with content, or a substantial multi-chapter project.
