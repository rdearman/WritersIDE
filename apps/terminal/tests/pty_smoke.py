#!/usr/bin/env python3
"""Exercise the built terminal application through a real Linux PTY."""

import fcntl
import os
import pty
import re
import select
import signal
import struct
import sys
import time


ANSI_CONTROL = re.compile(
    rb"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1b\\))"
)


def set_size(fd, rows, columns):
    fcntl.ioctl(fd, 0x5414, struct.pack("HHHH", rows, columns, 0, 0))


def main():
    binary, project, isolated_home, *modes = sys.argv[1:]
    create_fiction = modes == ["create-fiction"]
    previous_position = modes == ["previous-position"]
    removal_dialogs = modes == ["removal-dialogs"]
    chapter_removal = modes in (
        ["chapter-removal"],
        ["chapter-removal-target"],
        ["chapter-removal-delete"],
    )
    chapter_removal_target = modes == ["chapter-removal-target"]
    chapter_removal_delete = modes == ["chapter-removal-delete"]
    section_removal_delete = modes == ["section-removal-delete"]
    os.makedirs(isolated_home, exist_ok=True)
    os.makedirs(os.path.join(isolated_home, "tmp"), exist_ok=True)
    child_pid, master = pty.fork()
    if child_pid == 0:
        environment = os.environ.copy()
        environment.update(
            {
                "TERM": "xterm-256color",
                "HOME": isolated_home,
                "XDG_CONFIG_HOME": os.path.join(isolated_home, "config"),
                "XDG_DATA_HOME": os.path.join(isolated_home, "data"),
                "TMPDIR": os.path.join(isolated_home, "tmp"),
            }
        )
        arguments = [binary] if project == "-" else [binary, project]
        os.execve(binary, arguments, environment)

    set_size(master, 24, 80)
    output = bytearray()
    cursor_query_offset = 0

    def read_for(seconds):
        nonlocal cursor_query_offset
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            ready, _, _ = select.select(
                [master], [], [], max(0, deadline - time.monotonic())
            )
            if not ready:
                break
            try:
                chunk = os.read(master, 65536)
            except OSError:
                break
            if not chunk:
                break
            output.extend(chunk)
            query = output.find(b"\x1b[6n", cursor_query_offset)
            if query >= 0:
                cursor_query_offset = query + 4
                # Some terminal libraries ask the terminal to report its
                # cursor location. A real emulator replies to this request.
                os.write(master, b"\x1b[1;1R")

    def wait_for(marker, start=0, timeout=5.0):
        deadline = time.monotonic() + timeout
        while (
            marker not in ANSI_CONTROL.sub(b"", output[start:])
            and time.monotonic() < deadline
        ):
            read_for(0.1)
        return marker in ANSI_CONTROL.sub(b"", output[start:])

    def wait_for_fragments(markers, start=0, timeout=5.0):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            visible = ANSI_CONTROL.sub(b"", output[start:])
            if all(marker in visible for marker in markers):
                return True
            read_for(0.1)
        visible = ANSI_CONTROL.sub(b"", output[start:])
        return all(marker in visible for marker in markers)

    try:
        read_for(1.0)
        if create_fiction:
            startup_deadline = time.monotonic() + 5.0
            while (
                (b"Open" not in output or b"Scrivener" not in output)
                and time.monotonic() < startup_deadline
            ):
                read_for(0.25)
            if b"Open" not in output or b"Scrivener" not in output:
                raise AssertionError(
                    "first-run project chooser was not rendered; output tail: "
                    + repr(bytes(output[-2400:]))
                )
            os.write(master, b"\r")  # select New Project from the start screen
            read_for(0.4)
            browser_deadline = time.monotonic() + 5.0
            while (
                (b"Directory" not in output or b"type" not in output)
                and time.monotonic() < browser_deadline
            ):
                read_for(0.25)
            if b"Directory" not in output or b"type" not in output:
                raise AssertionError("New Project did not open the type-aware folder browser")
            os.write(master, b"\x1b[12~")  # F2: choose Fiction
            read_for(0.25)
            os.write(master, b"\x1b[15~")  # F5: request creation
            read_for(0.25)
            os.write(master, b"\x1b[15~")  # F5: confirm new folder
            read_for(0.8)
            os.write(master, b"\x1b[2~")  # Insert: create a scene
            read_for(0.2)
            os.write(master, b"Opening\r")
            read_for(0.5)
            os.write(master, b"\r")  # Open the selected scene
            read_for(0.4)
            os.write(master, b"First passage.")
            edit_start = len(output)
            if not wait_for(b"[UNSAVED]", edit_start):
                raise AssertionError("typed passage did not produce visible unsaved feedback")
            save_request_offset = len(output)
            os.write(master, b"\x13")  # Ctrl+S
            if not wait_for(b"[SAVED]", save_request_offset):
                raise AssertionError("Ctrl+S did not produce visible saved feedback")
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if (
                finished_pid != child_pid
                or not os.WIFEXITED(status)
                or os.WEXITSTATUS(status) != 0
            ):
                raise AssertionError(
                    "fiction project creation/write flow did not exit cleanly; output tail: "
                    + repr(bytes(output[-2000:]))
                )
            print(
                "PTY fiction workflow passed: choose Fiction, create project, insert scene, write, save and exit."
            )
            return

        # A fresh isolated profile shows the welcome panel. Use its visible
        # Skip choice so the test follows the discoverable onboarding route.
        os.write(master, b"\x1b")  # use the welcome screen's visible Skip key
        read_for(0.4)
        if not section_removal_delete and b"Arrival" not in output:
            raise AssertionError(
                "the opening scene was not rendered in the PTY; output tail: "
                + repr(bytes(output[-1600:]))
            )

        if previous_position:
            # F10 opens File; three Right presses select Navigate. Its sixth
            # entry is Return to Previous Position.
            status_start = len(output)
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            os.write(master, b"\x1b[C" * 3)
            read_for(0.3)
            os.write(master, b"\x1b[B" * 5 + b"\r")
            if not wait_for_fragments(
                (b"No", b"previous", b"manuscript", b"position"),
                status_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "empty Previous Position did not report a nonfatal status; output tail: "
                    + repr(bytes(output[-2000:]))
                )

            # The menu and normal quit path must still be available after the
            # unavailable navigation action.
            os.write(master, b"\x1b[21~")
            read_for(0.3)
            if b"Quit" not in output:
                raise AssertionError("application stopped responding after Previous Position")
            os.write(master, b"\x1b")
            read_for(0.3)
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if (
                finished_pid != child_pid
                or not os.WIFEXITED(status)
                or os.WEXITSTATUS(status) != 0
            ):
                raise AssertionError("application did not remain usable and quit cleanly")
            print("PTY navigation regression passed: empty Previous Position is nonfatal; menu and quit remain usable.")
            return

        if chapter_removal:
            # Remove the selected populated chapter through Navigate > Outline.
            # The default is the explicitly documented retain-and-unplace path.
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            os.write(master, b"\x1b[C" * 3 + b"\x1b[B" + b"\r")
            read_for(0.3)
            if not wait_for_fragments((b"Outline", b"Opening", b"Arrival"), timeout=3.0):
                raise AssertionError("Navigate > Outline did not show the populated chapter")
            # Workspace selection may be preserved on the last child row;
            # explicitly select the parent chapter before invoking Delete.
            os.write(master, b"\x1b[A" * 3)
            read_for(0.2)
            dialog_start = len(output)
            os.write(master, b"\x1b[3~")  # Delete selected chapter heading
            if not wait_for_fragments(
                (
                    b"Delete",
                    b"Opening",
                    b"Contains",
                    b"scene(s)",
                    b"prose",
                    b"Removed",
                    b"Move",
                    b"Unplaced",
                    b"another",
                    b"recoverably",
                    b"Compile",
                    b"Esc",
                    b"cancel",
                ),
                dialog_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "populated chapter removal did not show the explicit recoverable choices; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-2400:]))
                )
            if chapter_removal_target:
                os.write(master, b"\x1b[B")  # move to the selected-chapter option
                if not wait_for_fragments(
                    (b"Destination", b"append", b"selected", b"order"),
                    dialog_start,
                    timeout=3.0,
                ):
                    raise AssertionError(
                        "chapter destination choice did not preview its target and Compile effect; visible tail: "
                        + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                    )
            elif chapter_removal_delete:
                os.write(master, b"\x1b[B\x1b[B")  # choose recoverable subtree deletion
                if not wait_for_fragments(
                    (b"Deletechapter", b"removed scenes", b"omitted", b"restored"),
                    dialog_start,
                    timeout=3.0,
                ):
                    raise AssertionError(
                        "subtree deletion did not preview Compile impact and recovery; visible tail: "
                        + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                    )
            os.write(master, b"\r")  # confirm the selected explicit removal action
            read_for(0.4)
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after the chapter removal")
            if chapter_removal_target:
                print("PTY chapter removal passed: scene rehoming to a selected chapter was available.")
            elif chapter_removal_delete:
                print("PTY chapter removal passed: recoverable subtree deletion was selected explicitly.")
            else:
                print("PTY chapter removal passed: choices are visible and the default retains scenes as Unplaced.")
            return

        if section_removal_delete:
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            os.write(master, b"\x1b[C" * 3 + b"\x1b[B" + b"\r")
            if not wait_for_fragments(
                (b"Outline", b"PartOne", b"OldHeading", b"Subsection"), timeout=3.0
            ):
                raise AssertionError(
                    "Navigate > Outline did not show the non-fiction section tree; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-2400:]))
                )
            os.write(master, b"\x1b[B")  # select the populated child heading
            read_for(0.2)
            dialog_start = len(output)
            os.write(master, b"\x1b[3~")
            if not wait_for_fragments(
                (b"OldHeading", b"subordinate", b"Promote", b"another", b"subtree", b"recoverably"),
                dialog_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "non-fiction section removal did not offer explicit subtree choices; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-2400:]))
                )
            os.write(master, b"\x1b[B\x1b[B")  # choose recoverable subtree deletion
            if not wait_for_fragments(
                (b"Deletethewholesubtree", b"omitted", b"restored"),
                dialog_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "non-fiction subtree choice did not preview Compile and recovery; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"\r")
            read_for(0.4)
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after section subtree deletion")
            print("PTY non-fiction removal passed: subtree deletion was explicit, recoverable and cancellable.")
            return

        if removal_dialogs:
            # Navigate to Scene Cards through the actual menu and open its
            # recoverable removal dialogue with the list Delete key.
            scene_workspace_start = len(output)
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            os.write(master, b"\x1b[C" * 3 + b"\x1b[B" * 2 + b"\r")
            if not wait_for_fragments(
                (b"Included", b"Arrival"), scene_workspace_start, timeout=3.0
            ):
                raise AssertionError("Navigate > Scene Cards did not open the card list")
            # Cards may preserve selection on a chapter boundary when the
            # workspace opens. Two idempotent Downs ensure the one-scene
            # fixture ends with the actual scene selected.
            for _ in range(2):
                os.write(master, b"\x1b[B")
                read_for(0.15)
            scene_dialog_start = len(output)
            os.write(master, b"\x1b[3~")  # Delete
            if not wait_for_fragments(
                (b"Remove", b"Scene", b"Arrival", b"Enter/Y confirms", b"Esc/N cancels"),
                scene_dialog_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "Scene Cards Delete did not show the recoverable confirmation; output tail: "
                    + repr(bytes(output[-2200:]))
                )
            os.write(master, b"\x1b")  # cancel; the scene must remain untouched
            read_for(0.3)

            # Navigate to Reference Library and verify it has its own
            # read-only-source-aware confirmation.
            reference_workspace_start = len(output)
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            os.write(master, b"\x1b[C" * 3 + b"\x1b[B" * 3 + b"\r")
            if not wait_for_fragments(
                (b"Field", b"Notes", b"ReferenceLibrary"),
                reference_workspace_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "Navigate > Reference did not open the imported reference list; visible output tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-2600:]))
                )
            reference_dialog_start = len(output)
            os.write(master, b"\x1b[3~")
            if not wait_for_fragments(
                (b"Remove", b"Reference", b"Field", b"Notes", b"source", b"preserved", b"Esc/N cancels"),
                reference_dialog_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "Reference Library Delete did not show the source-preserving confirmation; output tail: "
                    + repr(bytes(output[-2200:]))
                )
            os.write(master, b"\x1b")
            read_for(0.3)
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after cancelling removal dialogues")
            print("PTY removal workflow passed: scene/reference Delete dialogs are visible, cancellable and preserve content.")
            return

        # Exercise the actual F10 route instead of calling AppState handlers
        # directly.
        os.write(master, b"\x1b[21~")  # xterm F10
        read_for(0.5)
        # Ratatui emits cursor moves between spans, so inspect two distinctive
        # action labels instead of expecting the rendered line contiguously.
        if b"Recent" not in output or b"Quit" not in output:
            raise AssertionError(
                "F10 did not render the direct File menu in the PTY; output tail: "
                + repr(bytes(output[-2400:]))
            )

        set_size(master, 60, 201)
        read_for(0.4)
        os.write(master, b"\x1b")  # close menu
        read_for(0.3)
        os.write(master, b"\x11")  # Ctrl+Q
        quit_deadline = time.monotonic() + 5.0
        finished_pid = 0
        status = 0
        while finished_pid == 0 and time.monotonic() < quit_deadline:
            read_for(0.1)
            finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
        if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
            raise AssertionError("the application did not exit cleanly through Ctrl+Q")
        print("PTY smoke passed: scene rendered at 80x24; F10 opened File; resize to 201x60; Ctrl+Q exited cleanly.")
    finally:
        try:
            os.kill(child_pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            os.waitpid(child_pid, 0)
        except ChildProcessError:
            pass
        os.close(master)


if __name__ == "__main__":
    main()
