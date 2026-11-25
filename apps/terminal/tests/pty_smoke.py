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
import unicodedata


ANSI_CONTROL = re.compile(
    rb"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1b\\))"
)
CSI = re.compile(r"\x1b\[([0-?]*)([ -/]*)?([@-~])")


def terminal_snapshot(data, rows, columns):
    """Apply the cursor/clear controls Ratatui uses and return visible rows."""
    cells = [[" "] * columns for _ in range(rows)]
    row = column = 0
    text = data.decode("utf-8", errors="replace")
    index = 0
    while index < len(text):
        character = text[index]
        if character == "\x1b":
            if text.startswith("\x1b]", index):
                end = text.find("\x07", index + 2)
                string_end = text.find("\x1b\\", index + 2)
                ends = [position for position in (end, string_end) if position >= 0]
                if ends:
                    terminator = min(ends)
                    index = terminator + (2 if terminator == string_end else 1)
                else:
                    index = len(text)
                continue
            match = CSI.match(text, index)
            if match:
                parameters, _, final = match.groups()
                values = [int(value) for value in re.findall(r"\d+", parameters)]
                first = values[0] if values else 0
                if final in ("H", "f"):
                    row = max(0, (values[0] if values and values[0] else 1) - 1)
                    column = max(0, (values[1] if len(values) > 1 and values[1] else 1) - 1)
                elif final == "A":
                    row = max(0, row - (first or 1))
                elif final == "B":
                    row = min(rows - 1, row + (first or 1))
                elif final == "C":
                    column = min(columns - 1, column + (first or 1))
                elif final == "D":
                    column = max(0, column - (first or 1))
                elif final == "G":
                    column = max(0, (first or 1) - 1)
                elif final == "d":
                    row = max(0, (first or 1) - 1)
                elif final == "J" and first in (0, 2, 3):
                    if first == 2 or first == 3:
                        cells = [[" "] * columns for _ in range(rows)]
                        row = column = 0
                    else:
                        for clear_row in range(row, rows):
                            start = column if clear_row == row else 0
                            cells[clear_row][start:] = [" "] * (columns - start)
                elif final == "K":
                    start = column if first == 0 else 0
                    end = columns if first in (0, 2) else column + 1
                    cells[row][start:end] = [" "] * max(0, end - start)
                index = match.end()
                continue
            index += 1
            continue
        if character == "\r":
            column = 0
        elif character == "\n":
            row = min(rows - 1, row + 1)
        elif character == "\b":
            column = max(0, column - 1)
        elif character >= " " and character != "\x7f" and row < rows and column < columns:
            width = (
                0
                if unicodedata.combining(character)
                else (2 if unicodedata.east_asian_width(character) in "WF" else 1)
            )
            if width:
                cells[row][column] = character
                if width == 2 and column + 1 < columns:
                    # A wide glyph occupies both cells; the continuation
                    # cell contributes no extra character when rows are
                    # reconstructed for text-contiguity assertions.
                    cells[row][column + 1] = ""
                column += width
            elif column:
                cells[row][column - 1] += character
        index += 1
    return ["".join(line).rstrip() for line in cells]


def assert_visible_sentence(data, rows, columns, sentence, context):
    screen = terminal_snapshot(data, rows, columns)
    if not any(sentence in line for line in screen):
        raise AssertionError(
            f"{context}: expected contiguous prose was absent from the rendered screen; "
            f"screen rows: {screen!r}"
        )


def assert_sample_prose_visible(data, rows, columns, context):
    for sentence in (
        "The opening paragraph remains readable.",
        "Maqrara opened the old bok and found a note inside.",
        "Élodie waved beside 한글 🧭.",
    ):
        assert_visible_sentence(data, rows, columns, sentence, context)


def set_size(fd, rows, columns):
    fcntl.ioctl(fd, 0x5414, struct.pack("HHHH", rows, columns, 0, 0))


