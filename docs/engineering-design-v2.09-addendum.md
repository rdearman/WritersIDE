# Engineering Design v2.09 Addendum — External text and Org import verification

This addendum records the automated workflow coverage for GitHub Issue #57.

**Open Text File** displays a UTF-8 source in a read-only reference pane. The author can mark the start of a passage with `v`, move by line, and copy the selected lines through the application's internal clipboard with Ctrl+C, then paste into the manuscript with Ctrl+V. This is the documented fallback when terminal system-clipboard integration is unavailable. Opening and copying do not modify the external file.

**Import Document** previews plain text, Markdown and Org-mode before applying it to the current project. Org imports can be switched between one document and heading hierarchy. Hierarchy import creates chapters/scenes from headings, preserves prose order, and lists unsupported constructs for review. Import provenance records the source fingerprint; the original source is left unchanged.

Regression coverage exercises the read-only copy/paste path and the Org preview-to-import workflow, including chapter/scene mapping, saved/reopened text and byte-for-byte source preservation. Real-author workflow acceptance on a representative Org file remains outstanding, so Issue #57 remains open.
