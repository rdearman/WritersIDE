# Writer's IDE — Engineering Design v0.7

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.6  
**Migration level:** 4

## Optional local Ollama integration

`story-ollama` is an optional application dependency and does no work until an author invokes its API. The client currently permits only `http://localhost`, `127.0.0.1`, or `::1`; HTTPS and remote model endpoints are rejected to keep this feature explicitly local. Ollama must be installed and running only for an invoked model operation. All non-AI workflows remain usable without it.

The `/api/generate` request asks Ollama for JSON. Extraction proposals cover candidate characters and aliases, locations, organisations, objects, story events and approximate chronology, world rules, and character knowledge claims. Each claim includes a verbatim evidence quote. `attach_provenance` records source kind/ID, SHA-256 fingerprint, source byte length, model name, and byte offsets when the quote occurs in source text. Every claim starts with `approved = false`.

`study_guide` requests summary and explanation, with optional comprehension questions. These are transient responses; they do not change project entities or manuscript text. The CLI `extract` command persists a JSON proposal under `.writers-ide/proposals/`; `approve-extraction` is an explicit review action that updates proposal review flags only. It does not create authoritative story facts. Thus extraction cannot silently alter story data. A later author-facing proposal browser and typed promotion operation remain required before this is considered a complete knowledge workflow.

## Failure and privacy behavior

- No Ollama availability check occurs at project open or application startup.
- Model network requests are only made when the CLI/application explicitly invokes an operation.
- Requests are restricted to a localhost HTTP endpoint and have a bounded timeout.
- HTTP failures, invalid JSON, and missing response content surface as errors; they are never treated as empty successful results.
- The selected source text is sent to the explicitly configured local Ollama service.
- Evidence offsets use UTF-8 byte coordinates. A quote that is not found has null offsets and remains an unverified proposal.
- Model outputs remain proposals even after review unless separately promoted by an explicit domain service; promotion is not implemented in this slice.

## Acceptance coverage and limitations

Unit tests verify local-endpoint restrictions, exact quote offsets, source fingerprints, and provisional state without requiring Ollama or internet access. A real Ollama integration test cannot run in restricted test environments and has not been claimed. Result-shape compatibility should be checked against supported Ollama versions and models before release.

## Remaining decisions

- Whether to persist proposals in SQLite or retain project-local JSON documents with indexing.
- How proposals should represent multiple quote occurrences and passages spanning text segments.
- Whether aliases and entity types need schema-specific candidate payloads.
- The explicit author workflow for approve, reject, edit, and promote operations.
- Model compatibility, prompt versioning, and deterministic request options.
- Whether HTTPS should later be allowed for explicitly configured private LAN model servers.
