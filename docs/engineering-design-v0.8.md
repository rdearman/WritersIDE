# Writer's IDE — Engineering Design v0.8

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.7  
**Migration level:** 4 (processing outputs are rebuildable files)

## Dependency-aware processing

`story-pipeline` receives a stable application snapshot ordered by manuscript placement. It is independent of Ratatui and SQLite. `WriterApp::processing_input` assembles canonical scene text, targets, character and plotline membership, chronology, and revision summary; `WriterApp::run_processing` executes it. Both the CLI (`process <project>`) and terminal command palette (`:process`) call the same service.

Implemented stages are compile, word count/budget preview, plotline distribution and gaps, character appearance/co-appearance, uncertainty-aware continuity candidates, manuscript export, and revision summary. The compiled manuscript is exported to `exports/compiled.md`; stage details and a cache manifest are stored under `.writers-ide/processing/` and `.writers-ide/pipeline-cache.json`.

Each stage has a fingerprint derived from only the inputs it consumes. Downstream text stages include the compile output fingerprint; analytical stages include their relationship/chronology inputs. Every cached artifact is rehashed before reuse. A missing or modified artifact triggers recomputation. Changed prose recompiles the manuscript, word count, and export while unchanged relationship analysis can be reused. Inputs and outputs are read-only projections of authoritative story data; only output files and cache metadata are written.

## Failure behavior

- Stage reports distinguish `Complete`, `Cached`, `Failed`, and `Skipped`.
- A failed stage does not supply an output to dependents; dependent stages are marked `Skipped`.
- A failed artifact write is reported and its cache entry is removed.
- A corrupt cache manifest returns an explicit processing error rather than claiming a successful run.
- Rebuildable cache/artifact files are not manuscript sources and may be deleted safely.
- Artifacts are written via same-directory temporary files, content sync, atomic rename, and parent-directory sync under the filesystem assumptions documented for project storage.

## Acceptance coverage

1. A fresh run produces compilation, reports, and a readable compiled Markdown export.
2. A second unchanged run verifies and reuses every artifact.
3. Changing only prose reruns text-dependent stages and reuses unchanged relationship analyses.
4. Missing or modified output is rebuilt.
5. An unwritable/invalid artifact destination reports failure and skips dependent stages.
6. App-level processing input uses current manuscript order and source text.

## Remaining decisions

- Whether to persist stage state in SQLite instead of a rebuildable JSON manifest.
- User-selectable stage subsets and export targets.
- DOCX/EPUB and configured compilation options.
- Fine-grained invalidation for per-scene analysis and custom metadata.
- A richer revision report with line-level changes and selected snapshot comparisons.
- More complete continuity diagnostics and user-reviewed diagnostics-to-issue actions.