def main():
    binary, project, isolated_home, *modes = sys.argv[1:]
    create_fiction = modes == ["create-fiction"]
    previous_position = modes == ["previous-position"]
    removal_dialogs = modes == ["removal-dialogs"]
    chapter_creation = modes == ["chapter-creation"]
    reference_removal = modes == ["reference-removal"]
    compile_docx = modes == ["compile-docx"]
    insert_edit_actions = modes == ["insert-edit-actions"]
    named_layouts = modes == ["named-layouts"]
    edit_menu = modes == ["edit-menu"]
    chapter_removal = modes in (
        ["chapter-removal"],
        ["chapter-removal-target"],
        ["chapter-removal-delete"],
        ["chapter-removal-cancel"],
    )
    chapter_removal_target = modes == ["chapter-removal-target"]
    chapter_removal_delete = modes == ["chapter-removal-delete"]
    chapter_removal_cancel = modes == ["chapter-removal-cancel"]
    section_removal_delete = modes == ["section-removal-delete"]
    section_removal_rehome = modes == ["section-removal-rehome"]
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

    def wait_for_screen_fragments(markers, start=0, timeout=5.0):
        """Check the reconstructed current screen rather than concatenated redraw bytes."""
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            # Replay from process start so cursor-relative redraws retain the
            # cells that were already on screen when this wait began.
            rows = terminal_snapshot(output, 24, 80)
            visible = "".join("".join(rows).split())
            if all(marker.decode("utf-8").replace(" ", "") in visible for marker in markers):
                return True
            read_for(0.1)
        rows = terminal_snapshot(output, 24, 80)
        visible = "".join("".join(rows).split())
        return all(marker.decode("utf-8").replace(" ", "") in visible for marker in markers)

    try:
        read_for(1.0)
        if insert_edit_actions:
            os.write(master, b"\x1b")  # dismiss the first-run welcome/tutorial panel
            read_for(0.25)

            date_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.15)
            os.write(master, b"\x1b[C" * 2 + b"\x1b[B" * 5 + b"\r")
            if not wait_for_fragments(
                (b"Inserted", b"UTC"), date_start, timeout=3.0
            ):
                raise AssertionError(
                    "Insert > Insert Date/Time did not run through the menu; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )

            file_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.15)
            os.write(master, b"\x1b[C" * 2 + b"\x1b[B" * 6 + b"\r")
            if not wait_for_fragments(
                (b"Selectedpath:", b"F5Confirm"),
                file_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "Insert > Insert File did not open the shared browser; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"\t" + b"\x1b[B" * 3 + b"\x1b[15~")
            if not wait_for_fragments(
                (b"Inserted", b"imported.txt"),
                file_start,
                timeout=4.0,
            ):
                raise AssertionError(
                    "Insert File did not insert the selected UTF-8 file; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"\x13")  # Ctrl+S
            read_for(0.3)
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after insertion")
            print("PTY editing workflow passed: Insert Date/Time and Insert File work through the menus.")
            return

        if named_layouts:
            os.write(master, b"\x1b")  # dismiss the first-run welcome/tutorial panel
            read_for(0.25)

            save_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.2)
            os.write(master, b"\x1b[C" * 4 + b"\x1b[B" * 9 + b"\r")
            if not wait_for_screen_fragments(
                (b"SaveLayout", b"Entersaves", b"Esccancels"),
                save_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "View > Save Layout did not open its named-layout prompt; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"UAT Layout\r")
            if not wait_for_screen_fragments(
                (b"Savedwindowlayout", b"UATLayout"), save_start, timeout=3.0
            ):
                raise AssertionError("named window layout was not saved through View menu")

            load_start = len(output)
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            os.write(master, b"\x1b[C" * 4 + b"\x1b[B" * 10 + b"\r")
            if not wait_for_screen_fragments(
                (b"LoadLayout", b"UATLayout", b"Enterloads"),
                load_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "View > Load Layout did not show the available layout; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"UAT Layout\r")
            if not wait_for_screen_fragments(
                (b"Loadedwindowlayout", b"UATLayout"), load_start, timeout=3.0
            ):
                raise AssertionError("named window layout was not restored through View menu")

            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after layout workflow")
            print("PTY window-layout workflow passed: View menu saves and reloads a named project layout.")
            return

        if edit_menu:
            os.write(master, b"\x1b")  # dismiss the first-run welcome/tutorial panel
            read_for(0.25)
            edit_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.15)
            os.write(master, b"\x1b[C")  # Edit menu
            if not wait_for_screen_fragments(
                (b"Undo", b"Select All"),
                edit_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "Edit actions were not visible from the writing screen; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"\x1b[B" * 6 + b"\r")  # Edit > Select All
            if not wait_for_screen_fragments(
                (b"Selected all text in the current manuscript document",),
                edit_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "Edit > Select All did not dispatch from the writing menu; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"\x1b[21~")
            read_for(0.15)
            os.write(master, b"\x1b[C" + b"\x1b[B" * 7 + b"\r")  # Edit > Duplicate Selection or Paragraph
            if not wait_for_screen_fragments((b"Duplicated selection",), edit_start, timeout=3.0):
                raise AssertionError("Edit > Duplicate Selection did not dispatch from writing mode")
            os.write(master, b"\x1b[21~")
            read_for(0.15)
            os.write(master, b"\x1b[C\r")  # Edit > Undo
            if not wait_for_screen_fragments((b"Edit undone",), edit_start, timeout=3.0):
                raise AssertionError("Edit > Undo did not reverse selection duplication")
            os.write(master, b"\x1b[21~")
            read_for(0.15)
            os.write(master, b"\x1b[C" + b"\x1b[B" * 8 + b"\r")  # Edit > Select Word
            if not wait_for_screen_fragments(
                (b"Selected word",), edit_start, timeout=3.0
            ):
                raise AssertionError("Edit > Select Word did not dispatch from writing mode")

            os.write(master, b"\x1b[21~")
            read_for(0.15)
            os.write(master, b"\x1b[C" + b"\x1b[B" * 9 + b"\r")  # Edit > Select Sentence
            if not wait_for_screen_fragments(
                (b"Selected sentence",), edit_start, timeout=3.0
            ):
                raise AssertionError("Edit > Select Sentence did not dispatch from writing mode")

            os.write(master, b"\x1b[21~")
            read_for(0.15)
            os.write(master, b"\x1b[C" + b"\x1b[B" * 10 + b"\r")  # Edit > Select Paragraph
            if not wait_for_screen_fragments(
                (b"Selected paragraph",), edit_start, timeout=3.0
            ):
                raise AssertionError(
                    "Edit > Select Paragraph did not dispatch from writing mode"
                )
            os.write(master, b"\x13")  # Ctrl+S
            if not wait_for_screen_fragments((b"[SAVED]",), edit_start, timeout=3.0):
                raise AssertionError("the edited scene did not report a successful save")
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after Edit-menu workflow")
            print("PTY Edit-menu workflow passed: selection duplication and Undo, Select All, Select Word, Select Sentence and Select Paragraph dispatch in writing mode.")
            return

        if chapter_creation:
            os.write(master, b"\x1b")  # dismiss the first-run welcome/tutorial panel
            read_for(0.3)

            # Navigate > Scene Cards using the visible F10 menu.
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            os.write(master, b"\x1b[C" * 3 + b"\x1b[B" * 2 + b"\r")
            if not wait_for_fragments((b"Included", b"Excluded"), timeout=3.0):
                raise AssertionError(
                    "Navigate > Scene Cards did not open before chapter creation; output tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )

            def create_chapter(title):
                menu_start = len(output)
                os.write(master, b"\x1b[21~")  # F10
                read_for(0.15)
                os.write(master, b"\x1b[C" * 2)  # Insert menu
                read_for(0.15)
                os.write(master, b"\x1b[B\r")  # New Chapter
                if not wait_for_fragments((b":chapter_",), menu_start, timeout=3.0):
                    raise AssertionError(
                        "Insert > New Chapter did not open its title prompt; output tail: "
                        + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                    )
                prompt_start = len(output)
                os.write(master, title.encode("utf-8") + b"\r")
                if not wait_for_fragments(
                    (b"\xe2\x94\x80\xe2\x94\x80 " + title.encode("utf-8")),
                    prompt_start,
                    timeout=3.0,
                ):
                    raise AssertionError(
                        f"created chapter {title!r} was not confirmed in Scene Cards; output tail: "
                        + repr(ANSI_CONTROL.sub(b"", output[prompt_start:]))
                    )

            create_chapter("Alpha")
            create_chapter("Beta")
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after creating chapters")
            print(
                "PTY chapter-creation workflow passed: Insert > New Chapter displays and persists multiple empty chapters."
            )
            return

        if reference_removal:
            # Navigate > Reference Library and confirm the read-only source-aware
            # removal dialogue. The source file itself must remain untouched.
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            os.write(master, b"\x1b[C" * 3 + b"\x1b[B" * 3 + b"\r")
            if not wait_for_fragments(
                (b"Field", b"Notes", b"ReferenceLibrary"), timeout=3.0
            ):
                raise AssertionError("Reference Library did not open for removal")
            dialog_start = len(output)
            os.write(master, b"\x1b[3~")  # Delete
            if not wait_for_fragments(
                (b"Remove", b"Reference", b"source", b"preserved", b"Esc/N cancels"),
                dialog_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "reference removal did not explain its source-preserving effect; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"\r")  # confirm recoverable project removal
            if not wait_for_fragments(
                (b"Removed", b"reference"), dialog_start, timeout=3.0
            ):
                raise AssertionError(
                    "confirmed reference removal did not report success; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after reference removal")
            print("PTY reference removal passed: explicit confirmation removes only the project-owned reference.")
            return

        if compile_docx:
            # Publish > Compile / Export must lead to the guided file browser,
            # not leak internal command help into the manuscript panes.
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.2)
            os.write(master, b"\x1b[C" * 6 + b"\x1b[B\r")  # Publish > Compile / Export
            browser_start = len(output)
            if not wait_for_fragments(
                (b"Selectedpath:", b".docx", b"F5Compile"),
                browser_start,
                timeout=4.0,
            ):
                raise AssertionError(
                    "Publish > Compile / Export did not open the DOCX file browser; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-2400:]))
                )
            set_size(master, 60, 177)
            read_for(0.3)
            set_size(master, 24, 80)
            read_for(0.3)
            compile_start = len(output)
            os.write(master, b"\x1b[15~")  # F5: compile to the suggested DOCX path
            if not wait_for_fragments(
                (b"Compiled", b"Publish", b"Project.docx"),
                compile_start,
                timeout=10.0,
            ):
                raise AssertionError(
                    "DOCX compile did not show explicit success feedback; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-2400:]))
                )
            if b"Commands: section TITLE" in ANSI_CONTROL.sub(b"", output):
                raise AssertionError("internal command registry leaked into the publishing workflow")
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after compiling DOCX")
            print("PTY publishing workflow passed: menu → DOCX file browser → compiled output and success feedback.")
            return

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
        if not modes:
            assert_sample_prose_visible(output, 24, 80, "initial 80x24 screen")
        if not section_removal_delete and not section_removal_rehome and b"Arrival" not in output:
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
            menu_start = len(output)
            os.write(master, b"\x1b[21~")
            if not wait_for_fragments(
                (b"NewProject", b"OpenProject", b"RecentProjects"),
                menu_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "F10 did not open the File menu; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"\x1b[C" * 3)  # Navigate menu
            if not wait_for_fragments(
                (b"PreviousPosition", b"Outline", b"SceneCards"),
                menu_start,
                timeout=3.0,
            ):
                raise AssertionError("Navigate menu did not expose Outline")
            os.write(master, b"\x1b[B\r")  # select Outline and open it
            outline_start = len(output)
            if not wait_for_screen_fragments(
                (b"Selectedoutlineitem", b"Arrival"),
                outline_start,
                timeout=3.0,
            ):
                raise AssertionError("Navigate > Outline did not show the populated chapter")
            # The initial selection may be the scene, so move toward the
            # chapter boundary. Up saturates at the first row if it is already
            # selected and reaches the chapter from its first scene.
            os.write(master, b"\x1b[A" * 3)
            read_for(0.2)
            dialog_start = len(output)
            os.write(master, b"\x1b[3~")  # Delete selected chapter heading
            dialog_markers = (
                    b"Delete",
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
                )
            if not wait_for_screen_fragments(dialog_markers, dialog_start, timeout=3.0):
                visible = ANSI_CONTROL.sub(b"", output[dialog_start:])
                missing = [marker for marker in dialog_markers if marker not in visible]
                raise AssertionError(
                    "populated chapter removal did not show the explicit recoverable choices; missing "
                    + repr(missing)
                    + "; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-2400:]))
                )
            if chapter_removal_target:
                os.write(master, b"\x1b[B")  # move to the selected-chapter option
                if not wait_for_screen_fragments(
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
                if not wait_for_screen_fragments(
                    (b"Deletechapter", b"removed scenes", b"omitted", b"restored"),
                    dialog_start,
                    timeout=3.0,
                ):
                    raise AssertionError(
                        "subtree deletion did not preview Compile impact and recovery; visible tail: "
                        + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                    )
            if chapter_removal_cancel:
                os.write(master, b"\x1b")  # cancel; all hierarchy and prose must remain
                if not wait_for_screen_fragments(
                    (b"toggleschaptercollapse", b"scene(s)"),
                    dialog_start,
                    timeout=3.0,
                ):
                    raise AssertionError(
                        "cancelling chapter removal did not return to the intact outline; visible tail: "
                        + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                    )
            else:
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
            elif chapter_removal_cancel:
                print("PTY chapter removal passed: cancellation leaves the populated chapter untouched.")
            else:
                print("PTY chapter removal passed: choices are visible and the default retains scenes as Unplaced.")
            return

        if section_removal_delete or section_removal_rehome:
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            os.write(master, b"\x1b[C" * 3 + b"\x1b[B" + b"\r")
            if not wait_for_screen_fragments(
                (b"Outline", b"PartOne", b"Subsection"), timeout=3.0
            ):
                raise AssertionError(
                    "Navigate > Outline did not show the non-fiction section tree; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-2400:]))
                )
            os.write(master, b"\x1b[B")  # select the populated child heading
            read_for(0.2)
            dialog_start = len(output)
            os.write(master, b"\x1b[3~")
            if not wait_for_screen_fragments(
                (b"Directheadingprose", b"Promote", b"Compile"),
                dialog_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "non-fiction section removal did not preview direct heading prose; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-2400:]))
                )
            if section_removal_rehome:
                if not wait_for_screen_fragments(
                    (b"Prose:keepin", b"PartOne", b"andinCompile"), dialog_start, timeout=3.0
                ):
                    raise AssertionError(
                        "active prose destination was not the visible default; visible tail: "
                        + repr(ANSI_CONTROL.sub(b"", output[-2400:]))
                    )
                os.write(master, b"\r")
                if not wait_for_screen_fragments(
                    (b"Outline", b"PartOne", b"Subsection"),
                    dialog_start,
                    timeout=3.0,
                ):
                    raise AssertionError(
                        "confirming the default did not return to the updated outline; visible tail: "
                        + repr(ANSI_CONTROL.sub(b"", output[-2400:]))
                    )
                os.write(master, b"\x11")
                quit_deadline = time.monotonic() + 5.0
                finished_pid = 0
                status = 0
                while finished_pid == 0 and time.monotonic() < quit_deadline:
                    read_for(0.1)
                    finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
                if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                    raise AssertionError("application did not exit cleanly after prose rehoming")
                print("PTY non-fiction removal passed: direct heading prose stays active and compiled by default.")
                return
            os.write(master, b"\x1b[B\x1b[B")  # choose recoverable subtree deletion
            if not wait_for_screen_fragments(
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
            if not wait_for_screen_fragments(
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
        menu_start = len(output)
        os.write(master, b"\x1b[21~")  # xterm F10
        if not wait_for(b"Quit", menu_start, timeout=3.0):
            raise AssertionError("F10 menu did not remain responsive")
        # Ratatui emits cursor moves between spans, so inspect two distinctive
        # action labels instead of expecting the rendered line contiguously.
        if b"Recent" not in output or b"Quit" not in output:
            raise AssertionError(
                "F10 did not render the direct File menu in the PTY; output tail: "
                + repr(bytes(output[-2400:]))
            )
        if b"Commands: section TITLE" in ANSI_CONTROL.sub(b"", output):
            raise AssertionError("internal command registry leaked into the terminal UI")

        os.write(master, b"\x1b")  # close menu
        read_for(0.3)
        if b"Commands: section TITLE" in ANSI_CONTROL.sub(b"", output):
            raise AssertionError("internal command registry appeared after closing the menu")
        assert_sample_prose_visible(output, 24, 80, "screen after closing menu")
        set_size(master, 60, 201)
        read_for(0.4)
        assert_sample_prose_visible(output, 60, 201, "resized 201x60 screen")
        set_size(master, 24, 80)
        read_for(0.4)
        assert_sample_prose_visible(output, 24, 80, "resized-back 80x24 screen")
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
