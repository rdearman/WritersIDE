# Engineering Design v2.42 Addendum — Removed Scenes navigation

This addendum records the correction for the Removed Scenes usability finding in GitHub issue #63. It supplements the existing lifecycle and recovery design; it does not supersede populated-heading deletion policy or claim the wider object-lifecycle requirements complete.

## Behavior

- Scene Cards keeps the Removed Scenes collection separate from the Included and Excluded manuscript sequence.
- Opening Removed Scenes records the currently selected scene by stable scene ID and the selected chapter boundary, where available.
- The empty collection displays `No removed scenes` and a direct `Esc or Scene > Return to Scene Cards` instruction. It does not suggest creating a scene in the removed collection.
- `Scene > Return to Scene Cards` is the explicit selectable return command. Escape also returns to Scene Cards.
- Returning restores the previous chapter boundary and scene selection when those entities remain available. If the prior scene was removed or is unavailable, selection falls back safely to the first available card.
- Restoring a removed scene returns selection to that scene after refresh.

## Verification

The terminal state regression renders the empty view at 80x24, checks the empty-state text and absence of the incorrect Insert instruction, enters the explicit menu return command, and verifies stable selection restoration. A Linux PTY regression exercises Navigate > Scene Cards, F10 > Scene > Show Removed Scenes, the empty-state guidance, Escape return, and clean exit. Existing removal, restoration, persistence and compile tests remain authoritative for content lifecycle behavior.

Automated terminal checks do not replace the human UAT requirement recorded in #63. Populated-heading removal, broader object lifecycle coverage and author acceptance remain tracked separately.
