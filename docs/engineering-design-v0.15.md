# Writer's IDE — Engineering Design v0.15

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.14

## Terminal spellcheck workflow

The terminal command `spellcheck LANGUAGE [PROJECT-WORDLIST] [ASPELL]` invokes a local Aspell executable against scene and chapter-level content units from the current project projection. The optional dictionary path must resolve to a regular file inside the canonical project directory; symlink/path escape is rejected. Project word lists use the existing plain, Aspell, and Hunspell header parser. Findings are shown by content unit with candidate counts. The command is read-only and never rewrites manuscript files. Missing Aspell, invalid configuration, or a failed subprocess returns an explicit command error.
