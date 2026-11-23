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

The command `Review → Grammar Check` checks the selected scene by default and opens findings in a reference pane. The optional scope syntax accepts `all`, `chapter:<UUID>` and `scene:<UUID>`. Findings include original UTF-8 byte offsets and the first suggestion where available. The report supports keyboard selection, **A** to apply a suggested replacement, and **I** to ignore a finding for that review. Applying first opens the source scene if needed, requires a clean editor buffer, validates the analysis hash and passage, and persists through the durable write journal. The normal editor undo stack receives the pre-change buffer; after undo, Ctrl+S writes the reverted text through the same optimistic save path. Ignore-once is session-local and does not suppress future checks.

The application service `apply_scene_text_suggestion` requires the source SHA-256, expected passage bytes and UTF-8 byte range from the analysis result, rejects stale hashes or changed passage text, and persists approved edits through the existing journalled save path. It is connected to the terminal report and editor undo stack. After an accepted edit, other findings from that source revision are intentionally rejected as stale; rerun Grammar Check to obtain current ranges before applying another suggestion.

## Safety and boundaries

- The checker runs locally and does not make HTTP or cloud requests.
- Suggestions remain provisional; each change requires an explicit author keypress.
- Harper's Markdown parser skips code blocks and tables. Current passage-language anchors are honored; stale anchors are skipped and disclosed. Other Markdown syntax still requires further verification.
- On-demand checking is implemented. Optional background checks run on a worker thread after a scene save, so they do not block the editor; results are discarded if the source hash has changed before receipt. Checks currently operate per saved scene rather than incrementally by edited passage. Compile-time checking and cancellation are not implemented.
- Aspell/Hunspell remain separate spelling workflows. Missing dictionaries do not prevent application startup.
- This is a foundation for #33, not completion of its multilingual acceptance criteria.

## Verification

Automated tests cover supported-language reporting, unsupported-language rejection, British English execution, a grammar finding after multibyte UTF-8 text, Markdown code exclusion, report immutability, keyboard ignore-once, explicit acceptance, editor undo, stale-source rejection and journalled persistence. Real-terminal interaction and practical language quality still require author verification; automated checks do not establish usability or proofing quality.

## Project-local grammar controls (23 November 2026)

The existing `.writers-ide/diagnostics.toml` is also the source of grammar-check preferences. Its versioned `grammar` table stores explicit Harper category enablement and severity plus excluded exact words/phrases. An omitted grammar table in an older configuration receives the default settings, so earlier project configurations remain readable. Unknown categories, unsupported severities and empty exclusions fail validation rather than being ignored.

`Review → Grammar Settings` opens a reference pane listing categories and excluded terms. The command accepts `grammar-settings CATEGORY on|off|severity LEVEL` and `grammar-settings exclude add|remove TERM`; edits are validated and atomically saved using a same-directory temporary file, file sync, rename and directory sync. The grammar service filters by enabled category, applies the configured severity, and omits findings whose matched text exactly equals an excluded term (case-insensitive). Exclusions do not alter manuscript text or dictionary data. Grammar checks continue to be explicitly invoked; background checking and idle scheduling remain outstanding.

Regression coverage verifies category filtering, severity configuration, exclusions, invalid settings, settings-pane visibility, project-local persistence, and asynchronous post-save results that leave manuscript prose unchanged. The workspace verification for this update passed formatting, Clippy and all tests; the Ollama loopback test required the already-authorized local socket permission in this environment.
