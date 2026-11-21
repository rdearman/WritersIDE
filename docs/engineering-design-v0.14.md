# Writer's IDE — Engineering Design v0.14

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.13

## Spatial planning board canvas

The Planning Boards workspace now renders positioned cards in a Ratatui canvas, using stored coordinates and dimensions independently from manuscript order. The currently selected card is highlighted. `Ctrl+arrow` moves the selected card; `Ctrl+Shift+arrow` changes its width or height in one-unit increments. Dimensions must remain finite and positive. Application, terminal, and CLI operations share the storage resize use case.

`board-export` writes a portable Markdown table to stdout in the CLI and displays the same board projection in the terminal. It includes entity type and stable ID, position, size, and note; it does not change scene order or other story data. This first canvas exposes coordinate planning and card resizing. Rich connectors drawn between cards, mouse dragging, canvas zoom/pan, and board image export remain future work.
