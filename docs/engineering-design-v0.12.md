# Writer's IDE — Engineering Design v0.12

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.11

## Processing recipe proofreading stage

The dependency-aware pipeline now exposes `proofread` as a selectable stage. It depends on the compiled manuscript and writes a JSON diagnostic artifact under `.writers-ide/processing/proofread.json`, with UTF-8 byte ranges, category, severity and message for each finding. The stage reads `.writers-ide/diagnostics.toml`; if the file is absent, the established built-in proofreading rules apply. Invalid configuration produces a `Failed` stage report instead of an incomplete artifact being presented as current.

The proofreading configuration's validated canonical TOML participates in the stage fingerprint. Changing rules invalidates the proofreading result while retaining verified cached results for unaffected stages. `proofread` is available to CLI recipes and watch-process through the existing stage-name parser. This adds no external runtime dependency and leaves manuscript content unchanged.
