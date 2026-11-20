# Writer's IDE — Engineering Design v0.11

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.10

## Saved query access

Saved query summaries expose stable query IDs and names in name order. The CLI supports `query-list`, `query-save`, and `query-run`. The terminal command palette supports `query-list`, `query-save <name> [key=value ...]`, and `query <uuid>`. Supported filters are status, character ID, plotline ID, before/after story-time tick, and unresolved issue. Filters retain the storage layer's AND semantics and current manuscript ordering.

Running a query creates a virtual Search workspace result set. It does not edit scene placement or outline order. Storage tests verify live result refresh and unchanged manuscript ordering; terminal tests verify create/list and CLI/API tests cover listing.
