# Writer's IDE — Engineering Design v0.14

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.13

## Ollama command integration and proposal storage

The application façade now exposes `extract_with_ollama(source_kind, source_id, model, endpoint)` for both scene prose and imported reading text. CLI and terminal command-palette invocations use the same operation. They persist a unique JSON proposal in `.writers-ide/proposals/`, with scene/reading identity, source fingerprint, model name, and exact quote offsets. Storage validates a single project-local `.json` filename and refuses to overwrite an existing proposal; it uses the project path safety checks and synced atomic file replacement.

Model use remains opt-in and local-only by default. The user must invoke `extract` explicitly and may choose a localhost endpoint. The proposal is separate from canonical entities, all facts start unapproved, and the existing `approve-extraction` command only records review; it still does not promote a fact. Imported reading bytes remain untouched.

The CLI can also ask for a study guide over scene or reading text, including optional comprehension questions. This response is transient and does not alter story data.

## Acceptance coverage and remaining limitations

- Storage tests check project-local proposal writing, duplicate refusal, and traversal rejection.
- Offline tests check endpoint restrictions and quote/provenance anchoring.
- A successful live Ollama request remains unverified in this sandbox because local TCP binding is denied; no compatibility claim for particular models is made.
- Proposal review/edit/reject/promotion UI, prompt versioning, and passage-level link creation remain outstanding.
