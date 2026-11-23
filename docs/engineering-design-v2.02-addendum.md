# Engineering Design v2.02 Addendum — Terminal text rendering and tutorial recovery

Alpha UAT 7 reported widely separated manuscript characters, text apparently
showing through the Reference Library, an unclear tutorial resume route, and a
lesson continuation line with missing spaces. The current automated layout
tests did not reproduce the screen corruption. The terminal editor previously
created one Ratatui span per grapheme, even when adjacent graphemes had identical
style. It now coalesces contiguous graphemes into style runs. Selection,
mention, and display-only formatting marks remain supported. Reading text
continues to coalesce annotation styles by source byte ranges. This reduces
unnecessary terminal output and makes the ordinary text path simpler, but the
exact Alpha UAT 7 rendering cause remains unconfirmed pending retest in the
user's terminal environment.

The rendering regression test checks manuscript and reference text at 201×60
and 80×24, switches through the Reference Library and tutorial overlay, and
asserts that source text does not show through the library workspace. It also
checks that scene text remains unchanged. These automated checks do not
establish visual usability on every terminal emulator.

The tutorial footer and Help menu use **Help > Resume Tutorial** as the
reliable route. The unbound `Ctrl+Shift+R` is not advertised. Hiding a lesson
with Esc no longer describes progress as paused or directs users to restart
from the dashboard; saved progress remains active and the message points to
the visible Help command. Lesson completion and its continuation instruction
are separate lines so spacing and the Enter action are unambiguous. Tests hide
the lesson, activate **Help > Resume Tutorial**, and verify the same lesson
step and persistent progress are restored.

Menu rows display concise action labels. Command descriptions remain available
in command search and contextual help, not appended to normal menu choices.
The Recent Projects action is labeled **Recent Projects**.

The shared File menu also exposes **Import Document…**. Text and Org-mode
documents are previewed before import; Org headings can be imported as
chapter/scene structure. This continues Issue #57's import work and does not
change the source file.

Issue #56 (rendering) and Issue #41 (tutorial navigation) remain open for human
verification. Automated tests establish regression coverage, not successful
human UAT.
