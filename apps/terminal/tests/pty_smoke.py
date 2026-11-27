#!/usr/bin/env python3
"""Exercise the built terminal application through a real Linux PTY."""

import fcntl
import json
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
    create_nonfiction = modes == ["create-nonfiction"]
    continue_fiction = modes == ["continue-fiction"]
    tutorial_navigation = modes == ["tutorial-navigation"]
    scene_rename = modes == ["scene-rename"]
    heading_rename = modes == ["heading-rename"]
    previous_position = modes == ["previous-position"]
    external_conflict = modes == ["external-conflict"]
    removal_dialogs = modes == ["removal-dialogs"]
    chapter_creation = modes == ["chapter-creation"]
    revision_snapshot = modes == ["revision-snapshot"]
    revision_document = modes == ["revision-document"]
    backup_archive = modes == ["backup-archive"]
    authoring_audit = modes == ["authoring-audit"]
    reference_removal = modes == ["reference-removal"]
    compile_docx = modes == ["compile-docx"]
    scrivener_import = modes == ["scrivener-import"]
    save_as_populated = modes == ["save-as-populated"]
    recent_projects = modes == ["recent-projects"]
    insert_edit_actions = modes == ["insert-edit-actions"]
    outline_enter = modes == ["outline-enter"]
    special_character = modes == ["special-character"]
    named_layouts = modes == ["named-layouts"]
    edit_menu = modes == ["edit-menu"]
    date_time_insertion = modes == ["date-time-insertion"]
    date_time_unavailable = modes == ["date-time-unavailable"]
    find_replace_focus = modes == ["find-replace-focus"]
    review_thesaurus = modes == ["review-thesaurus"]
    review_menu = modes in (["review-menu"], ["review-thesaurus"])
    removed_scenes = modes == ["removed-scenes"]
    outline_view = modes == ["outline-view"]
    chapter_removal = modes in (
        ["chapter-removal"],
        ["chapter-removal-target"],
        ["chapter-removal-delete"],
        ["chapter-removal-cancel"],
        ["chapter-removal-undo-redo"],
    )
    chapter_removal_undo_redo = modes == ["chapter-removal-undo-redo"]
    chapter_removal_target = modes == ["chapter-removal-target"]
    chapter_removal_delete = modes in (["chapter-removal-delete"], ["chapter-removal-undo-redo"])
    chapter_removal_cancel = modes == ["chapter-removal-cancel"]
    section_removal_delete = modes == ["section-removal-delete"]
    section_removal_rehome = modes == ["section-removal-rehome"]
    os.makedirs(isolated_home, exist_ok=True)
    os.makedirs(os.path.join(isolated_home, "tmp"), exist_ok=True)
    if create_fiction or create_nonfiction or recent_projects or backup_archive:
        # Keep this workflow focused on project lifecycle rather than allowing
        # first-run onboarding to consume editing keys.
        onboarding = os.path.join(isolated_home, "config", "writers-ide")
        os.makedirs(onboarding, exist_ok=True)
        with open(os.path.join(onboarding, "tutorial-state.json"), "w", encoding="utf-8") as state_file:
            state_file.write(
                '{"version":1,"welcome_seen":true,"tutorial_dismissed":true,'
                '"tutorial_completed":false,"active_tutorial":null}'
            )
    if recent_projects:
        recent_path = os.environ.get("WIDE_PTY_RECENT_PROJECT")
        if not recent_path:
            raise AssertionError("recent-projects PTY requires a target project path")
        recent_file = os.path.join(isolated_home, "config", "writers-ide", "recent-projects.json")
        os.makedirs(os.path.dirname(recent_file), exist_ok=True)
        with open(recent_file, "w", encoding="utf-8") as recent_store:
            json.dump({"version": 1, "paths": [recent_path]}, recent_store)
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

    def wait_for_workspace_title(title, timeout=5.0):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            rows = terminal_snapshot(output, 24, 80)
            if len(rows) > 1 and rows[1].startswith(f"┌{title}"):
                return True
            read_for(0.1)
        rows = terminal_snapshot(output, 24, 80)
        return len(rows) > 1 and rows[1].startswith(f"┌{title}")

    def move_outline_selection_until(predicate, direction, attempts=180):
        """Navigate by rendered row labels, not fixture-specific row offsets."""
        key = b"\x1b[A" if direction == "up" else b"\x1b[B"
        for _ in range(attempts):
            rows = terminal_snapshot(output, 24, 80)
            selected = next(
                (row for row in rows if row.strip("│ ").startswith("▸")),
                "",
            )
            if predicate(selected):
                return True
            os.write(master, key)
            read_for(0.12)
        return False

    def move_outline_selection_to_manuscript_root(attempts=180):
        for _ in range(attempts):
            rows = terminal_snapshot(output, 24, 80)
            selected = next(
                (row for row in rows if row.strip("│ ").startswith("▸")),
                "",
            )
            if "Manuscript" in selected:
                return True
            os.write(master, b"\x1b[A")
            read_for(0.12)
        return False

    def show_scene_cards_presentation():
        """Select the card presentation from the unified Outline's View menu."""
        menu_start = len(output)
        os.write(master, b"\x1b[21~")  # F10
        read_for(0.2)
        os.write(master, b"\x1b[C" * 4)  # View
        if wait_for_screen_fragments((b"Show Scene Cards",), menu_start, timeout=2.0):
            os.write(master, b"\r")
        elif wait_for_screen_fragments((b"Show Outline Tree",), menu_start, timeout=2.0):
            os.write(master, b"\x1b")  # Already in the card presentation.
        else:
            raise AssertionError("View menu did not identify the active Outline presentation")
        if not wait_for_screen_fragments((b"Included", b"Excluded"), menu_start, timeout=3.0):
            raise AssertionError("Scene Cards presentation did not render its Included and Excluded collections")

    def show_outline_tree_presentation():
        """Open the unified Outline through the visible View menu."""
        rows = terminal_snapshot(output, 24, 80)
        if len(rows) > 1 and rows[1].startswith("┌Editor"):
            os.write(master, b"\x1b")  # use the editor's advertised return route first
            if not wait_for_screen_fragments((b"Included", b"Excluded"), timeout=3.0) and not wait_for_workspace_title("Outline"):
                raise AssertionError(
                    "Esc did not return from the editor to its manuscript outliner; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
        if wait_for_workspace_title("Outline", timeout=0.1):
            return
        menu_start = len(output)
        os.write(master, b"\x1b[21~")  # F10
        read_for(0.2)
        os.write(master, b"\x1b[C" * 4)  # View
        if wait_for_screen_fragments((b"Show Outline Tree",), menu_start, timeout=2.0):
            select_visible_menu_item("Show Outline Tree")
            os.write(master, b"\r")
        elif wait_for_screen_fragments((b"Show Scene Cards",), menu_start, timeout=2.0):
            # The menu offers the other presentation when Outline is already
            # active; dismiss it without changing the current view.
            os.write(master, b"\x1b")
        else:
            raise AssertionError(
                "View menu did not identify the Outline presentation; screen: "
                + repr(terminal_snapshot(output, 24, 80))
            )
        if not wait_for_workspace_title("Outline", timeout=3.0):
            raise AssertionError(
                "View > Show Outline Tree did not open the manuscript outline; screen: "
                + repr(terminal_snapshot(output, 24, 80))
            )

    def select_visible_menu_item(label):
        for _ in range(64):
            if wait_for_screen_fragments((f"▸ {label}".encode("utf-8"),), timeout=0.12):
                return
            os.write(master, b"\x1b[B")
            read_for(0.04)
        raise AssertionError(
            f"menu did not expose selectable item {label!r}; visible tail: "
            + repr(ANSI_CONTROL.sub(b"", output[-2200:]))
            + "\ncurrent screen: "
            + repr(terminal_snapshot(output, 24, 80))
        )

    def open_outline_menu():
        start = len(output)
        os.write(master, b"\x1b[21~")  # F10
        read_for(0.2)
        for _ in range(12):
            if wait_for_screen_fragments(("Outline · actions".encode("utf-8"),), start, timeout=0.1):
                return start
            os.write(master, b"\x1b[C")
            read_for(0.05)
        raise AssertionError(
            "Outline menu was not reachable from the manuscript outliner; screen: "
            + repr(terminal_snapshot(output, 24, 80))
        )

    def open_help_menu(already_open=False):
        """Find Help from the current menu root instead of assuming its index."""
        if not already_open:
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.2)
        for _ in range(10):
            rows = terminal_snapshot(output, 24, 80)
            if any("▸ Context Help" in row or "▸ Resume Tutorial" in row for row in rows):
                return
            os.write(master, b"\x1b[C")
            read_for(0.1)
        raise AssertionError(
            "Help menu was not reachable from the current root; screen: "
            + repr(terminal_snapshot(output, 24, 80))
        )

    try:
        read_for(1.0)
        if backup_archive:
            start = len(output)
            os.write(master, b"\x1b[21~")  # File menu
            select_visible_menu_item("Back Up Project")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Back Up Project", b"Choose a separate destination"), start, timeout=3.0
            ):
                raise AssertionError("Back Up Project did not open the destination browser")
            os.write(master, b"\x1b[15~")  # F5: create backup archive
            if not wait_for_screen_fragments(
                (b"Project backup archive created at",), start, timeout=8.0
            ):
                raise AssertionError(
                    "whole-project backup did not report verified success; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 8.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after project backup")
            print("PTY whole-project backup passed: File > Back Up Project creates a verified archive.")
            return

        if revision_snapshot or revision_document:
            os.write(master, b"\x1b")  # dismiss first-run help if present
            if not wait_for_screen_fragments((b"The original passage stays.",), timeout=3.0):
                raise AssertionError("the test manuscript did not open in the writing workspace")

            snapshot_start = len(output)
            os.write(master, b"\x10snapshot Before revision test\r")
            if not wait_for_screen_fragments((b"Snapshot created:",), snapshot_start, timeout=4.0):
                raise AssertionError(
                    "the author-facing command palette did not confirm the named snapshot; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )

            for character in "A later discovery.\n":
                os.write(master, character.encode("utf-8"))
                read_for(0.015)
            os.write(master, b"\x13")  # Ctrl+S
            if not wait_for_screen_fragments((b"[SAVED]",), snapshot_start, timeout=4.0):
                raise AssertionError("the edited manuscript did not report a successful save")

            revisions_start = len(output)
            os.write(master, b"\x10Revisions\r")
            if not wait_for_workspace_title("Revisions", timeout=4.0):
                raise AssertionError(
                    "the command palette did not open the revision workspace; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            if not wait_for_screen_fragments(
                ("A Discovery  ·  Before revision test".encode("utf-8"),),
                revisions_start,
                timeout=4.0,
            ):
                raise AssertionError(
                    "the named snapshot was not discoverable in revision history; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"hunk 1/", b"A later discovery."), revisions_start, timeout=4.0):
                raise AssertionError(
                    "opening the saved revision did not show a readable text diff; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            if revision_document:
                os.write(master, b"R")
                if not wait_for_screen_fragments((b"Replace the entire document", b"Before revision test"), timeout=4.0):
                    raise AssertionError("whole-document restore did not show its confirmation")
                os.write(master, b"\x1b")
                if not wait_for_screen_fragments((b"Document restore cancelled",), timeout=4.0):
                    raise AssertionError("cancelling whole-document restore gave no feedback")
                os.write(master, b"R")
                if not wait_for_screen_fragments((b"Replace the entire document",), timeout=4.0):
                    raise AssertionError("whole-document restore confirmation could not be reopened")
                os.write(master, b"\r")
                if not wait_for_screen_fragments((b"Entire document restored",), timeout=4.0):
                    raise AssertionError("whole-document restore gave no success feedback")
                os.write(master, b"\x11")  # Ctrl+Q
                quit_deadline = time.monotonic() + 8.0
                finished_pid = 0
                status = 0
                while finished_pid == 0 and time.monotonic() < quit_deadline:
                    read_for(0.1)
                    finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
                if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                    raise AssertionError("the whole-document restore workflow did not exit cleanly")
                print("PTY whole-document restore passed: named snapshot, preview, cancel, confirm and safety snapshot.")
                return
            os.write(master, b"r")
            if not wait_for_screen_fragments((b"Enter/y confirms",), timeout=4.0):
                raise AssertionError("revision restore did not ask for explicit confirmation")
            os.write(master, b"\x1b")  # cancel without changing current manuscript
            if not wait_for_screen_fragments((b"Revision restore cancelled",), timeout=4.0):
                raise AssertionError("cancelling revision restoration gave no feedback")
            os.write(master, b"r")
            if not wait_for_screen_fragments((b"Enter/y confirms",), timeout=4.0):
                raise AssertionError("revision restore confirmation could not be reopened")
            os.write(master, b"\r")  # confirm hunk restoration
            if not wait_for_screen_fragments((b"Selected revision hunk restored",), timeout=4.0):
                raise AssertionError("restoring the selected revision hunk gave no success feedback")

            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 8.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("the revision workflow did not exit cleanly")
            print("PTY revision workflow passed: named snapshot, saved edit, readable diff and selective restore.")
            return

        if recent_projects:
            menu_start = len(output)
            os.write(master, b"\x1b[21~")  # F10, File menu
            read_for(0.2)
            select_visible_menu_item("Recent Projects")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Recent Projects", b"Enter open"), menu_start, timeout=5.0
            ):
                raise AssertionError("File > Recent Projects did not open its project picker")
            # Startup adds the current project at index zero; the seeded
            # destination is therefore the second visible project.
            os.write(master, b"\x1b[B\r")
            if not wait_for_screen_fragments(
                (b"Target Project", b"Included", b"The Target Scene"), timeout=5.0
            ):
                raise AssertionError(
                    "Recent Projects did not open the selected book; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\r")
            if not wait_for_workspace_title("Editor", timeout=5.0) or not wait_for_screen_fragments(
                (b"Text from the selected recent book.",), timeout=5.0
            ):
                raise AssertionError(
                    "the opened recent book did not navigate to its manuscript; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 8.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after opening a recent project")
            print("PTY Recent Projects workflow passed: File menu → select recent book → open its outline.")
            return

        if save_as_populated:
            os.write(master, b"\x1b")  # dismiss first-run help if present
            read_for(0.2)
            menu_start = len(output)
            os.write(master, b"\x1b[21~")  # F10, File menu
            read_for(0.2)
            select_visible_menu_item("Save Project As")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Save Project As", b"F5 Save Project Copy"), menu_start, timeout=5.0
            ):
                raise AssertionError("File > Save Project As did not open its folder browser")
            os.write(master, b"\x1b[15~")  # F5: prepare the copy
            if not wait_for_screen_fragments(
                (b"Create an independent project copy", b"F5"), timeout=5.0
            ):
                raise AssertionError("Save As did not show its destination confirmation")
            os.write(master, b"\x1b[15~")  # F5: confirm the independent copy
            if not wait_for_screen_fragments((b"Saved project at",), timeout=10.0):
                raise AssertionError(
                    "Save As did not report the copied project; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 8.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after Save As")
            print("PTY Save As workflow passed: File menu → destination browser → independent copy → explicit success.")
            return

        if scrivener_import:
            expected_titles = os.environ.get(
                "WIDE_PTY_IMPORT_TITLES", "Part One|Chapter One|Opening Scene"
            ).split("|")
            expected_titles = [title for title in expected_titles if title]
            expected_text = os.environ.get(
                "WIDE_PTY_IMPORT_TEXT", "The patient blacksmith"
            )
            os.write(master, b"\x1b")  # dismiss first-run help if present
            read_for(0.2)
            menu_start = len(output)
            os.write(master, b"\x1b[21~")  # F10, File menu
            read_for(0.2)
            select_visible_menu_item("Import Scrivener as New Project")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Import Scrivener", b"Folder contents"), menu_start, timeout=5.0
            ):
                raise AssertionError("File > Import Scrivener did not open the file browser")
            os.write(master, b"\t\r")  # select .scriv folder; Enter imports directly into staging
            deadline = time.monotonic() + 120.0
            imported_workspace_ready = False
            imported_in_outline = False
            while time.monotonic() < deadline:
                rows = terminal_snapshot(output, 24, 80)
                visible = "".join("".join(rows).split())
                imported_in_outline = len(rows) > 1 and rows[1].startswith("┌Outline")
                if imported_in_outline or ("Included" in visible and "Excluded" in visible):
                    imported_workspace_ready = True
                    break
                read_for(0.1)
            if not imported_workspace_ready:
                raise AssertionError(
                    "import did not open the preserved manuscript workspace; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            # Import may briefly render its structural outline before the
            # normal fiction card workspace settles. Use the visible View
            # menu to ensure this workflow proceeds in the hierarchy.
            show_outline_tree_presentation()
            if expected_titles and not wait_for_screen_fragments(
                tuple(title.encode("utf-8") for title in expected_titles), timeout=3.0
            ):
                raise AssertionError(
                    "the imported hierarchy did not preserve its expected titles; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            if expected_titles and not wait_for_screen_fragments(
                ("▸ [Included] [ ] Opening Scene".encode("utf-8"),), timeout=2.0
            ):
                raise AssertionError(
                    "the imported project did not focus its first scene in the outline; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            direct_prose_workflow = os.environ.get("WIDE_PTY_TEST_DIRECT_PROSE") == "1"
            if direct_prose_workflow:
                # Exercise chapter-owned prose while the imported project is
                # still temporary. The reported conflict occurred before the
                # first explicit Save/promotion, so testing only a permanent
                # project would miss the affected lifecycle path.
                # Real projects may have nested sections and many rows before
                # direct prose, so locate the visible body row by its normal
                # outliner label instead of assuming it is one Up key away.
                if not move_outline_selection_to_manuscript_root():
                    raise AssertionError("could not navigate to the top of the imported outline")
                body_row_marker = os.environ.get(
                    "WIDE_PTY_DIRECT_PROSE_ROW_MARKER", "chapter_body"
                )
                if not move_outline_selection_until(
                    lambda row: body_row_marker in row,
                    "down",
                ):
                    raise AssertionError(
                        f"outline did not expose selected {body_row_marker!r} content; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
                selected_direct_row = next(
                    (
                        row
                        for row in terminal_snapshot(output, 24, 80)
                        if row.strip("│ ").startswith("▸")
                    ),
                    "<no selected row>",
                )
                output_before_direct_edit = len(output)
                os.write(master, b"\r")
                direct_prose_markers = [b"direct prose"]
                source_prose_marker = os.environ.get(
                    "WIDE_PTY_DIRECT_PROSE_MARKER",
                    "Chapter prose survives direct editing",
                )
                if source_prose_marker:
                    direct_prose_markers.append(source_prose_marker.encode("utf-8"))
                if not wait_for_screen_fragments(
                    tuple(direct_prose_markers),
                    output_before_direct_edit,
                    timeout=5.0,
                ):
                    raise AssertionError(
                        f"temporary chapter prose did not open from {selected_direct_row!r}; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
                os.write(master, b"Edited ")
                if not wait_for_screen_fragments(
                    (b"Autosaved",), output_before_direct_edit, timeout=8.0
                ):
                    raise AssertionError(
                        "temporary chapter prose did not autosave; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
                if b"Autosave could not save" in output[output_before_direct_edit:]:
                    raise AssertionError("temporary chapter prose autosave reported a conflict")
                navigation_start = len(output)
                os.write(master, b"\x1b")  # return to outline
                if not wait_for_screen_fragments((b"Outline",), navigation_start, timeout=3.0):
                    raise AssertionError("Escape did not return from temporary chapter prose")
                if not move_outline_selection_until(
                    lambda row: "[ ]" in row or "[x]" in row,
                    "down",
                ):
                    raise AssertionError(
                        "outline did not expose a scene after direct-prose editing; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
                os.write(master, b"\r")
                scene_open_markers = (
                    (expected_text.encode("utf-8"),) if expected_text else (b"Editor",)
                )
                if not wait_for_screen_fragments(
                    scene_open_markers,
                    output_before_direct_edit,
                    timeout=5.0,
                ):
                    raise AssertionError(
                        "scene row or text changed after temporary prose autosave; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
            else:
                os.write(master, b"\r")  # open the first imported scene
                editor_markers = (
                    (expected_text.encode("utf-8"),) if expected_text else (b"Editor",)
                )
                if not wait_for_screen_fragments(editor_markers, timeout=5.0):
                    raise AssertionError(
                        "imported manuscript did not open in the editor; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
            os.write(master, b"Additional UAT sentence.")
            os.write(master, b"\x13")  # Ctrl+S
            if not wait_for_screen_fragments((b"Save Project As", b"Folder contents"), timeout=5.0):
                raise AssertionError(
                    "first explicit Save did not open the permanent-location browser; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\x1b[15~")  # F5: use the proposed permanent project path
            if not wait_for_screen_fragments((b"F5 Confirm Save Copy",), timeout=3.0):
                raise AssertionError(
                    "Save As did not show its no-overwrite confirmation; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\x1b[15~")
            if not wait_for_screen_fragments((b"Saved project at",), timeout=10.0):
                raise AssertionError(
                    "edited import was not promoted to a permanent project; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\x1b")  # return to the manuscript outline
            if not wait_for_screen_fragments(
                (b"Outline", b"Selected outline item"), timeout=5.0
            ):
                raise AssertionError(
                    "editor did not return to the imported outline; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            if direct_prose_workflow:
                # After first Save, revisit both persisted documents and
                # verify the staged autosave survived promotion.
                if not move_outline_selection_to_manuscript_root():
                    raise AssertionError("could not navigate to the top of the saved outline")
                if not move_outline_selection_until(
                    lambda row: body_row_marker in row,
                    "down",
                ):
                    raise AssertionError("saved outline lost its direct-prose row")
                output_before_direct_edit = len(output)
                os.write(master, b"\r")
                saved_prose_marker = os.environ.get(
                    "WIDE_PTY_DIRECT_PROSE_SAVED_MARKER",
                    "Edited Chapter prose survives direct editing",
                )
                if not wait_for_screen_fragments(
                    (saved_prose_marker.encode("utf-8"),),
                    output_before_direct_edit,
                    timeout=5.0,
                ):
                    raise AssertionError(
                        "staged chapter-prose autosave did not survive first Save; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
                navigation_start = len(output)
                os.write(master, b"\x1b")  # return to outliner
                if not wait_for_screen_fragments((b"Outline",), navigation_start, timeout=3.0):
                    raise AssertionError("Escape did not return from reopened chapter prose")
                if not move_outline_selection_until(
                    lambda row: "[ ]" in row or "[x]" in row,
                    "down",
                ):
                    raise AssertionError("saved outline lost its scene row")
                os.write(master, b"\r")
                saved_scene_markers = [b"Additional UAT sentence."]
                saved_scene_markers.append(
                    expected_text.encode("utf-8") if expected_text else b"Editor"
                )
                if not wait_for_screen_fragments(
                    tuple(saved_scene_markers),
                    output_before_direct_edit,
                    timeout=5.0,
                ):
                    raise AssertionError(
                        "scene row disappeared or scene text changed after direct-prose autosave; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
            if os.environ.get("WIDE_PTY_SKIP_COMPILE") == "1":
                os.write(master, b"\x11")  # Ctrl+Q after persistence checks
                quit_deadline = time.monotonic() + 8.0
                finished_pid = 0
                status = 0
                while finished_pid == 0 and time.monotonic() < quit_deadline:
                    read_for(0.1)
                    finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
                if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                    raise AssertionError("application did not exit cleanly after persistence checks")
                print("PTY Scrivener persistence workflow passed: import → direct prose edit/autosave → scene navigation → promotion → reopen; compile deliberately skipped.")
                return
            menu_start = len(output)
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            if os.environ.get("WIDE_PTY_COMPILE_PROFILE") == "1":
                for _ in range(10):
                    if wait_for_screen_fragments(
                        (b"Compile with Imported Layout",), menu_start, timeout=0.2
                    ):
                        break
                    os.write(master, b"\x1b[C")
                    read_for(0.05)
                else:
                    raise AssertionError(
                        "Publish menu did not expose imported Scrivener layouts; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                        + "\nvisible output tail: "
                        + repr(ANSI_CONTROL.sub(b"", output[-3000:]))
                    )
                select_visible_menu_item("Compile with Imported Layout")
                os.write(master, b"\r")
                if not wait_for_screen_fragments(
                    (b"Choose Scrivener Layout", b"Default"), menu_start, timeout=5.0
                ):
                    raise AssertionError("imported Scrivener layout chooser was not shown")
                os.write(master, b"\r")  # select the source's default layout
            else:
                selected_publish = False
                for _ in range(10):
                    if wait_for_screen_fragments((b"Compile / Export",), menu_start, timeout=0.2):
                        selected_publish = True
                        break
                    os.write(master, b"\x1b[C")
                    read_for(0.05)
                if not selected_publish:
                    raise AssertionError("Publish menu did not expose Compile / Export")
                select_visible_menu_item("Compile / Export")
                os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Selected path:", b".docx", b"F5 Compile"), timeout=5.0):
                raise AssertionError("imported project did not reach the publication file browser")
            os.write(master, b"\x1b[15~")
            if not wait_for_screen_fragments((b"Compiled",), timeout=15.0):
                raise AssertionError("imported manuscript did not compile successfully")
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 8.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after importing and compiling")
            print("PTY Scrivener workflow passed: menu → source browser → automatic staging import → edit → first Save promotion → compile → exit.")
            return

        if review_menu:
            os.write(master, b"\x1b")  # dismiss the visible first-run welcome
            read_for(0.25)
            menu_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.2)
            found_review = False
            for _ in range(10):
                if wait_for_screen_fragments((b"Spellcheck Manuscript",), menu_start, timeout=0.15):
                    found_review = True
                    break
                os.write(master, b"\x1b[C")
                read_for(0.1)
            if not found_review:
                raise AssertionError("Review menu did not expose Spellcheck Manuscript")
            # Menu viewports show a subset of a long command group at once.
            # Walk the selectable entries just as a keyboard user would; do
            # not require every command to fit in the initial viewport.
            select_visible_menu_item("Thesaurus" if review_thesaurus else "Spellcheck Manuscript")
            os.write(master, b"\r")
            if review_thesaurus:
                if not wait_for_screen_fragments((b"thesaurus ",), menu_start, timeout=2.0):
                    raise AssertionError("Review > Thesaurus did not open its word prompt")
                os.write(master, b"colour\r")
                if not wait_for_screen_fragments((b"Thesaurus", b"hue", b"pigment"), menu_start, timeout=5.0):
                    raise AssertionError(
                        "offline thesaurus results did not appear in the reference pane; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
                os.write(master, b"\x11")
                quit_deadline = time.monotonic() + 5.0
                finished_pid = 0
                status = 0
                while finished_pid == 0 and time.monotonic() < quit_deadline:
                    read_for(0.1)
                    finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
                if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                    raise AssertionError("application did not exit cleanly after offline thesaurus test")
                print("PTY Review workflow passed: offline thesaurus opened from its menu and showed local results.")
                return
            if not wait_for_screen_fragments((b"spellcheck",), menu_start, timeout=2.0):
                raise AssertionError("Review > Spellcheck did not open its command prompt")
            os.write(master, b"\x1b")
            read_for(0.2)
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after Review menu test")
            print("PTY Review workflow passed: spellcheck and grammar actions are visible and keyboard-selectable.")
            return

        if tutorial_navigation:
            if not wait_for_screen_fragments(
                (b"Welcome to Writer's IDE", b"Guided Tour", b"Quick Start"), timeout=10.0
            ):
                raise AssertionError("first-run welcome did not present the tutorial choices")
            # New Project, Open Project, Recent Projects, Import, Guided Tour.
            os.write(master, b"\x1b[B" * 4 + b"\r")
            if not wait_for_screen_fragments(
                (b"Tutorial", b"1/6", b"Press F10 to open the menu"), timeout=10.0
            ):
                raise AssertionError("Guided Tour did not open its first actionable lesson")

            # F6 is assigned to Characters globally; the focused tutorial must
            # retain the key and keep the same step and task visible.
            os.write(master, b"\x1b[17~")
            read_for(0.2)
            if not wait_for_screen_fragments(
                (b"Tutorial", b"1/6", b"0/9 verified", b"Press F10 to open the menu"),
                timeout=3.0,
            ):
                raise AssertionError("F6 took focus away from the active tutorial lesson")

            # Complete the visible route: F10 → Navigate → Outline → scene.
            menu_start = len(output)
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            os.write(master, b"\x1b[C" * 3)
            selected_outline = False
            for _ in range(16):
                if wait_for_screen_fragments(("▸ Outline".encode(),), menu_start, timeout=0.12):
                    selected_outline = True
                    break
                os.write(master, b"\x1b[B")
                read_for(0.05)
            if not selected_outline:
                raise AssertionError("Navigate did not visibly select Outline during the tutorial")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"A Sample Chapter", b"The First Page"), timeout=5.0
            ):
                raise AssertionError("tutorial did not navigate to its sample outline")
            os.write(master, b"\x1b[B\r")
            if not wait_for_screen_fragments((b"Mara opened the old book",), timeout=5.0):
                raise AssertionError("tutorial sample scene did not open from the outline")
            if not wait_for_workspace_title("Editor"):
                raise AssertionError("tutorial scene selection did not open the manuscript editor")
            # Send prose at a human typing cadence. A single PTY write can
            # coalesce many key events and make the application-under-test
            # drop the tail under load, which tests the PTY's burst handling
            # rather than an author's normal editing workflow.
            os.write(master, b"\n")
            read_for(0.05)
            for character in "First tutorial sentence.":
                os.write(master, character.encode("utf-8"))
                read_for(0.015)
            if not wait_for_screen_fragments(
                (b"6/9 verified", b"Save your writing"), timeout=3.0
            ):
                raise AssertionError(
                    "tutorial did not verify the actual sample-scene edit; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            if not wait_for_screen_fragments((b"First tutorial sentence.",), timeout=3.0):
                raise AssertionError(
                    "tutorial stopped accepting text after the first character; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\x13")  # Ctrl+S
            if not wait_for_screen_fragments(
                (b"7/9 verified", b"Press Esc to return to Outline"), timeout=3.0
            ):
                raise AssertionError("tutorial did not verify the saved sample-scene edit")
            os.write(master, b"\x1b")  # return to Outline, as the lesson requests
            if not wait_for_workspace_title("Outline"):
                raise AssertionError("Esc did not return to the tutorial outline")
            read_for(0.2)

            # Resume from the visible Help menu and advance only after the
            # required writing, save, return, and resume states are verified.
            os.write(master, b"\x1b")  # pause the tutorial before opening Help
            read_for(0.3)
            rows = terminal_snapshot(output, 24, 80)
            if any("Tutorial · Find your way around" in row for row in rows):
                raise AssertionError(
                    "Escape did not hide the tutorial panel; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            if not any("Resume: Help > Resume Tutorial" in row for row in rows):
                raise AssertionError("paused tutorial footer did not show its Help resume route")
            os.write(master, b"\x1b[21~")
            if not wait_for_screen_fragments((b"New Project", b"Quit"), timeout=3.0):
                raise AssertionError(
                    "F10 did not open the File menu while the tutorial was active; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            open_help_menu(already_open=True)
            select_visible_menu_item("Resume Tutorial")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Navigation lesson complete", b"Enter continues"), timeout=5.0
            ):
                raise AssertionError("Help > Resume Tutorial did not restore the completed lesson")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Tutorial", b"2/6", b"Show Scene Cards from View"), timeout=5.0
            ):
                raise AssertionError("Enter did not advance the tutorial to lesson 2/6")

            # Pause and resume through the same visible command; the exact
            # lesson number and pending action must be restored.
            os.write(master, b"\x1b")
            read_for(0.2)
            open_help_menu()
            select_visible_menu_item("Resume Tutorial")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Tutorial", b"2/6", b"Show Scene Cards from View"), timeout=10.0
            ):
                raise AssertionError(
                    "tutorial resume did not restore lesson 2 and its pending task; current screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\x1b[17;2~")  # Shift+F6 must not switch workspace either
            if not wait_for_screen_fragments((b"Tutorial", b"2/6"), timeout=3.0):
                raise AssertionError("Shift+F6 took focus away from the resumed tutorial")

            os.write(master, b"\x1b")  # pause before the ordinary application quit command
            read_for(0.2)
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit after the tutorial pause")
            print("PTY tutorial passed: first-run navigation, editing, saving, Help resume and lesson restoration work.")
            return
        if scene_rename:
            if wait_for_screen_fragments((b"Welcome to Writer's IDE",), timeout=1.0):
                os.write(master, b"\x1b")  # skip first-run welcome for this existing project
            if not wait_for_screen_fragments((b"Original scene body",), timeout=10.0):
                raise AssertionError("the selected scene did not open in the manuscript editor")
            if not wait_for_workspace_title("Editor"):
                raise AssertionError("the scene did not open in the manuscript editor")
            os.write(master, b"\x1b")  # return to the originating manuscript outline
            if not wait_for_screen_fragments((b"Included", b"Excluded"), timeout=3.0):
                raise AssertionError("Esc did not restore the originating Scene Cards view")
            show_outline_tree_presentation()
            if not wait_for_workspace_title("Outline"):
                raise AssertionError("the View menu did not open the hierarchical Outline")
            menu_start = open_outline_menu()
            if not wait_for_screen_fragments((b"Rename Selected Scene",), timeout=3.0):
                raise AssertionError(
                    "Outline menu did not expose Rename Selected Scene; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            select_visible_menu_item("Rename Selected Scene")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Original Scene", b"Ctrl+A clears title", b"Enter saves"), timeout=3.0
            ):
                raise AssertionError(
                    "rename dialogue did not show and focus the current scene title; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\x01The Returning Light\r")  # Ctrl+A, replace title, save
            if not wait_for_screen_fragments((b"The Returning Light",), timeout=3.0):
                raise AssertionError("renamed scene title was not visible in the outline")
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit after the scene rename")
            print("PTY scene rename passed: Outline menu opened a prefilled editable title and saved it.")
            return
        if heading_rename:
            if wait_for_screen_fragments((b"Welcome to Writer's IDE",), timeout=1.0):
                os.write(master, b"\x1b")
            if not wait_for_screen_fragments((b"Original scene body",), timeout=10.0):
                raise AssertionError("the selected scene did not open in the manuscript editor")
            os.write(master, b"\x1b")
            if not wait_for_screen_fragments((b"Included", b"Excluded"), timeout=3.0):
                raise AssertionError("Esc did not restore the originating Scene Cards view")
            show_outline_tree_presentation()
            if not wait_for_workspace_title("Outline"):
                raise AssertionError("the View menu did not open the manuscript Outline")
            os.write(master, b"\x1b[A")  # select the chapter heading above the scene
            read_for(0.3)
            open_outline_menu()
            select_visible_menu_item("Rename Selected Section or Chapter")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Old Chapter", b"Ctrl+A clears title", b"Enter saves"), timeout=3.0
            ):
                raise AssertionError("heading rename did not show its prefilled title field")
            rename_start = len(output)
            os.write(
                master,
                b"\x01" + "Part One · The Returning Light\r".encode("utf-8"),
            )
            if not wait_for_screen_fragments(
                (b"Part One", b"The Returning Light"), rename_start, timeout=3.0
            ):
                raise AssertionError(
                    "renamed chapter was not visible in the outline; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit after the heading rename")
            print("PTY heading rename passed: Outline menu saved a prefilled section or chapter heading.")
            return
        if outline_view:
            os.write(master, b"\x1b")  # dismiss first-run welcome
            read_for(0.25)
            menu_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.2)
            os.write(master, b"\x1b[C" * 4)  # select View
            if not wait_for_screen_fragments((b"Show Scene Cards",), menu_start, timeout=3.0) and not wait_for_screen_fragments((b"Show Outline Tree",), menu_start, timeout=3.0):
                raise AssertionError("View menu did not expose the active outline presentation command")
            first_view_label = "Show Scene Cards" if "Show Scene Cards" in "".join(terminal_snapshot(output, 24, 80)) else "Show Outline Tree"
            os.write(master, b"\r")
            read_for(0.25)
            menu_start = len(output)
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            os.write(master, b"\x1b[C" * 4)
            expected_view_label = "Show Outline Tree" if first_view_label == "Show Scene Cards" else "Show Scene Cards"
            if not wait_for_screen_fragments((expected_view_label.encode("utf-8"),), menu_start, timeout=3.0):
                raise AssertionError("View menu label did not change after switching outline presentation")
            os.write(master, b"\x1b")  # close the View menu before exiting
            read_for(0.2)
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after switching outline presentations; visible tail: " + repr(ANSI_CONTROL.sub(b"", output[-1200:])))
            print("PTY unified-outline workflow passed: View menu switches between tree and scene-card presentations.")
            return

        if outline_enter:
            if not wait_for_screen_fragments(
                (b"Guided Tour", b"Quick Start"), timeout=10.0
            ):
                raise AssertionError("the first-run welcome screen did not become ready")
            os.write(master, b"\x1b")  # dismiss the visible first-run welcome screen
            read_for(0.25)

            menu_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.2)
            os.write(master, b"\x1b[C" * 3)  # Navigate
            selected_outline = False
            for _ in range(16):
                if wait_for_screen_fragments(("▸ Outline".encode(),), menu_start, timeout=0.15):
                    selected_outline = True
                    break
                os.write(master, b"\x1b[B")
                read_for(0.1)
            if not selected_outline:
                raise AssertionError("Navigate did not expose the selected Outline command")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Part I", b"Chapter 1", b"Opening"), menu_start, timeout=5.0
            ):
                raise AssertionError("the hierarchical outline did not render its seeded nodes")

            # The tree is Section > Chapter > Scene. Move to the section and
            # verify Enter opens its first manuscript descendant.
            os.write(master, b"\x1b[A" * 2)
            read_for(0.2)
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Opening paragraph remains editable.",), timeout=5.0
            ):
                raise AssertionError("Enter on a section did not open its first scene")

            os.write(master, b"\x1b")
            if not wait_for_screen_fragments(
                (b"Part I", b"Chapter 1", b"Opening"), timeout=5.0
            ):
                raise AssertionError("returning from the editor did not restore the outline")
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after Outline navigation")
            print("PTY Outline workflow passed: Enter opens a section descendant and restores the selected hierarchy.")
            return

        if insert_edit_actions:
            os.write(master, b"\x1b")  # dismiss the first-run welcome/tutorial panel
            read_for(0.25)

            date_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.2)
            os.write(master, b"\x1b[C" * 2)  # Insert
            select_visible_menu_item("Insert Date and Time")
            os.write(master, b"\r")
            if not wait_for_fragments(
                (b"Inserted", b"UTC"), date_start, timeout=3.0
            ):
                raise AssertionError(
                    "Insert > Insert Date/Time did not run through the menu; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )

            file_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.2)
            os.write(master, b"\x1b[C" * 2)  # Insert
            select_visible_menu_item("Insert File…")
            os.write(master, b"\r")
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

        if removed_scenes:
            os.write(master, b"\x1b")  # dismiss the first-run welcome/tutorial panel
            read_for(0.25)
            show_scene_cards_presentation()
            menu_start = open_outline_menu()
            select_visible_menu_item("Show Removed Scenes")
            os.write(master, b"\r")
            if not wait_for_fragments((b"Noremovedscenes", b"ReturntoOutline"), menu_start, timeout=3.0):
                raise AssertionError(
                    "Removed Scenes did not show its specific empty state; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            return_start = len(output)
            os.write(master, b"\x1b")  # Esc returns to the unified Outline
            if not wait_for_fragments((b"Included", b"Arrival"), return_start, timeout=3.0):
                raise AssertionError(
                    "Esc did not restore Scene Cards and its prior selection; visible tail: "
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
                raise AssertionError("application did not exit cleanly after returning from Removed Scenes")
            print("PTY Removed Scenes workflow passed: Outline menu, empty-state guidance, Esc return and selection restoration.")
            return

        if special_character:
            os.write(master, b"\x1b")  # dismiss the first-run welcome/tutorial panel
            read_for(0.25)
            picker_start = len(output)
            os.write(master, b"\x1b[21~" + b"\x1b[C" * 2)
            select_visible_menu_item("Special Character…")
            os.write(master, b"\r")
            if not wait_for_fragments(
                (b"Insert Special Character", b"U+XXXX"), picker_start, timeout=3.0
            ):
                raise AssertionError(
                    "Insert > Special Character did not open the searchable picker; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"U+2014\r")
            if not wait_for_screen_fragments(("—The opening paragraph.".encode(),), picker_start, timeout=3.0):
                raise AssertionError(
                    "The U+2014 code point was not visible in the manuscript; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"\x13")  # Ctrl+S
            if not wait_for_screen_fragments((b"[SAVED]",), picker_start, timeout=3.0):
                raise AssertionError("special-character insertion did not report a saved state")
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after special-character insertion")
            print("PTY special-character workflow passed: Insert menu, Unicode code point, save, and quit.")
            return

        if named_layouts:
            os.write(master, b"\x1b")  # dismiss the first-run welcome/tutorial panel
            read_for(0.25)

            save_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.2)
            os.write(master, b"\x1b[C" * 4)  # View
            select_visible_menu_item("Save Layout")
            os.write(master, b"\r")
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
            os.write(master, b"\x1b[C" * 4)  # View
            select_visible_menu_item("Load Layout")
            os.write(master, b"\r")
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
            os.write(master, b"\x1b[B" * 10 + b"\r")  # Edit > Select All
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
            os.write(master, b"\x1b[C" + b"\x1b[B" * 11 + b"\r")  # Edit > Duplicate Selection or Paragraph
            if not wait_for_screen_fragments((b"Duplicated selection",), edit_start, timeout=3.0):
                raise AssertionError("Edit > Duplicate Selection did not dispatch from writing mode")
            os.write(master, b"\x1b[21~")
            read_for(0.15)
            os.write(master, b"\x1b[C\r")  # Edit > Undo
            if not wait_for_screen_fragments((b"Edit undone",), edit_start, timeout=3.0):
                raise AssertionError("Edit > Undo did not reverse selection duplication")
            os.write(master, b"\x1b[21~")
            read_for(0.15)
            os.write(master, b"\x1b[C" + b"\x1b[B" * 12 + b"\r")  # Edit > Select Word
            if not wait_for_screen_fragments(
                (b"Selected word",), edit_start, timeout=3.0
            ):
                raise AssertionError("Edit > Select Word did not dispatch from writing mode")

            os.write(master, b"\x1b[21~")
            read_for(0.15)
            os.write(master, b"\x1b[C" + b"\x1b[B" * 13 + b"\r")  # Edit > Select Sentence
            if not wait_for_screen_fragments(
                (b"Selected sentence",), edit_start, timeout=3.0
            ):
                raise AssertionError("Edit > Select Sentence did not dispatch from writing mode")

            os.write(master, b"\x1b[21~")
            read_for(0.15)
            os.write(master, b"\x1b[C" + b"\x1b[B" * 14 + b"\r")  # Edit > Select Paragraph
            if not wait_for_screen_fragments(
                (b"Selected paragraph",), edit_start, timeout=3.0
            ):
                raise AssertionError(
                    "Edit > Select Paragraph did not dispatch from writing mode"
                )
            join_start = len(output)
            os.write(master, b"\x1b[21~")
            read_for(0.15)
            os.write(master, b"\x1b[C" + b"\x1b[B" * 9 + b"\r")  # Edit > Join Adjacent Paragraphs
            if not wait_for_screen_fragments(
                (b"Paragraphsjoined", b"Asecondparagraphwaits."),
                join_start,
                timeout=3.0,
            ):
                raise AssertionError("Edit > Join Adjacent Paragraphs did not merge the current paragraph")
            os.write(master, b"\x1b[21~")
            read_for(0.15)
            os.write(master, b"\x1b[C\r")  # Edit > Undo
            if not wait_for_screen_fragments(
                (b"Editundone", b"Asecondparagraphwaits."),
                join_start,
                timeout=3.0,
            ):
                raise AssertionError("Edit > Undo did not restore the separate paragraph")
            os.write(master, b"\x1b[21~")
            read_for(0.15)
            os.write(master, b"\x1b[C" + b"\x1b[B" * 9 + b"\r")
            if not wait_for_screen_fragments(
                (b"Paragraphsjoined", b"Asecondparagraphwaits."),
                join_start,
                timeout=3.0,
            ):
                raise AssertionError("Edit > Join Adjacent Paragraphs did not work after Undo")
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
            print("PTY Edit-menu workflow passed: selection, duplication, paragraph joining, Undo, save and reopen dispatch in writing mode.")
            return

        if date_time_insertion:
            insertion_start = len(output)
            os.write(master, b"\x1b[21~")  # F10 opens the menu bar.
            read_for(0.2)
            os.write(master, b"\x1b[C" * 2)  # Insert menu.
            select_visible_menu_item("Insert Date and Time")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Inserted date and time", b"The clock struck noon."),
                insertion_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "Insert > Date and Time did not both report success and render inserted text; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\x13")  # Ctrl+S
            if not wait_for_screen_fragments((b"[SAVED]",), insertion_start, timeout=3.0):
                raise AssertionError("date/time insertion did not save through the editor")
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("the application did not exit cleanly after date/time insertion")
            print("PTY date/time workflow passed: Insert menu action changed the active scene and saved successfully.")
            return

        if date_time_unavailable:
            # A new empty project has no manuscript target. Let the normal
            # delayed-autosave interval elapse and verify startup did not
            # manufacture a dirty editor or an external-change conflict.
            os.write(master, b"\x1b")  # dismiss first-run help, if present
            read_for(0.2)
            read_for(2.2)
            initial_screen = terminal_snapshot(output, 24, 80)
            if any("conflict" in row.lower() or "[save failed]" in row.lower() for row in initial_screen):
                raise AssertionError(
                    "an idle empty project displayed a spurious save conflict; screen: "
                    + repr(initial_screen)
                )
            if any("Editor" in row and "No scene selected" in row for row in initial_screen):
                raise AssertionError(
                    "an empty project opened an unattached editor instead of its outline; screen: "
                    + repr(initial_screen)
                )
            insertion_start = len(output)
            os.write(master, b"\x1b[21~")  # F10 opens the menu bar.
            read_for(0.2)
            os.write(master, b"\x1b[C" * 2)  # Insert menu.
            select_visible_menu_item("Insert Date and Time")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Insert Date and Time unavailable", b"Open a manuscript document first"),
                insertion_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "date/time insertion without a document did not explain why it was unavailable; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            if b"Inserted date and time" in ANSI_CONTROL.sub(b"", output[insertion_start:]):
                raise AssertionError("WIDE claimed to insert date/time without an active document")
            os.write(master, b"\x1b")  # close menu without changing the project
            read_for(0.2)
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit after the unavailable insertion test")
            print("PTY date/time precondition passed: no document means a clear unavailable response and no mutation.")
            return

        if find_replace_focus:
            search_start = len(output)
            os.write(master, b"\x1b[21~")  # F10 opens the menu bar.
            read_for(0.2)
            os.write(master, b"\x1b[C")  # Edit menu.
            select_visible_menu_item("Find and Replace")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Find: ", b"Replace with:", b"Find: \xe2\x96\x8f"),
                search_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "Find and Replace did not show the focused Find caret; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\t")  # replacement is editable before any search runs
            if not wait_for_screen_fragments((b"Replace with: \xe2\x96\x8f",), search_start, timeout=3.0):
                raise AssertionError("the empty replacement field did not show its insertion caret")
            os.write(master, b"fox")
            if not wait_for_screen_fragments((b"Replace with: fox\xe2\x96\x8f",), search_start, timeout=3.0):
                raise AssertionError("typing before a search did not update the replacement field")
            os.write(master, b"\tcat")  # move to Find, then enter the search term
            if not wait_for_screen_fragments((b"Find: cat\xe2\x96\x8f",), search_start, timeout=3.0):
                raise AssertionError("Tab did not focus Find and place its caret after typed text")
            os.write(master, b"\x1b")  # close without searching or modifying the manuscript
            if not wait_for_screen_fragments((b"The red fox waits.",), search_start, timeout=3.0):
                raise AssertionError("closing Find and Replace did not restore the manuscript")
            read_for(0.2)
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit after the Find and Replace focus test")
            print("PTY Find and Replace focus passed: empty fields show a caret, replacement-first entry works, and Esc preserves prose.")
            return

        if chapter_creation:
            os.write(master, b"\x1b")  # dismiss the first-run welcome/tutorial panel
            read_for(0.3)

            # Open the unified hierarchical Outline from the Navigate menu.
            show_outline_tree_presentation()

            def create_chapter(title):
                menu_start = len(output)
                os.write(master, b"\x10")  # Ctrl+P: searchable command palette
                if not wait_for_screen_fragments((b"Search commands",), menu_start, timeout=3.0):
                    raise AssertionError("command palette did not open for chapter creation")
                os.write(master, b"New Chapter")
                if not wait_for_screen_fragments((b"New Chapter",), menu_start, timeout=3.0):
                    raise AssertionError("command palette did not find New Chapter")
                os.write(master, b"\r")
                if not wait_for_screen_fragments((b"New chapter", b"Enter creates"), menu_start, timeout=3.0):
                    raise AssertionError(
                        "Insert > New Chapter did not open its title prompt; output tail: "
                        + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                    )
                prompt_start = len(output)
                os.write(master, title.encode("utf-8") + b"\r")
                if not wait_for_fragments(
                    (title.encode("utf-8"), b"0scene(s)"),
                    prompt_start,
                    timeout=3.0,
                ):
                    raise AssertionError(
                        f"created chapter {title!r} was not rendered with its empty count in Outline; output tail: "
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

        if authoring_audit:
            os.write(master, b"\x1b")  # dismiss first-run onboarding
            read_for(0.25)
            show_outline_tree_presentation()

            def outline_command(label, text=None):
                start = len(output)
                open_outline_menu()
                if not wait_for_screen_fragments((label.encode("utf-8"),), start, timeout=3.0):
                    raise AssertionError(f"Outline menu did not show {label!r}")
                select_visible_menu_item(label)
                os.write(master, b"\r")
                if text is not None:
                    prompt_marker = b"New scene" if label == "New Scene" else b"Enter creates"
                    if not wait_for_screen_fragments((prompt_marker,), start, timeout=3.0):
                        raise AssertionError(f"{label} did not open a clear title dialogue")
                    os.write(master, text.encode("utf-8") + b"\r")
                    if not wait_for_screen_fragments((text.encode("utf-8"),), start, timeout=3.0):
                        raise AssertionError(f"{label} did not reveal the created item")

            outline_command("New Section", "Part One")
            outline_command("New Chapter", "Chapter One")
            outline_command("New Scene", "Opening")
            if not wait_for_screen_fragments((b"[Included]", b"Opening"), timeout=3.0):
                raise AssertionError("unified outline did not display the scene inclusion state")
            toggle_start = len(output)
            os.write(master, b" ")
            if not wait_for_screen_fragments((b"[Excluded]", b"Opening"), toggle_start, timeout=3.0):
                raise AssertionError("Space did not visibly move the scene to Excluded")
            os.write(master, b" ")
            if not wait_for_screen_fragments((b"[Included]", b"Opening"), toggle_start, timeout=3.0):
                raise AssertionError("Space did not visibly restore the scene to Included")
            outline_command("New Scene", "Discovery")

            os.write(master, b"\x1b[A")  # select Opening
            edit_start = len(output)
            os.write(master, b"\r")
            if not wait_for_workspace_title("Editor", timeout=3.0):
                raise AssertionError("Enter did not open the selected scene")
            read_for(0.15)
            # Send text like a person rather than one burst: bulk PTY writes
            # can coalesce terminal key events and drop the tail under load.
            for character in "The map revealed a hidden pass.":
                os.write(master, character.encode("utf-8"))
                read_for(0.015)
            if not wait_for_screen_fragments((b"hidden pass",), edit_start, timeout=3.0):
                raise AssertionError("manuscript typing was not visible; screen: " + repr(terminal_snapshot(output, 24, 80)) + "\noutput tail: " + repr(ANSI_CONTROL.sub(b"", output[-1600:])))
            os.write(master, b"\x13")  # Ctrl+S
            if not wait_for_screen_fragments((b"[SAVED]",), edit_start, timeout=3.0):
                raise AssertionError("scene save did not provide visible success feedback")
            return_start = len(output)
            os.write(master, b"\x1b")
            if not wait_for_screen_fragments((b"[Included]", b"Opening"), return_start, timeout=3.0):
                raise AssertionError("Esc did not return to the active scene in the outline")

            link_start = len(output)
            os.write(master, b"\x1b[21~" + b"\x1b[C" * 2)
            select_visible_menu_item("Create Story Link")
            os.write(master, b"\rOpening\rDiscovery\rrelated\r\r")
            if not wait_for_screen_fragments((b"Created related relationship", b"Scene", b"Opening", b"Discovery"), link_start, timeout=5.0):
                raise AssertionError("searchable story-link selection did not create a named link; screen: " + repr(terminal_snapshot(output, 24, 80)))
            type_start = len(output)
            os.write(master, b"\x1b[21~")  # F10 > File
            if not wait_for_screen_fragments((b"Book Type",), type_start, timeout=3.0):
                raise AssertionError("File menu did not expose Book Type")
            select_visible_menu_item("Book Type")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Choose the kind of book", b"Fiction", b"Non-fiction"), type_start, timeout=3.0):
                raise AssertionError("Book Type did not open a visible choice dialogue")
            os.write(master, b"\x1b[B\r")  # Fiction -> Non-fiction
            if not wait_for_screen_fragments((b"Book type set to Non-fiction",), type_start, timeout=3.0):
                raise AssertionError("Book Type did not confirm its selection")
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after authoring audit")
            print("PTY authoring audit passed: menu-driven structure, scene inclusion feedback, editing/save, identity-based outline return, searchable story links and editable book type.")
            return

        if reference_removal:
            # Navigate > Reference Library and confirm the read-only source-aware
            # removal dialogue. The source file itself must remain untouched.
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            os.write(master, b"\x1b[C" * 3 + b"\x1b[B" * 2 + b"\r")
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

        if create_fiction or create_nonfiction:
            read_for(0.5)
            if not wait_for_screen_fragments((b"No scenes yet",), timeout=3.0):
                raise AssertionError("welcome panel did not reveal the starting project")
            menu_start = len(output)
            os.write(master, b"\x1b[21~")  # F10, File menu
            read_for(0.2)
            select_visible_menu_item("New Project")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"New Project", b"Fiction"), menu_start, timeout=3.0):
                raise AssertionError("File > New Project did not open its title/type dialogue")
            browser_deadline = time.monotonic() + 5.0
            while (
                (b"New Project" not in output or b"Fiction" not in output)
                and time.monotonic() < browser_deadline
            ):
                read_for(0.25)
            if b"New Project" not in output or b"Fiction" not in output:
                raise AssertionError("New Project did not open the compact title/type dialogue")
            if b"Directory tree" in ANSI_CONTROL.sub(b"", output[-1200:]):
                raise AssertionError("New Project unexpectedly displayed a directory browser")
            # Select Non-fiction when requested, then create directly into
            # managed temporary storage without a location picker.
            if create_nonfiction:
                os.write(master, b"\t\x1b[C\t\r")
            else:
                os.write(master, b"\t\t\r")
            expected_project_markers = (b"Created",) if create_nonfiction else (b"Created My Novel", b"Included", b"Excluded")
            if not wait_for_screen_fragments(expected_project_markers, timeout=5.0):
                raise AssertionError(
                    "Enter on Create Project did not open the new fiction project; output tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-2400:]))
                )
            exited_pid, exited_status = os.waitpid(child_pid, os.WNOHANG)
            if exited_pid == child_pid:
                raise AssertionError(
                    f"application exited after creating the staged project with status {exited_status}; "
                    + repr(ANSI_CONTROL.sub(b"", output[-1200:]))
                )
            read_for(0.3)
            menu_start = len(output)
            os.write(master, b"\x10")  # Ctrl+P: searchable command palette
            if not wait_for_screen_fragments((b"Search commands",), menu_start, timeout=3.0):
                raise AssertionError(
                    "command palette did not open for chapter creation; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"New Chapter")
            if not wait_for_screen_fragments((b"New Chapter",), menu_start, timeout=3.0):
                raise AssertionError("command palette did not find New Chapter")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"New chapter", b"Enter creates"), menu_start, timeout=3.0):
                raise AssertionError("New Chapter did not open its title prompt")
            os.write(master, b"Opening Chapter\r")
            if not wait_for_screen_fragments((b"Opening Chapter",), timeout=3.0):
                raise AssertionError("New Chapter was not shown in the outline")
            if create_nonfiction:
                menu_start = len(output)
                os.write(master, b"\x10")  # command palette: write chapter prose
                if not wait_for_screen_fragments((b"Search commands",), menu_start, timeout=3.0):
                    raise AssertionError("command palette did not open for chapter writing")
                os.write(master, b"Write Selected Chapter")
                if not wait_for_screen_fragments((b"Write Selected Chapter",), menu_start, timeout=3.0):
                    raise AssertionError("command palette did not find Write Selected Chapter")
                os.write(master, b"\r")
                if not wait_for_workspace_title("Editor"):
                    raise AssertionError(
                        "selected non-fiction chapter did not open for writing; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
                os.write(master, b"A chapter written without compulsory scenes.")
                edit_start = len(output)
                if not wait_for_screen_fragments((b"Autosaved",), edit_start, timeout=10.0):
                    raise AssertionError(
                        "non-fiction chapter prose did not autosave; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
                os.write(master, b"\x13")
                if not wait_for(b"Save Project As", edit_start):
                    raise AssertionError("first non-fiction Save did not request a permanent location")
                os.write(master, b"\x1b[15~")
                if not wait_for_screen_fragments((b"Confirm Save Copy",), timeout=3.0):
                    raise AssertionError("non-fiction Save As did not ask before creating its destination")
                os.write(master, b"\x1b[15~")
                if not wait_for_screen_fragments((b"Saved project at",), timeout=3.0):
                    raise AssertionError("non-fiction project was not promoted to permanent storage")
                os.write(master, b"\x11")
                quit_deadline = time.monotonic() + 5.0
                finished_pid = 0
                status = 0
                while finished_pid == 0 and time.monotonic() < quit_deadline:
                    read_for(0.1)
                    finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
                if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                    raise AssertionError("application did not exit cleanly after non-fiction writing")
                print("PTY non-fiction lifecycle passed: created a chapter-only project, wrote, autosaved and promoted it.")
                return
            menu_start = len(output)
            os.write(master, b"\x10")  # Ctrl+P: searchable command palette
            if not wait_for_screen_fragments((b"Search commands",), menu_start, timeout=3.0):
                raise AssertionError("command palette did not open for scene creation")
            os.write(master, b"New Scene")
            if not wait_for_screen_fragments((b"New Scene",), menu_start, timeout=3.0):
                raise AssertionError("command palette did not find New Scene")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"New scene",), menu_start, timeout=3.0):
                raise AssertionError("New Scene did not open its title prompt")
            os.write(master, b"Opening\r")
            if not wait_for_screen_fragments((b"Opening",), timeout=3.0):
                raise AssertionError("New Scene was not shown in the outline")
            read_for(0.3)
            os.write(master, b"\r")  # Open the selected scene
            read_for(0.4)
            os.write(master, b"First passage.")
            edit_start = len(output)
            if not wait_for_screen_fragments((b"Autosaved",), edit_start, timeout=5.0) and not wait_for_screen_fragments((b"[UNSAVED]",), edit_start, timeout=0.2):
                raise AssertionError(
                    "typed passage was neither autosaved nor visibly unsaved; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-2200:]))
                )
            save_request_offset = len(output)
            os.write(master, b"\x13")  # Ctrl+S
            if not wait_for(b"Save Project As", save_request_offset):
                raise AssertionError("first Ctrl+S did not open the permanent-location browser")
            os.write(master, b"\x1b[15~")
            if not wait_for_screen_fragments((b"Confirm Save Copy",), timeout=3.0):
                raise AssertionError(
                    "Save As did not request collision-safe confirmation; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-2200:]))
                )
            os.write(master, b"\x1b[15~")
            if not wait_for_screen_fragments((b"Saved project at",), timeout=3.0):
                raise AssertionError("temporary project did not promote to the chosen location")
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

        if continue_fiction:
            if wait_for_screen_fragments((b"Welcome to Writer's IDE",), timeout=1.0):
                os.write(master, b"\x1b")
            menu_start = len(output)
            os.write(master, b"\x10")  # Open Project through the shared command registry
            if not wait_for_screen_fragments((b"Search commands",), menu_start, timeout=3.0):
                raise AssertionError("Open Project could not be discovered after reopening the project")
            os.write(master, b"Open Project")
            if not wait_for_screen_fragments((b"Open Project",), menu_start, timeout=3.0):
                raise AssertionError("command palette did not find Open Project")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Open Project", b"Projects"), menu_start, timeout=5.0):
                raise AssertionError(
                    "Open Project did not show its folder browser; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            # Open the Projects pane and select the project folder. Open Project
            # treats a recognised WIDE directory as one project, not as a folder
            # whose internal files need to be browsed.
            os.write(master, b"\t\x1b[B\x1b[B\r")
            if not wait_for_screen_fragments((b"Opening Chapter", b"Opening"), timeout=10.0):
                raise AssertionError("Open Project did not enter the selected project workspace")
            # Explicitly route through the outliner and open the selected
            # manuscript item. Old terminal output can otherwise make a
            # historical scene preview look like the currently focused editor.
            show_outline_tree_presentation()
            os.write(master, b"\r")
            if not wait_for_workspace_title("Editor", timeout=5.0):
                raise AssertionError("reopened manuscript item did not open in the editor")
            if not wait_for_screen_fragments((b"First passage.",), timeout=5.0):
                raise AssertionError("reopened scene content was not visible for continued writing")
            os.write(master, b"\x1b[1;5F Continued.")  # Ctrl+End, then append at document end
            edit_start = len(output)
            if not wait_for_screen_fragments((b"Autosaved",), edit_start, timeout=5.0):
                raise AssertionError("continued writing was not autosaved after reopening")
            os.write(master, b"\x13")  # Ctrl+S: save the promoted project
            if not wait_for_screen_fragments((b"Saved",), edit_start, timeout=5.0):
                raise AssertionError("continued writing did not report a successful save")
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after continuing the project")
            print("PTY reopen passed: reopened the promoted project, continued writing, autosaved and saved.")
            return

        # A fresh isolated profile shows the welcome panel. Use its visible
        # Skip choice so the test follows the discoverable onboarding route.
        os.write(master, b"\x1b")  # use the welcome screen's visible Skip key
        read_for(0.4)
        if not modes:
            assert_sample_prose_visible(output, 24, 80, "initial 80x24 screen")
        if (
            not section_removal_delete
            and not section_removal_rehome
            and not chapter_removal
            and not date_time_unavailable
            and b"Arrival" not in output
        ):
            raise AssertionError(
                "the opening scene was not rendered in the PTY; output tail: "
                + repr(bytes(output[-1600:]))
            )

        if previous_position:
            # F10 opens the last focused menu root, which may vary by project
            # or key history. Find Navigate by its visible command instead of
            # assuming a fixed menu-bar index.
            status_start = len(output)
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            for _ in range(10):
                if wait_for_screen_fragments(
                    (b"Return to Previous Position",), status_start, timeout=0.2
                ):
                    break
                os.write(master, b"\x1b[C")
                read_for(0.1)
            selected_return = False
            for _ in range(12):
                if wait_for_screen_fragments(
                    ("▸ Return to Previous Position".encode(),), status_start, timeout=0.1
                ):
                    selected_return = True
                    break
                os.write(master, b"\x1b[B")
                read_for(0.1)
            if not selected_return:
                raise AssertionError("Navigate menu could not focus Return to Previous Position")
            os.write(master, b"\r")
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

        if external_conflict:
            manuscript_path = os.environ["WIDE_PTY_EXTERNAL_MANUSCRIPT"]
            replacement = manuscript_path + ".external-tmp"
            with open(replacement, "w", encoding="utf-8") as manuscript:
                manuscript.write("External editor version remains on disk.")
            os.replace(replacement, manuscript_path)
            os.write(master, b"Local unsaved draft.")
            conflict_start = len(output)
            if not wait_for_screen_fragments(
                (b"Autosave could not save", b"Local unsaved draft."),
                conflict_start,
                timeout=8.0,
            ):
                raise AssertionError(
                    "external edit was not reported while preserving the in-memory draft; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\x11")  # Ctrl+Q; conflict must not trap exit.
            if not wait_for_screen_fragments(
                (b"Quit Without Saving",), conflict_start, timeout=3.0
            ):
                raise AssertionError(
                    "the conflicted editor did not offer an explicit quit-without-saving choice; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"d")
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
                raise AssertionError("discarding conflicted in-memory edits did not exit cleanly")
            print("PTY conflict workflow passed: external disk version was preserved and unsaved edits could be discarded safely.")
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
                (b"PreviousPosition", b"Outline"),
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
                    (b"Opening", b"scene(s)"),
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
            if chapter_removal_undo_redo:
                undo_start = len(output)
                os.write(master, b"\x1b[21~\x1b[C\r")  # F10 > Edit > Undo
                if not wait_for_screen_fragments((b"Removedstructurerestored",), undo_start, timeout=3.0):
                    raise AssertionError(
                        "Edit > Undo did not restore the chapter outside the editor; visible tail: "
                        + repr(ANSI_CONTROL.sub(b"", output[-1600:]))
                    )
                redo_start = len(output)
                os.write(master, b"\x1b[21~\x1b[C\x1b[B\r")  # F10 > Edit > Redo
                if not wait_for_screen_fragments((b"Removedstructureredone",), redo_start, timeout=3.0):
                    raise AssertionError(
                        "Edit > Redo did not return the chapter to Removed Documents; visible tail: "
                        + repr(ANSI_CONTROL.sub(b"", output[-1600:]))
                    )
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
            # The outline restores selection on the owned prose row; move to
            # its section heading before testing structural removal.
            os.write(master, b"\x1b[A")
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
                    + "\n".join(terminal_snapshot(output, 24, 80))
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
            # Open the card presentation through View and use its recoverable
            # removal dialogue with the list Delete key.
            scene_workspace_start = len(output)
            show_scene_cards_presentation()
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
            os.write(master, b"\x1b[C" * 3 + b"\x1b[B" * 2 + b"\r")
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

        # Exercise both advertised New Project routes through terminal key
        # delivery. The menu mnemonic must win over the Navigate category's
        # N mnemonic, and Ctrl+N must dispatch the same command globally.
        os.write(master, b"\x1b[21~")  # F10 opens File
        read_for(0.2)
        os.write(master, b"n")
        if not wait_for_screen_fragments((b"Title", b"Create"), timeout=3.0):
            raise AssertionError(
                "File > n did not open the New Project dialogue; screen: "
                + repr(terminal_snapshot(output, 24, 80))
            )
        os.write(master, b"\x1b")  # cancel without creating a project
        read_for(0.3)
        assert_sample_prose_visible(output, 24, 80, "screen after cancelling New Project")

        os.write(master, b"\x0e")  # Ctrl+N
        if not wait_for_screen_fragments((b"Title", b"Create"), timeout=3.0):
            raise AssertionError("Ctrl+N did not open the New Project dialogue")
        os.write(master, b"\x1b")  # cancel without creating a project
        read_for(0.3)
        assert_sample_prose_visible(output, 24, 80, "screen after cancelling Ctrl+N")

        # Exercise an actual manuscript → imported reference → manuscript
        # transition. Both panes contain distinct synthetic prose so stale
        # text leakage is observable in the reconstructed terminal screen.
        os.write(master, b"\x1b[21~")  # F10
        read_for(0.2)
        os.write(master, b"\x1b[C" * 3)  # Navigate
        select_visible_menu_item("Reference")
        os.write(master, b"\r")
        if not wait_for_screen_fragments((b"Reference Library",), timeout=3.0):
            raise AssertionError("Navigate > Reference did not open the Reference Library")
        os.write(master, b"\r")  # open the selected imported reference
        if not wait_for_screen_fragments((b"Reference only stays clear.",), timeout=3.0):
            raise AssertionError("the imported reference prose was not rendered contiguously")
        assert_visible_sentence(output, 24, 80, "Reference only stays clear.", "Reference Library")
        if any("Maqrara opened the old bok" in row for row in terminal_snapshot(output, 24, 80)):
            raise AssertionError("manuscript prose leaked into the Reference Library")

        set_size(master, 60, 201)
        read_for(0.4)
        assert_visible_sentence(output, 60, 201, "Reference only stays clear.", "Reference at 201x60")
        set_size(master, 24, 80)
        read_for(0.4)
        assert_visible_sentence(output, 24, 80, "Reference only stays clear.", "Reference at 80x24")

        os.write(master, b"\x1b[21~")  # F10
        read_for(0.2)
        os.write(master, b"\x1b[C" * 3)  # Navigate
        read_for(0.2)
        select_visible_menu_item("Manuscript")
        os.write(master, b"\r")
        if not wait_for_workspace_title("Editor", timeout=3.0):
            raise AssertionError(
                "Navigate > Manuscript did not restore the editor; screen: "
                + repr(terminal_snapshot(output, 24, 80))
            )
        if not wait_for_screen_fragments((b"The opening paragraph remains readable.",), timeout=3.0):
            raise AssertionError("manuscript prose was not rendered after returning from Reference Library")
        assert_sample_prose_visible(output, 24, 80, "screen after returning from Reference Library")
        if any("Reference only stays clear." in row for row in terminal_snapshot(output, 24, 80)):
            raise AssertionError("reference prose leaked into the manuscript editor")

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
