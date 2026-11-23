# Engineering Design v2.12 Addendum — Offline Grammar Check Foundation

**Date:** 23 November 2026  
**Related issues:** #4, #18, #33

## Engine evaluation

| Engine | Licence / operation | Language coverage | Decision |
|---|---|---|---|
| Harper 3.2 | Apache-2.0; embedded Rust library; offline | English dialects GB, US, CA, AU and IN | Selected for on-demand grammar and spelling suggestions. Its Markdown parser skips fenced code and tables, and returns character spans and optional edits. |
| LanguageTool | Open source local Java HTTP server; localhost operation is possible | Broad language list including English GB, French and Italian | Not required or started by Writer's IDE. A separately installed local server remains a possible future provider for languages Harper does not cover. |
| Vale | MIT; local prose/style linter | Depends on installed style packages | Not selected as a general grammar engine; its strongest use is configurable style rules. |
| Aspell / Hunspell | Existing local executables and dictionaries | Depends on installed dictionaries | Retained as the established multilingual spelling path and fallback when grammar is unavailable. |

Sources: [Harper](https://github.com/Automattic/harper), [LanguageTool language support](https://help.languagetool.org/hc/en-us/articles/39254526141463-What-languages-does-LanguageTool-support), [LanguageTool local server](https://dev.languagetool.org/http-server), [Vale](https://github.com/vale-cli/vale).

The selected Harper library adds a substantial dependency graph, including its Brill tagging implementation. This is accepted for the first embedded prototype because it avoids a daemon, network calls and model downloads; build size and latency must be measured on release targets before calling this a finished multilingual proofing solution.

Ratatui is advanced to 0.30 because its Unicode-width dependency range can be unified with Harper's requirement. The terminal continues to use Crossterm 0.28 through Ratatui's explicit `crossterm_0_28` compatibility feature. Canvas tests assert Braille-pattern output rather than a particular dot glyph, since rasterisation can choose different valid patterns across Ratatui versions.

## Domain and application contract

`story_analysis::grammar_check(text, language)` accepts canonical UTF-8 Markdown and an effective BCP 47 tag. It returns advisory findings with byte ranges, rule category, explanation and replacement suggestions. Harper's character offsets are mapped to UTF-8 byte boundaries before leaving the analysis crate. Invalid engine offsets return errors rather than being clamped.

The supported grammar dialects are explicitly `en-GB`, `en-US`, `en-CA`, `en-AU` and `en-IN`. Any other language returns an unsupported-language error; the caller can continue with existing Aspell/Hunspell spelling checks. No British rules are silently applied to another language. Project and scene language inheritance already provides the effective language used by the terminal action.

The command `Review → Grammar Check` checks the selected scene by default and opens findings in a read-only reference pane. The optional scope syntax accepts `all`, `chapter:<UUID>` and `scene:<UUID>`. Findings include original UTF-8 byte offsets and the first suggestion where available. Checking does not change manuscript bytes. The pane currently offers inspection only; applying a finding from that pane, ignoring rules and custom-dictionary actions are not implemented in this slice.

The application service now exposes `apply_scene_text_suggestion`. It requires the source SHA-256, expected passage bytes and UTF-8 byte range from the analysis result, rejects stale hashes or changed passage text, and persists an accepted replacement through the existing journalled save path. It is deliberately a service foundation: the terminal does not yet present an accept/ignore workflow, and this operation is not yet connected to the editor's in-memory undo stack.

## Safety and boundaries

- The checker runs locally and does not make HTTP or cloud requests.
- Suggestions remain provisional; there is no auto-apply path.
- Harper's Markdown parser skips code blocks and tables. Current passage-language anchors are honored; stale anchors are skipped and disclosed. Other Markdown syntax still requires further verification.
- On-demand checking is implemented; idle-time, incremental, compile-time and cancellation behavior are not.
- Aspell/Hunspell remain separate spelling workflows. Missing dictionaries do not prevent application startup.
- This is a foundation for #33, not completion of its multilingual acceptance criteria.

## Verification

Automated tests cover supported-language reporting, unsupported-language rejection, British English execution, a grammar finding after multibyte UTF-8 text, Markdown code exclusion, terminal report opening and manuscript immutability. The application suggestion service is tested for journalled persistence, reopen, Unicode offsets and rejection of stale hashes, stale passages and invalid UTF-8 boundaries. Real-terminal interaction and practical language quality still require author verification; automated checks do not establish usability or proofing quality.
