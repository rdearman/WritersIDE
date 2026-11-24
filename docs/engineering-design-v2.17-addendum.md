# Engineering Design v2.17 Addendum — State-verified tutorial actions

**Status:** Implemented in code; first-time usability remains subject to human UAT  
**Date:** 2026-11-24  
**Related issues:** #36, #41, #59

## Tutorial action model

The tutorial panel derives its required actions from the active track and lesson. Lesson completion is based on observed application state and persisted results, not the number of keypresses or a manually checked box. Actions are recorded in the active tutorial preference record only when the sample project is open and the expected state is true.

The current guided tour verifies:

| Lesson | Required state/action |
|---|---|
| Find your way around | Open the menu, reach Navigate, select Outline, switch to Outline, open the sample scene, edit text, save, return to Outline, and resume from Help |
| Your sample book | Open Scene Cards |
| Write and save | Change sample scene text and save it |
| References | Open the Outline reference, then focus it |
| Search and review | Search sample manuscript text and reach a result |
| Compile | Successfully compile an EPUB from the sample project |

Quick Start retains its shorter route: write and save sample prose, then compile EPUB. Enter advances only after every required action for the lesson has completed; F4 returns to the previous lesson, F5 retries, F7 explicitly skips, F8 exits to the original project, and Help > Resume Tutorial restores the saved step and expected workspace. Tutorial commands do not alter the author's original project.

## Focus and dialog ownership

The expected action gate permits keys needed by the active lesson. Once a file browser or Find panel owns input, its required keys are allowed through the tutorial layer. In particular, F5 in the Compile save dialogue confirms output; it must not be interpreted as Retry. The command path is the same registered menu command used outside the tutorial. A successful compile records the lesson only after the output workflow returns success.

The reference lesson explicitly verifies both opening the pane and focusing it. Search requires actual result locations; a no-match search does not count. Compile requires the successful-compile status. Returning to a lesson reconstructs the appropriate workspace and, for the writing lesson, selects the stable sample scene identifier.

## Verification

The terminal regression `guided_tutorial_requires_real_actions_and_resumes_at_the_current_lesson_context` follows the menu command for Find in Manuscript, enters a real query, confirms a result, advances, opens Publish > Compile, cycles the file browser to EPUB, confirms with F5, and checks the produced file and tutorial state. It also verifies that Enter alone cannot complete a lesson, that edits and save are recorded, and that exit/resume preserves the step and sample scene context.

These automated checks demonstrate application state and dispatch in the in-process terminal event handler. They do not establish first-time usability in a local terminal, SSH, or tmux. Human UAT requested by #36, #41, and #59 remains necessary; these issues must remain open.

## Limitations and next steps

This addendum does not claim a complete tutorial redesign or completion of #36. The lessons remain a finite, hard-coded curriculum rather than declarative content linked to versioned help topics. The project-creation and Scrivener-import lessons requested by #36 are not added here, nor have all 16 requested curriculum modules been verified. The #59 issue requires human validation of the initial navigation experience before expanding the guided material. Search and reference focus still mention current application key bindings where applicable; essential navigation should continue to be tested with remapped keys and terminal input variants.
