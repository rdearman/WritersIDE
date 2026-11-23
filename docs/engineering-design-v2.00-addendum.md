# Engineering Design v2.00 Addendum — Tutorial progression focus

Issue #41's Alpha UAT 4 retest showed that the tutorial displayed F6 as its
continue action, while F6 is also assigned to the Characters workspace. The
tutorial now uses Enter as its only progression action. While a tutorial
lesson is visible, F6 is consumed by the tutorial dialog and shows an
explanation; it cannot switch the underlying workspace.

Lesson 1 presents a single current task and a compact completed-task count
instead of showing a checklist with every task at once. Instructions include
the concrete menu route or shortcut. When all required navigation, writing,
save, return and resume actions have been verified, the dialog displays
“Lesson 1 complete — Press Enter: Continue to Lesson 2”. Progress is saved
before the next lesson is displayed. The focused dialog preserves its normal
pause and resume behavior.

Automated tests verify that a completed lesson remains on lesson 1 after F6,
that F6 does not change workspaces, and that Enter advances to lesson 2. The
dialog is rendered at 80×24 and 236×60 in tests. These checks do not replace a
real terminal/PTY retest; Issue #41 remains open for human verification.
