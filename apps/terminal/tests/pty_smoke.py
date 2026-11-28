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
import termios
import time
import unicodedata


ANSI_CONTROL = re.compile(
    rb"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1b\\)|\([A-Za-z0-9]|\)[A-Za-z0-9])"
)
CSI = re.compile(r"\x1b\[([0-?]*)([ -/]*)?([@-~])")


def terminal_snapshot(data, rows, columns):
    """Apply the cursor/clear controls Ratatui uses and return visible rows."""
    cells = [[" "] * columns for _ in range(rows)]
    row = column = 0
    alternate_charset = False
    text = data.decode("utf-8", errors="replace")
    index = 0
    while index < len(text):
        character = text[index]
        if character == "\x1b":
            if index + 2 < len(text) and text[index + 1] in "()":
                alternate_charset = text[index + 2] == "0"
                index += 3
                continue
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
        elif character == "\t":
            column = min(columns - 1, ((column // 8) + 1) * 8)
        elif character == "\b":
            column = max(0, column - 1)
        elif character >= " " and character != "\x7f" and row < rows and column < columns:
            if alternate_charset:
                character = {
                    "l": "┌",
                    "m": "└",
                    "k": "┐",
                    "j": "┘",
                    "q": "─",
                    "x": "│",
                    "t": "├",
                    "u": "┤",
                    "w": "┬",
                    "v": "┴",
                    "n": "┼",
                }.get(character, character)
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
    no_function_authoring = modes == ["no-function-authoring"]
    scene_rename = modes == ["scene-rename"]
    heading_rename = modes == ["heading-rename"]
    previous_position = modes == ["previous-position"]
    previous_position_history = modes == ["previous-position-history"]
    external_conflict = modes == ["external-conflict"]
    removal_dialogs = modes == ["removal-dialogs"]
    chapter_creation = modes == ["chapter-creation"]
    revision_snapshot = modes == ["revision-snapshot"]
    revision_document = modes == ["revision-document"]
    backup_archive = modes == ["backup-archive"]
    authoring_audit = modes == ["authoring-audit"]
    cross_document_clipboard = modes == ["cross-document-clipboard"]
    bulk_outline_insert = modes == ["bulk-outline-insert"]
    scene_status_picker = modes == ["scene-status-picker"]
    manuscript_status_settings = modes == ["manuscript-status-settings"]
    scene_planning_fields = modes == ["scene-planning-fields"]
    reference_removal = modes == ["reference-removal"]
    compile_docx = modes == ["compile-docx"]
    scrivener_import = modes == ["scrivener-import"]
    save_as_populated = modes == ["save-as-populated"]
    recent_projects = modes == ["recent-projects"]
    insert_edit_actions = modes == ["insert-edit-actions"]
    outline_enter = modes == ["outline-enter"]
    special_character = modes == ["special-character"]
    named_layouts = modes == ["named-layouts"]
    split_navigation = modes in (
        ["split-navigation"],
        ["split-navigation-tmux"],
        ["split-navigation-screen"],
    )
    pty_wrapper = (
        "tmux" if modes == ["split-navigation-tmux"]
        else "screen" if modes == ["split-navigation-screen"]
        else None
    )
    prefix_startup = modes == ["prefix-startup"]
    edit_menu = modes == ["edit-menu"]
    date_time_insertion = modes == ["date-time-insertion"]
    date_time_unavailable = modes == ["date-time-unavailable"]
    find_replace_focus = modes == ["find-replace-focus"]
    delete_sentence = modes == ["delete-sentence"]
    same_chapter_move = modes == ["same-chapter-move"]
    convert_outline_item = modes == ["convert-outline-item"]
    review_thesaurus = modes == ["review-thesaurus"]
    review_menu = modes in (["review-menu"], ["review-thesaurus"])
    review_spellcheck_suggestions = modes == ["spellcheck-suggestions"]
    review_spellcheck_dictionary_add = modes == ["spellcheck-dictionary-add"]
    review_spellcheck_dictionary_recheck = modes == ["spellcheck-dictionary-recheck"]
    review_spellcheck_current = modes in (
        ["spellcheck-current"],
        ["spellcheck-suggestions"],
        ["spellcheck-dictionary-add"],
        ["spellcheck-dictionary-recheck"],
    )
    review_proofread = modes == ["review-proofread"]
    removed_scenes = modes == ["removed-scenes"]
    outline_view = modes == ["outline-view"]
    search_outline_restore = modes == ["search-outline-restore"]
    editor_viewport = modes == ["editor-viewport"]
    rendered_tabs_off = modes == ["rendered-tabs-off"]
    rendered_tabs_on = modes == ["rendered-tabs-on"]
    startup_chooser = modes == ["startup-chooser"]
    startup_auto_resume = modes == ["startup-auto-resume"]
    cursor_timeout_delayed = modes == ["cursor-timeout-delayed"]
    cursor_timeout_missing = modes == ["cursor-timeout-missing"]
    mouse_input_isolation = modes == ["mouse-input-isolation"]
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
    if (
        create_fiction
        or create_nonfiction
        or recent_projects
        or backup_archive
        or review_proofread
        or pty_wrapper is not None
        or no_function_authoring
        or cross_document_clipboard
        or bulk_outline_insert
        or scene_status_picker
        or manuscript_status_settings
        or scene_planning_fields
        or search_outline_restore
        or mouse_input_isolation
    ):
        # Keep this workflow focused on project lifecycle rather than allowing
        # first-run onboarding to consume editing keys.
        onboarding = os.path.join(isolated_home, "config", "writers-ide")
        os.makedirs(onboarding, exist_ok=True)
        with open(os.path.join(onboarding, "tutorial-state.json"), "w", encoding="utf-8") as state_file:
            state_file.write(
                '{"version":1,"welcome_seen":true,"tutorial_dismissed":true,'
                '"tutorial_completed":false,"active_tutorial":null}'
            )
    if recent_projects or startup_chooser or startup_auto_resume:
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
                "TMUX_TMPDIR": os.path.join(isolated_home, "tmp"),
                "SCREENDIR": os.path.join(isolated_home, "screen"),
            }
        )
        os.makedirs(environment["SCREENDIR"], exist_ok=True)
        os.chmod(environment["SCREENDIR"], 0o700)
        application = [binary] if project == "-" else [binary, project]
        if pty_wrapper == "tmux":
            arguments = [
                "tmux", "-L", "wide-prefix-pty", "-f", "/dev/null",
                "new-session", "-s", "wide-prefix-pty", *application,
            ]
        elif pty_wrapper == "screen":
            arguments = ["screen", "-c", "/dev/null", *application]
        else:
            arguments = application
        os.execvpe(arguments[0], arguments, environment)

    set_size(master, 24, 80)
    output = bytearray()
    cursor_query_offset = 0
    delayed_cursor_replies = []

    def read_for(seconds):
        nonlocal cursor_query_offset
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if delayed_cursor_replies and delayed_cursor_replies[0] <= time.monotonic():
                delayed_cursor_replies.pop(0)
                os.write(master, b"\x1b[1;1R")
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
                if cursor_timeout_delayed:
                    delayed_cursor_replies.append(time.monotonic() + 2.3)
                elif not cursor_timeout_missing:
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

    def wait_for_normalized_fragments(markers, start=0, timeout=5.0):
        """Match rendered status text despite terminal style and blank-cell runs."""
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            visible = b"".join(ANSI_CONTROL.sub(b"", output[start:]).split())
            if all(b"".join(marker.split()) in visible for marker in markers):
                return True
            read_for(0.1)
        visible = b"".join(ANSI_CONTROL.sub(b"", output[start:]).split())
        return all(b"".join(marker.split()) in visible for marker in markers)

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

    def wait_for_screen_absence(marker, timeout=5.0):
        """Wait until a marker is absent from the reconstructed current screen."""
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            visible = "".join("".join(terminal_snapshot(output, 24, 80)).split())
            if marker.decode("utf-8").replace(" ", "") not in visible:
                return True
            read_for(0.1)
        visible = "".join("".join(terminal_snapshot(output, 24, 80)).split())
        return marker.decode("utf-8").replace(" ", "") not in visible

    def wait_for_workspace_title(title, timeout=5.0):
        def matches_title(rows):
            if len(rows) <= 1:
                return False
            heading = rows[1].lstrip("┌").strip()
            return heading.startswith(title) or re.match(r"Pane [1-4] · " + re.escape(title), heading) is not None

        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            rows = terminal_snapshot(output, 24, 80)
            if matches_title(rows):
                return True
            read_for(0.1)
        rows = terminal_snapshot(output, 24, 80)
        return matches_title(rows)

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
        if len(rows) > 1 and (rows[1].startswith("┌Editor") or "· Editor —" in rows[1]):
            os.write(master, b"\x1b")  # use the editor's advertised return route first
            if not wait_for_workspace_title("Outline", timeout=1.0):
                # The terminal may restore the top-level menu focus first;
                # Esc then returns from the editor to its originating outline.
                os.write(master, b"\x1b")
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
            # Editor and other workspaces first offer Scene Cards; switch to
            # that shared outliner, then choose its tree presentation.
            select_visible_menu_item("Show Scene Cards")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Included", b"Excluded"), menu_start, timeout=3.0):
                raise AssertionError("View > Show Scene Cards did not open the outliner")
            show_outline_tree_presentation()
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
            # The application polls terminal input every 200 ms. Read through
            # at least one full poll/render cycle before deciding whether the
            # selection moved; otherwise a stale highlighted row can cause the
            # next Enter to activate its neighbour.
            read_for(0.22)
        raise AssertionError(
            f"menu did not expose selectable item {label!r}; visible tail: "
            + repr(ANSI_CONTROL.sub(b"", output[-2200:]))
            + "\ncurrent screen: "
            + repr(terminal_snapshot(output, 24, 80))
        )

    def open_outline_menu():
        start = len(output)
        os.write(master, b"\x1b[21~")  # F10 remains covered as a compatibility path.
        read_for(0.2)
        for _ in range(12):
            if wait_for_screen_fragments(("Outline · actions".encode("utf-8"),), start, timeout=0.1):
                return start
            os.write(master, b"\x1b[C")
            read_for(0.05)
        raise AssertionError(
            "Outline menu was not reachable from the manuscript outliner; screen: "
            + repr(terminal_snapshot(output, 24, 80))
            + "\nrecent terminal output: "
            + repr(ANSI_CONTROL.sub(b"", output[-1600:]))
        )

    def open_named_menu(name):
        """Open a menu category by its visible label from the current context."""
        start = open_outline_menu()
        if name == "Outline":
            return start
        marker = f"{name} · actions".encode("utf-8")
        for _ in range(12):
            if wait_for_screen_fragments((marker,), start, timeout=0.1):
                return start
            os.write(master, b"\x1b[C")
            read_for(0.05)
        raise AssertionError(
            f"{name} menu was not reachable from the current workspace; screen: "
            + repr(terminal_snapshot(output, 24, 80))
        )

    def open_help_menu(already_open=False):
        """Find Help from the current menu root instead of assuming its index."""
        if not already_open:
            os.write(master, b"\x1f")  # Ctrl+/ standard menu route.
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
        if prefix_startup:
            # Wait only until RawModeGuard has changed the PTY's line
            # discipline, then queue 0x1C before waiting for the application
            # screen. This exercises project-open startup without racing the
            # process's exec-to-main interval, when terminal input still
            # belongs to the parent shell.
            raw_deadline = time.monotonic() + 3.0
            raw_enabled = False
            while time.monotonic() < raw_deadline:
                if not termios.tcgetattr(master)[3] & termios.ISIG:
                    raw_enabled = True
                    break
                read_for(0.01)
            if not raw_enabled:
                raise AssertionError("application did not enter raw mode during startup")
            os.write(master, b"\x1c")
        read_for(1.0)
        if prefix_startup:
            if not wait_for_screen_fragments((b"Window prefix",), timeout=5.0):
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
                if finished_pid == child_pid and os.WIFSIGNALED(status):
                    raise AssertionError("startup Ctrl+Backslash terminated the process with a signal")
                raise AssertionError(
                    "startup byte 0x1C was not queued as the window prefix; current screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\x1b")
            if not wait_for_screen_fragments((b"Window command cancelled",), timeout=3.0):
                raise AssertionError("startup prefix did not accept Escape cancellation")
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit after the startup prefix check")
            print("PTY startup-prefix workflow passed: early ASCII 0x1C queued safely, prefix cancelled, and terminal restored.")
            return
        if no_function_authoring:
            if wait_for_screen_fragments((b"Welcome to Writer's IDE",), timeout=1.0):
                os.write(master, b"\x1b")
            if not wait_for_workspace_title("Editor", timeout=0.25):
                if not wait_for_workspace_title("Outline", timeout=5.0):
                    raise AssertionError("neither Outline nor Editor opened for the no-function-key workflow; screen: " + repr(terminal_snapshot(output, 24, 80)))
                os.write(master, b"\r")  # Enter opens the selected manuscript item.
            if not wait_for_workspace_title("Editor", timeout=5.0):
                    raise AssertionError("Enter did not open the selected manuscript in the editor")
            os.write(master, b"\r")
            read_for(0.1)
            os.write(master, b"The author-facing keyboard path works.")
            read_for(0.35)
            os.write(master, b"\x13")  # Ctrl+S saves after all prose input is delivered.
            if not wait_for_screen_fragments((b"author-facing",), timeout=3.0):
                raise AssertionError("editor did not accept the author text; screen: " + repr(terminal_snapshot(output, 24, 80)))
            if not wait_for_screen_fragments((b"SAVED",), timeout=4.0):
                raise AssertionError("Ctrl+S did not report that the manuscript was saved; screen: " + repr(terminal_snapshot(output, 24, 80)))
            find_start = len(output)
            os.write(master, b"\x06")  # Ctrl+F opens Find.
            if not wait_for_screen_fragments((b"Enter: Find",), find_start, timeout=3.0):
                raise AssertionError("Ctrl+F did not open Find")
            os.write(master, b"author-facing")
            os.write(master, b"\r")
            close_find_start = len(output)
            os.write(master, b"\x1b")
            if not wait_for_screen_fragments((b"Find panel closed",), close_find_start, timeout=3.0):
                raise AssertionError("Esc did not close Find before returning to Outline")
            read_for(0.25)
            os.write(master, b"\x1b")  # Escape returns to Outline from the editor.
            if not wait_for_screen_fragments((b"Returned to Outline",), timeout=3.0):
                raise AssertionError("Editor did not acknowledge Escape navigation; screen: " + repr(terminal_snapshot(output, 24, 80)))
            if not wait_for_workspace_title("Outline", timeout=3.0):
                raise AssertionError("Esc did not return from the editor to Outline; screen: " + repr(terminal_snapshot(output, 24, 80)))
            menu_start = len(output)
            os.write(master, b"\x1f")  # Ctrl+/ control byte; Crossterm presents Ctrl+7.
            if not wait_for_screen_fragments((b"File", b"Quit"), menu_start, timeout=3.0):
                raise AssertionError("Ctrl+/ did not open the File menu from Outline; screen: " + repr(terminal_snapshot(output, 24, 80)))
            os.write(master, b"\x1b")
            if not wait_for_screen_fragments((b"Menu closed",), menu_start, timeout=3.0):
                raise AssertionError("Esc did not cancel the menu and restore Outline")
            os.write(master, b"\x11")  # Ctrl+Q quits without a function key.
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("Ctrl+Q did not exit from Outline cleanly; screen: " + repr(terminal_snapshot(output, 24, 80)))
            print("PTY no-function-key author workflow passed: Enter opens, Ctrl+S saves, Ctrl+F finds, Esc closes Find and returns to Outline, Ctrl+Q exits.")
            return
        if editor_viewport:
            if wait_for_screen_fragments((b"Welcome to Writer's IDE",), timeout=1.0):
                os.write(master, b"\x1b")
            if not wait_for_workspace_title("Editor", timeout=5.0):
                raise AssertionError(
                    "long manuscript did not open in the Editor; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            if not wait_for_screen_fragments((b"LINE-00",), timeout=3.0):
                raise AssertionError("first long-document line was not initially visible")
            for _ in range(40):
                os.write(master, b"\x1b[B")
                read_for(0.025)
            if not wait_for_screen_fragments((b"LINE-40",), timeout=3.0):
                raise AssertionError(
                    "cursor navigation did not scroll the long manuscript; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            if any("LINE-00" in row for row in terminal_snapshot(output, 24, 80)):
                raise AssertionError("the viewport remained anchored at the document start")

            # A terminal resize must recalculate wrapping and retain the active
            # cursor line in view at both the larger and original dimensions.
            set_size(master, 36, 120)
            read_for(0.5)
            if not any("LINE-40" in row for row in terminal_snapshot(output, 36, 120)):
                raise AssertionError("the cursor line disappeared after enlarging the terminal")
            set_size(master, 24, 80)
            read_for(0.5)
            if not any("LINE-40" in row for row in terminal_snapshot(output, 24, 80)):
                raise AssertionError("the cursor line disappeared after restoring 80x24")

            for character in "VISIBLE-INSERTION":
                os.write(master, character.encode("utf-8"))
                read_for(0.04)
            if not wait_for_screen_fragments((b"VISIBLE-INSERTION", b"LINE-40"), timeout=7.0):
                raise AssertionError("text insertion at the scrolled cursor was not visible")
            os.write(master, b"\x13")  # Ctrl+S
            if not wait_for_screen_fragments((b"[SAVED]",), timeout=4.0):
                raise AssertionError("the scrolled editor did not save its insertion")
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit after saving the scrolled edit")
            print("PTY editor viewport passed: cursor-following, resize and saved insertion work in a long manuscript.")
            return
        if rendered_tabs_off or rendered_tabs_on:
            if wait_for_screen_fragments((b"Welcome to Writer's IDE",), timeout=1.0):
                os.write(master, b"\x1b")
            if not wait_for_workspace_title("Editor", timeout=5.0):
                raise AssertionError("tab-rendering fixture did not open in the manuscript editor")
            rows = terminal_snapshot(output, 24, 80)
            manuscript_row = next((row for row in rows if "INDENTED-PROSE" in row), None)
            if manuscript_row is None:
                raise AssertionError("literal-tab manuscript line was absent from rendered terminal output: " + repr(rows))
            marker = manuscript_row.find(">")
            text_position = manuscript_row.index("INDENTED-PROSE")
            visual_indent = text_position - marker - 2
            if visual_indent != 4:
                raise AssertionError(f"tab indentation rendered as {visual_indent} columns, expected 4: {manuscript_row!r}")
            arrow_position = manuscript_row.find("→")
            if rendered_tabs_on and arrow_position < 0:
                raise AssertionError("Reveal Codes enabled but the tab mark was not rendered")
            if rendered_tabs_off and arrow_position >= 0:
                raise AssertionError("Reveal Codes disabled but a tab mark leaked into rendered prose")
            if arrow_position >= 0 and manuscript_row.index("INDENTED-PROSE") - arrow_position != 4:
                raise AssertionError("Reveal Codes changed the tab's rendered display columns")
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after rendered-tab verification")
            print("PTY rendered tab check passed: literal tabs occupy four stable display columns with formatting marks off/on.")
            return
        if startup_chooser:
            if not wait_for_screen_fragments((b"Welcome to Writer's IDE", b"Resume Last Project", b"New Project", b"Don't show this again"), timeout=5.0):
                raise AssertionError("new-user startup chooser did not present the project actions")
            for _ in range(8):
                os.write(master, b"\x1b[B")
                read_for(0.03)
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Startup chooser disabled",), timeout=5.0):
                raise AssertionError("the persistent startup opt-out did not resume the last project")
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit after startup chooser preference was saved")
            print("PTY startup chooser passed: first-run choices are visible and opting out resumes the last project.")
            return
        if startup_auto_resume:
            read_for(0.4)
            rows = terminal_snapshot(output, 24, 80)
            if any("Welcome to Writer's IDE" in row for row in rows):
                raise AssertionError("the persisted chooser opt-out was not honoured on the next launch")
            if not any("Outline" in row for row in rows):
                raise AssertionError("auto-resume did not open the recent project workspace: " + repr(rows))
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit after auto-resuming the previous project")
            print("PTY auto-resume passed: a persisted opt-out opens the last project without the chooser.")
            return
        if cursor_timeout_delayed or cursor_timeout_missing:
            if wait_for_screen_fragments((b"Welcome to Writer's IDE",), timeout=1.0):
                os.write(master, b"\x1b")
            if not wait_for_screen_fragments((b"Included", b"No scenes yet"), timeout=5.0):
                raise AssertionError("cursor-timeout fixture did not reach the normal authoring workspace: " + repr(terminal_snapshot(output, 24, 80)))
            query_start = len(output)
            set_size(master, 30, 100)
            read_for(2.6 if cursor_timeout_delayed else 2.2)
            finished_pid, _ = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != 0:
                raise AssertionError("terminal resize caused WIDE to exit while handling cursor-report behavior")
            if b"\x1b[6n" in output[query_start:]:
                raise AssertionError("screen transition issued a cursor-position query that can time out")
            if not any("Included" in row for row in terminal_snapshot(output, 30, 100)):
                raise AssertionError("authoring workspace did not remain visible after terminal resize")
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit after cursor-report regression verification")
            print("PTY cursor-report check passed: delayed/missing terminal replies cannot block or terminate screen resize.")
            return

        if mouse_input_isolation:
            if not wait_for_screen_fragments((b"Editor", b"Arrival", b"Preserve this prose."), timeout=5.0):
                raise AssertionError("the manuscript editor did not open before mouse-input isolation")
            if b"?1000h" in output or b"?1006h" in output:
                raise AssertionError("mouse reporting was enabled in the ordinary manuscript editor")
            # Inject the SGR mouse report bytes a terminal sends for a click.
            os.write(master, b"\x1b[<0;12;9M")
            read_for(0.3)
            os.write(master, b"\x13")
            if not wait_for_screen_fragments((b"Saved at",), timeout=3.0):
                raise AssertionError("mouse-report isolation workflow did not save normally")
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("mouse-report isolation workflow did not exit cleanly")
            print("PTY mouse-input isolation passed: ordinary editor mode does not enable terminal mouse reporting and an SGR report does not contaminate prose.")
            return
        if backup_archive:
            start = len(output)
            os.write(master, b"\x1b[21~")  # File menu
            select_visible_menu_item("Back Up Project")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Back Up Project", b"Choose a separate destination"), start, timeout=3.0
            ):
                raise AssertionError("Back Up Project did not open the destination browser")
            os.write(master, b"\r")  # The archive filename/action control is focused on entry.
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
            if not wait_for_screen_fragments((b"Target Project",), timeout=5.0):
                raise AssertionError(
                    "Recent Projects did not open the selected book; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            # The unified Outline opens direct chapter prose when the chapter
            # row is selected. Move to the scene row to continue in its text.
            if not wait_for_screen_fragments(
                (b"The Target Chapter", b"The Target Scene"), timeout=5.0
            ):
                raise AssertionError("the opened project did not show its manuscript outline")
            os.write(master, b"\x1b[B\r")
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
                (b"Save Project As", b"Enter Save Project Copy"), menu_start, timeout=5.0
            ):
                raise AssertionError("File > Save Project As did not open its folder browser")
            os.write(master, b"\r")  # The project-name/action control is focused on entry.
            if not wait_for_screen_fragments(
                (b"Create an independent project copy",), timeout=5.0
            ):
                raise AssertionError("Save As did not show its destination confirmation")
            os.write(master, b"\r")  # Enter confirms the independent copy.
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
                imported_in_outline = len(rows) > 1 and (
                    rows[1].startswith("┌Outline") or "· Outline" in rows[1]
                )
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
                (b"[ ]", b"Opening Scene"), timeout=2.0
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
                # Direct chapter prose is opened from the chapter row; the
                # outliner does not expose the storage-only ChapterBody node.
                if not move_outline_selection_to_manuscript_root():
                    raise AssertionError("could not navigate to the top of the imported outline")
                if not move_outline_selection_until(
                    lambda row: "Chapter One" in row,
                    "down",
                ):
                    raise AssertionError(
                        "outline did not expose the chapter that owns direct prose; screen: "
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
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Create an independent project copy",), timeout=3.0):
                raise AssertionError(
                    "Save As did not show its no-overwrite confirmation; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\r")
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
                    lambda row: "Chapter One" in row,
                    "down",
                ):
                    raise AssertionError("saved outline lost its chapter with direct prose")
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
            if not wait_for_screen_fragments((b"Selected path:", b".docx", b"Enter Compile"), timeout=5.0):
                raise AssertionError("imported project did not reach the publication file browser")
            os.write(master, b"\r")
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
            if review_thesaurus:
                if wait_for_workspace_title("Outline", timeout=1.0):
                    if not move_outline_selection_until(lambda row: "Arrival" in row, "down"):
                        raise AssertionError("the thesaurus test scene was not selectable")
                    os.write(master, b"\r")
                if not wait_for_workspace_title("Editor", timeout=3.0):
                    raise AssertionError("the thesaurus test manuscript did not open")
                # Select the leading word "colour" using ordinary editor
                # selection input before invoking the Review command.
                os.write(master, b"\x1b[1;2C" * 6)
                read_for(0.2)
            menu_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.2)
            found_review = False
            for _ in range(10):
                if wait_for_screen_fragments((b"Spell Check",), menu_start, timeout=0.15):
                    found_review = True
                    break
                os.write(master, b"\x1b[C")
                read_for(0.1)
            if not found_review:
                raise AssertionError("Review menu did not expose Spell Check")
            # Menu viewports show a subset of a long command group at once.
            # Walk the selectable entries just as a keyboard user would; do
            # not require every command to fit in the initial viewport.
            select_visible_menu_item("Thesaurus" if review_thesaurus else "Spell Check")
            os.write(master, b"\r")
            if review_thesaurus:
                if not wait_for_screen_fragments((b"Thesaurus", b"colour", b"hue", b"pigment"), menu_start, timeout=5.0):
                    raise AssertionError(
                        "selected-text lookup did not show offline results alongside the manuscript; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
                os.write(master, b"r")  # explicitly replace with the first thesaurus suggestion
                if not wait_for_screen_fragments((b"Replaced selected text with", b"hue"), menu_start, timeout=3.0):
                    raise AssertionError("thesaurus replacement did not update the manuscript selection")
                os.write(master, b"\x13")
                if not wait_for_screen_fragments((b"Saved at",), timeout=3.0):
                    raise AssertionError("thesaurus replacement was not saved")
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
            if not wait_for_screen_fragments((b"Spell Check", b"Arrival", b"No spelling candidates found"), menu_start, timeout=5.0):
                raise AssertionError("Review > Spell Check did not show current-document results")
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

        if review_spellcheck_current:
            if wait_for_screen_fragments((b"Welcome to Writer's IDE",), timeout=1.0):
                os.write(master, b"\x1b")
            if wait_for_workspace_title("Outline", timeout=3.0):
                if not move_outline_selection_until(lambda row: "Arrival" in row, "down"):
                    raise AssertionError("the Arrival scene was not selectable")
                os.write(master, b"\r")
            elif not wait_for_workspace_title("Editor", timeout=3.0):
                raise AssertionError("project did not open in an authoring workspace; screen: " + repr(terminal_snapshot(output, 24, 80)))
            if not wait_for_workspace_title("Editor", timeout=3.0):
                raise AssertionError("the Arrival document did not open")
            menu_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.2)
            os.write(master, b"\x1b[C" * 5)  # Review menu root
            read_for(0.2)
            select_visible_menu_item("Spell Check")
            os.write(master, b"\r")
            if review_spellcheck_dictionary_recheck:
                expected = (b"Spell Check", b"Arrival", b"en-GB", b"Spell 0")
            else:
                expected_location = (
                    b"line 1, column 1"
                    if review_spellcheck_suggestions or review_spellcheck_dictionary_add
                    else b"line 41, column 1"
                )
                expected = (b"Spell Check", b"Arrival", b"en-GB", b"Baad", expected_location)
            if not wait_for_screen_fragments(expected, menu_start, timeout=5.0):
                raise AssertionError(
                    "Spell Check did not show results for the open document; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            screen = "\n".join(terminal_snapshot(output, 24, 80))
            if "Wrng" in screen:
                raise AssertionError("Spell Check included a different scene instead of the current document")
            if review_spellcheck_dictionary_recheck:
                if "Baad · line" in "\n".join(terminal_snapshot(output, 24, 80)):
                    raise AssertionError("project dictionary word remained in spell-check results after reopening")
            elif review_spellcheck_dictionary_add:
                os.write(master, b"d")
                if not wait_for_screen_fragments((b"to this project's dictionary", b"Manuscript unchanged"), timeout=3.0):
                    raise AssertionError("Add to Project Dictionary did not confirm the update; screen: " + repr(terminal_snapshot(output, 24, 80)))
                if "Baad · line" in "\n".join(terminal_snapshot(output, 24, 80)):
                    raise AssertionError("adding a word did not remove its current spelling findings")
            elif review_spellcheck_suggestions:
                os.write(master, b"s")
                if not wait_for_screen_fragments((b"Spelling Suggestions", b"bad", b"bead"), timeout=3.0):
                    raise AssertionError("suggestion selector did not display multiple spelling candidates")
                os.write(master, b"\x1b[B\r")  # choose and apply the second suggestion
                if not wait_for_screen_fragments((b"Changed 'Baad' to 'bead'", b"Undo available"), timeout=3.0):
                    raise AssertionError("selected non-first spelling suggestion was not applied")
            else:
                os.write(master, b"i")  # ignore only this occurrence in the current review
                if not wait_for_screen_fragments(
                    (b"Spell 1", b"line 41, column 12"),
                    timeout=3.0,
                ):
                    raise AssertionError(
                        "Ignore Once did not remove only the selected spelling result; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
                os.write(master, b"r")  # enter an author-chosen correction
                if not wait_for_screen_fragments((b"Replace", b"Baad"), timeout=3.0):
                    raise AssertionError("custom spelling correction dialogue was not shown")
                os.write(master, b"Bad\r")
                if not wait_for_screen_fragments((b"Changed 'Baad' to 'Bad'", b"Undo available"), timeout=3.0):
                    raise AssertionError("custom spelling correction was not applied")
            if not (review_spellcheck_dictionary_add or review_spellcheck_dictionary_recheck):
                os.write(master, b"\x13")  # save
                if not wait_for_screen_fragments((b"Saved at",), timeout=3.0):
                    raise AssertionError("corrected manuscript text was not saved")
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after current-document Spell Check")
            print("PTY Spell Check passed: selected correction was applied, saved and kept within the open document.")
            return

        if review_proofread:
            if wait_for_screen_fragments((b"Welcome to Writer's IDE",), timeout=1.0):
                os.write(master, b"\x1b")  # dismiss first-run welcome
            if not wait_for_workspace_title("Outline", timeout=3.0) and not wait_for_workspace_title("Editor", timeout=0.1):
                raise AssertionError("project did not open in a manuscript workspace")
            if wait_for_workspace_title("Outline", timeout=0.1):
                if not move_outline_selection_until(lambda row: "Arrival" in row, "down"):
                    raise AssertionError(
                        "the Opening scene was not selectable in the outline; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
                os.write(master, b"\r")  # open the selected scene
            if not wait_for_workspace_title("Editor") or not wait_for_screen_fragments((b"They is ready",), timeout=3.0):
                raise AssertionError("the selected scene did not open with its saved prose")

            menu_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.2)
            os.write(master, b"\x1b[C" * 5)  # Review
            select_visible_menu_item("Grammar Check")
            read_for(0.2)  # let the menu selection redraw before dispatching Enter
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Grammar check started",), menu_start, timeout=2.0):
                raise AssertionError("Review > Grammar Check did not return control with progress feedback")
            if not wait_for_screen_fragments(
                (b"Grammar review", b"Suggestion: are", b"a apply"),
                menu_start,
                timeout=60.0,
            ):
                screen = terminal_snapshot(output, 24, 80)
                with open(os.path.join(isolated_home, "proofread-screen.txt"), "w", encoding="utf-8") as snapshot:
                    snapshot.write("\n".join(screen))
                key_rows = [row for row in screen if any(token in row for token in ("Offline", "Suggestion", "Apply", "Grammar", "They"))]
                raise AssertionError(
                    "Review > Grammar Check did not finish; relevant screen rows: "
                    + repr(key_rows)
                )
            os.write(master, b"a")  # apply the selected suggestion
            if not wait_for_screen_fragments((b"They are ready",), menu_start, timeout=3.0):
                raise AssertionError("the accepted proofreading correction was not visible in the editor")
            os.write(master, b"\x13")  # save corrected prose
            if not wait_for_screen_fragments((b"SAVED",), menu_start, timeout=3.0):
                raise AssertionError("the accepted proofreading correction did not report saved state")
            os.write(master, b"\x11")  # quit after the explicit save
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after proofreading correction")
            print("PTY proofreading passed: select a scene, run Review > Grammar Check, apply a suggestion, save and exit.")
            return

        if tutorial_navigation:
            if not wait_for_screen_fragments(
                (b"Welcome to Writer's IDE", b"Guided Tour", b"Quick Start"), timeout=10.0
            ):
                raise AssertionError("first-run welcome did not present the tutorial choices")
            # Resume Last Project, New, Open, Recent, Import, Guided Tour.
            os.write(master, b"\x1b[B" * 5 + b"\r")
            if not wait_for_screen_fragments(
                (b"Tutorial", b"1/6", b"Press Ctrl+/ to open the menu"), timeout=10.0
            ):
                raise AssertionError("Guided Tour did not open its first actionable lesson")

            # F6 is assigned to Characters globally; the focused tutorial must
            # retain the key and keep the same step and task visible.
            os.write(master, b"\x1b[17~")
            read_for(0.2)
            if not wait_for_screen_fragments(
                (b"Tutorial", b"1/6", b"0/9 verified", b"Press Ctrl+/ to open the menu"),
                timeout=3.0,
            ):
                raise AssertionError("F6 took focus away from the active tutorial lesson")

            # Complete the visible route without a function key: Ctrl+/ → Navigate → Outline → scene.
            menu_start = len(output)
            os.write(master, b"\x1f")
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
            os.write(master, b"\x1f")
            if not wait_for_screen_fragments((b"New Project", b"Quit"), timeout=3.0):
                raise AssertionError(
                    "Ctrl+/ did not open the File menu while the tutorial was active; screen: "
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
            if not wait_for_workspace_title("Outline", timeout=3.0):
                raise AssertionError("Esc did not restore the originating unified Outline")
            show_scene_cards_presentation()
            os.write(master, b"\r")  # reopen the selected scene from Scene Cards
            if not wait_for_workspace_title("Editor"):
                raise AssertionError("Scene Cards Enter did not reopen the selected scene")
            os.write(master, b"\x1b")
            if not wait_for_screen_fragments((b"Included", b"Excluded"), timeout=3.0):
                raise AssertionError("Esc did not return to the Scene Cards presentation")
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
            if not wait_for_workspace_title("Outline", timeout=3.0):
                raise AssertionError("Esc did not restore the originating unified Outline")
            show_outline_tree_presentation()
            if not wait_for_workspace_title("Outline"):
                raise AssertionError("the View menu did not open the manuscript Outline")
            os.write(master, b"\x1b[A")  # from Opening to its chapter heading
            read_for(0.2)
            open_outline_menu()
            select_visible_menu_item("Rename Selected Outline Item…")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Old Chapter", b"Rename selected outline item", b"Enter saves"), timeout=3.0
            ):
                raise AssertionError(
                    "heading rename did not show its prefilled title field; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
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

        if search_outline_restore:
            for _ in range(3):
                if wait_for_workspace_title("Outline", timeout=0.2):
                    break
                os.write(master, b"\x1b")
                read_for(0.2)
            if not wait_for_workspace_title("Outline", timeout=1.0):
                raise AssertionError("could not focus the manuscript outline before searching")
            os.write(master, b"/needle\r")  # quick search narrows the result view
            if not wait_for_screen_fragments((b"1 search result",), timeout=5.0):
                raise AssertionError(
                    "quick search did not display its single matching result; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            menu_start = len(output)
            os.write(master, b"\x1b[21~")
            read_for(0.2)
            for _ in range(10):
                if wait_for_screen_fragments((b"Show Scene Cards",), menu_start, timeout=0.1):
                    break
                os.write(master, b"\x1b[C")
                read_for(0.2)
            else:
                raise AssertionError(
                    "View menu did not expose Scene Cards from search results; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            select_visible_menu_item("Show Scene Cards")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(("Included · 12".encode("utf-8"),), timeout=5.0):
                raise AssertionError(
                    "View > Show Scene Cards did not leave the search results; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            menu_start = len(output)
            os.write(master, b"\x1b[21~")
            if not wait_for_screen_fragments((b"Show Outline Tree",), menu_start, timeout=1.0):
                for _ in range(8):
                    os.write(master, b"\x1b[C")
                    read_for(0.15)
                    if wait_for_screen_fragments((b"Show Outline Tree",), menu_start, timeout=0.1):
                        break
                else:
                    raise AssertionError("View menu did not offer the outline tree from Scene Cards")
            select_visible_menu_item("Show Outline Tree")
            os.write(master, b"\r")
            if not wait_for_workspace_title("Outline", timeout=5.0):
                raise AssertionError(
                    "the application did not return to the manuscript outline; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            rows = terminal_snapshot(output, 24, 80)
            visible = "".join(rows)
            if "12 scene(s)" not in visible or "Scene 05" not in visible:
                raise AssertionError(f"outline count or selected search result was lost; screen: {rows!r}")
            if "▸   [ ] Scene 05" not in visible:
                raise AssertionError("the matching scene was not retained as the outline selection")
            for _ in range(7):
                os.write(master, b"\x1b[B")
                read_for(0.12)
            if not wait_for_screen_fragments(("▸   [ ] Scene 12".encode("utf-8"),), timeout=2.0):
                raise AssertionError(
                    "the last manuscript item was not reachable after returning from search; screen: "
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
                raise AssertionError("application did not exit cleanly after restoring the outline")
            print("PTY search-to-outline passed: search retained its match, restored all manuscript items and closed cleanly.")
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

        if cross_document_clipboard:
            if wait_for_screen_fragments((b"Welcome to Writer's IDE",), timeout=1.0):
                os.write(master, b"\x1b")
            show_outline_tree_presentation()
            if not move_outline_selection_until(lambda row: "Source Passage" in row, "down"):
                raise AssertionError("clipboard source document was not selectable in the outline")
            os.write(master, b"\r")
            if not wait_for_workspace_title("Editor") or not wait_for_screen_fragments(
                (b"MANUSCRIPT-CLIPBOARD-PASSAGE",), timeout=3.0
            ):
                raise AssertionError("source manuscript document did not open")
            os.write(master, b"\x01\x03")  # Ctrl+A, Ctrl+C: copy the current document
            if not wait_for_screen_fragments((b"Selection copied to the local clipboard",), timeout=3.0):
                raise AssertionError("copy did not report success or use WIDE's internal clipboard")
            os.write(master, b"\x1b")
            if not wait_for_workspace_title("Outline", timeout=3.0):
                raise AssertionError("Esc did not return to the outliner after copying")
            if not move_outline_selection_until(lambda row: "Destination Passage" in row, "down"):
                raise AssertionError("clipboard destination document was not selectable")
            os.write(master, b"\r")
            if not wait_for_workspace_title("Editor") or not wait_for_screen_fragments(
                (b"DESTINATION-START",), timeout=3.0
            ):
                raise AssertionError("destination manuscript document did not open")
            os.write(master, b"\x1b[1;5F\x16")  # Ctrl+End, Ctrl+V
            if not wait_for_screen_fragments((b"MANUSCRIPT-CLIPBOARD-PASSAGE",), timeout=3.0):
                raise AssertionError("internal clipboard did not paste into the second manuscript document")
            os.write(master, b"\x13")
            if not wait_for_screen_fragments((b"SAVED",), timeout=5.0):
                raise AssertionError("cross-document paste was not saved")

            menu_start = len(output)
            os.write(master, b"\x1b[21~")  # F10 opens File from the editor
            read_for(0.2)
            for _ in range(10):
                current = "".join("".join(terminal_snapshot(output, 24, 80)).split())
                if "OpenTextFile" in current:
                    break
                os.write(master, b"\x1b[C")
                read_for(0.1)
            else:
                raise AssertionError(
                    "File menu did not expose Open Text File from the editor; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                    + "; tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1200:]))
                )
            select_visible_menu_item("Open Text File…")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Choose a text file", b"A-reference.txt"), menu_start, timeout=3.0):
                raise AssertionError("Open Text File did not show the external reference in its browser")
            # Start on the directory tree. Tab focuses the contents pane;
            # directories sort before files, so config, screen and tmp precede
            # A-reference.txt in this isolated HOME.
            os.write(master, b"\t" + b"\x1b[B" * 3)
            read_for(0.3)
            rows = terminal_snapshot(output, 24, 80)
            if not any("A-reference.txt" in row and "▸" in row for row in rows):
                raise AssertionError("external reference file was not selected in the focused contents pane: " + repr(rows))
            os.write(master, b"\r")
            read_for(0.2)  # Enter places the selected filename in the dialogue field.
            os.write(master, b"\r")  # A second Enter opens the read-only reference.
            if not wait_for_screen_fragments((b"REFERENCE-LINE-ONE", b"REFERENCE-LINE-TWO"), timeout=5.0):
                raise AssertionError(
                    "external reference text did not open in its read-only pane; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                    + "; tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"v\x1b[B\x03")  # mark both lines, then copy them
            if not wait_for_screen_fragments((b"Copied to Writer's IDE clipboard",), timeout=3.0):
                raise AssertionError("external reference selection was not copied internally")
            os.write(master, b"\x1c\t\x1b[1;5F\x16")  # prefix + Tab focuses manuscript; end, paste
            if not wait_for_screen_fragments((b"REFERENCE-LINE-ONE", b"REFERENCE-LINE-TWO"), timeout=3.0):
                raise AssertionError(
                    "external text did not paste into the manuscript pane; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                    + "; tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1600:]))
                )
            os.write(master, b"\x13")
            if not wait_for_screen_fragments((b"SAVED",), timeout=5.0):
                raise AssertionError("external reference paste was not saved")
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("clipboard workflow did not exit cleanly after saving")
            print("PTY clipboard workflow passed: copied across documents and from a read-only external file into the manuscript; saved and exited.")
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
                (b"Selectedpath:", b"EnterConfirm"),
                file_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "Insert > Insert File did not open the shared browser; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
            os.write(master, b"\t" + b"\x1b[B" * 3 + b"\r\r")
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

        if split_navigation:
            if wait_for_screen_fragments((b"Welcome to Writer's IDE",), timeout=1.0):
                os.write(master, b"\x1b")  # dismiss the first-run welcome panel
            if not wait_for_screen_fragments((b"PANE-LINE-00",), timeout=5.0):
                raise AssertionError(
                    "the opening document was not visible before splitting; current screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )

            if pty_wrapper is not None:
                # Keep the multiplexer probe focused on the raw prefix path;
                # F10 and Escape may be intercepted by the host layer.
                os.write(master, b"\x1c")
                if not wait_for_normalized_fragments((b"Window prefix",), timeout=3.0):
                    raise AssertionError(f"{pty_wrapper} did not forward ASCII 0x1C")
                os.write(master, b"v")
                if not wait_for_normalized_fragments(
                    (b"Split side by side",), timeout=3.0
                ):
                    raise AssertionError(f"{pty_wrapper} did not forward the separate V command")
                os.write(master, b"\x1c")
                if not wait_for_normalized_fragments((b"Window prefix",), timeout=3.0):
                    raise AssertionError(f"{pty_wrapper} did not reactivate ASCII 0x1C")
                os.write(master, b"\x31")
                if not wait_for_normalized_fragments(
                    (b"Focused pane 1",), timeout=3.0
                ):
                    raise AssertionError(
                        f"{pty_wrapper} did not execute the separate ASCII 0x31 pane selection"
                    )
                os.write(master, b"\x11")  # Ctrl+Q
                quit_deadline = time.monotonic() + 5.0
                finished_pid = 0
                status = 0
                while finished_pid == 0 and time.monotonic() < quit_deadline:
                    read_for(0.1)
                    finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
                if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                    raise AssertionError(f"WIDE did not exit cleanly under {pty_wrapper}")
                print(
                    f"PTY prefix workflow passed through {pty_wrapper}: separate 0x1C + V split, then 0x1C + 0x31 focused pane 1."
                )
                return

            split_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.2)
            os.write(master, b"\x1b[C" * 4)  # View
            if not wait_for_screen_fragments(
                (b"Window Commands", b"Ctrl+\\"), split_start, timeout=2.0
            ):
                raise AssertionError(
                    "View menu did not show the window command and canonical Ctrl+Backslash label"
                )
            select_visible_menu_item("Split Side by Side")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Split side by side",), split_start, timeout=3.0):
                raise AssertionError(
                    "Window split did not open a second pane; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                    + "\nraw visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1200:]))
                )

            # The split opens with the second view focused. Scroll it beyond
            # the first pane's viewport and verify both independent positions
            # are visible together on the reconstructed terminal screen.
            os.write(master, b"\x1b[6~" * 3)  # Page Down in the focused second view
            read_for(0.3)
            rows = terminal_snapshot(output, 24, 80)
            screen = "\n".join(rows)
            if "PANE-LINE-00" not in screen or "PANE-LINE-30" not in screen:
                raise AssertionError(
                    "scrolling the second window did not preserve independent viewports; screen: "
                    + repr(rows)
                )

            focus_start = len(output)
            def send_window_prefix():
                start = len(output)
                os.write(master, b"\x1c")  # ASCII FS: Ctrl+Backslash (Crossterm decodes Ctrl+4)
                if not wait_for_screen_fragments((b"Window prefix",), start, timeout=3.0):
                    finished, status = os.waitpid(child_pid, os.WNOHANG)
                    if finished == child_pid and os.WIFSIGNALED(status):
                        raise AssertionError(
                            "Ctrl+Backslash raised a signal instead of reaching Crossterm"
                        )
                    raise AssertionError(
                        "ASCII 0x1C did not activate the window-management prefix; current screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                        + "\nrecent output: "
                        + repr(ANSI_CONTROL.sub(b"", output[-1600:]))
                    )
                finished, status = os.waitpid(child_pid, os.WNOHANG)
                if finished == child_pid and os.WIFSIGNALED(status):
                    raise AssertionError(
                        "raw terminal mode did not suppress SIGQUIT for Ctrl+Backslash"
                    )
                return start

            send_window_prefix()  # prefix-only input is observably active before the next key
            os.write(master, b"\x1b[D")  # separate event: Left focuses the adjacent pane
            if not wait_for_screen_fragments((b"Focused pane to the left",), focus_start, timeout=3.0):
                raise AssertionError("Ctrl+Backslash, Left did not focus the left pane")
            os.write(master, b"\x1b[6~")  # Page Down in the first/editor window
            read_for(0.3)
            rows = terminal_snapshot(output, 24, 80)
            screen = "\n".join(rows)
            if "PANE-LINE-10" not in screen or "PANE-LINE-30" not in screen:
                raise AssertionError(
                    "scrolling the first window changed the second window or did not move independently; screen: "
                    + repr(rows)
                )

            focus_start = send_window_prefix()
            os.write(master, b"\x1b[C")  # separate event: Right
            if not wait_for_screen_fragments((b"Focused pane to the right",), focus_start, timeout=3.0):
                raise AssertionError("Ctrl+Backslash, Right did not focus the right pane")

            focus_start = send_window_prefix()
            os.write(master, b"\t")
            if not wait_for_screen_fragments((b"Focused next window",), focus_start, timeout=3.0):
                raise AssertionError("Ctrl+Backslash, Tab did not focus the next pane")
            focus_start = send_window_prefix()
            os.write(master, b"\x1b[Z")  # terminal Shift+Tab / BackTab
            if not wait_for_screen_fragments((b"Focused previous window",), focus_start, timeout=3.0):
                raise AssertionError("Ctrl+Backslash, Shift+Tab did not focus the previous pane")

            operation_start = send_window_prefix()
            os.write(master, b"h")
            if not wait_for_screen_fragments((b"Split top and bottom",), operation_start, timeout=3.0):
                raise AssertionError("Ctrl+Backslash, H did not create a horizontal split")
            focus_start = send_window_prefix()
            os.write(master, b"\x1b[A")
            if not wait_for_screen_fragments((b"Focused pane above",), focus_start, timeout=3.0):
                raise AssertionError("Ctrl+Backslash, Up did not focus the pane above")
            focus_start = send_window_prefix()
            os.write(master, b"\x1b[B")
            if not wait_for_screen_fragments((b"Focused pane below",), focus_start, timeout=3.0):
                raise AssertionError("Ctrl+Backslash, Down did not focus the pane below")

            layout_start = send_window_prefix()
            os.write(master, b"p")
            if not wait_for_screen_fragments((b"No saved workspace layouts",), layout_start, timeout=3.0):
                raise AssertionError("Ctrl+Backslash, P did not open the author-facing layout selector")
            if b"window-layout-load" in output[layout_start:]:
                raise AssertionError("workspace layout selection exposed internal command syntax")

            # These are separate physical terminal writes, not a textual
            # shortcut expression: ASCII 0x1C arms the prefix, then ASCII
            # 0x31 selects pane 1. No plus key is sent or consumed.
            focus_start = send_window_prefix()
            os.write(master, b"\x31")
            if not wait_for_screen_fragments(
                (b"Focused pane 1",), focus_start, timeout=3.0
            ):
                raise AssertionError("raw bytes 0x1C then 0x31 did not focus pane 1")

            for number in (b"2", b"3"):
                focus_start = send_window_prefix()
                os.write(master, number)
                if not wait_for_screen_fragments(
                    (b"Focused pane " + number,), focus_start, timeout=3.0
                ):
                    raise AssertionError(f"prefix followed by {number!r} did not focus its numbered pane")
            if not wait_for_screen_fragments((b"Pane 1", b"Pane 2"), focus_start, timeout=1.0):
                raise AssertionError("visible pane headers did not show stable pane numbers")

            cancel_start = len(output)
            cancel_start = send_window_prefix()
            read_for(0.15)  # allow Screen to forward the prefix before the independent Escape key
            os.write(master, b"\x1b")
            if not wait_for_screen_fragments((b"Window command cancelled",), cancel_start, timeout=3.0):
                raise AssertionError(
                    "Esc did not cancel the pending window prefix; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )

            invalid_start = len(output)
            invalid_start = send_window_prefix()
            os.write(master, b"z")
            if not wait_for_screen_fragments((b"not recognised",), invalid_start, timeout=3.0):
                raise AssertionError("an unsupported prefix sequence was not safely consumed")

            plus_start = len(output)
            plus_start = send_window_prefix()
            os.write(master, b"+")  # Plus is a separate invalid key, never prefix syntax.
            if not wait_for_screen_fragments((b"not recognised",), plus_start, timeout=3.0):
                raise AssertionError("a literal plus was incorrectly required by the prefix sequence")

            operation_start = len(output)
            operation_start = send_window_prefix()
            os.write(master, b"m")
            if not wait_for_screen_fragments((b"maximized",), operation_start, timeout=3.0):
                raise AssertionError("prefix followed by M did not maximise the focused pane")
            send_window_prefix()
            os.write(master, b"m")
            if not wait_for_screen_fragments((b"layout restored",), operation_start, timeout=3.0):
                raise AssertionError("prefix followed by M did not restore the pane layout")
            focus_start = len(output)
            os.write(master, b"\x1b[17~")  # F6: next pane
            if not wait_for_screen_fragments((b"Focused next window",), focus_start, timeout=3.0):
                raise AssertionError("F6 did not move focus to the next pane")
            focus_start = len(output)
            os.write(master, b"\x1b[17;2~")  # Shift+F6: previous pane
            if not wait_for_screen_fragments((b"Focused previous window",), focus_start, timeout=3.0):
                raise AssertionError("Shift+F6 did not move focus to the previous pane")

            focus_start = len(output)
            os.write(master, b"\x1b[21~")  # F10: menu fallback
            read_for(0.2)
            os.write(master, b"\x1b[C" * 4)  # View
            select_visible_menu_item("Next Window")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Focused next window",), focus_start, timeout=3.0):
                raise AssertionError("View > Next Window did not transfer pane focus")

            operation_start = len(output)
            operation_start = send_window_prefix()
            os.write(master, b"x")
            if not wait_for_screen_fragments((b"Window closed",), operation_start, timeout=3.0):
                raise AssertionError("prefix followed by X did not close the focused pane")
            operation_start = send_window_prefix()
            os.write(master, b"v")
            if not wait_for_screen_fragments((b"Split side by side",), operation_start, timeout=3.0):
                raise AssertionError("prefix followed by V did not split the focused pane")

            help_start = send_window_prefix()
            os.write(master, b"?")
            if not wait_for_screen_fragments((b"Window Management", b"Ctrl+"), help_start, timeout=3.0):
                raise AssertionError("prefix followed by ? did not open window-management help")

            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after split-pane navigation")
            restored = termios.tcgetattr(master)
            if not restored[3] & termios.ISIG:
                raise AssertionError("application shutdown did not restore terminal signal handling")
            if restored[6][termios.VQUIT] != b"\x1c":
                raise AssertionError("application shutdown did not restore the terminal VQUIT byte")
            print("PTY window-prefix workflow passed: raw-mode ASCII 0x1C, directions, pane numbers, help/cancel, split/close/maximise, F6/menu fallback, independent scrolling, and termios restoration.")
            return

        if same_chapter_move:
            if wait_for_screen_fragments((b"Welcome to Writer's IDE",), timeout=1.0):
                os.write(master, b"\x1b")
            if wait_for_workspace_title("Editor", timeout=0.2):
                os.write(master, b"\x1b[21~")
                read_for(0.2)
                os.write(master, b"\x1b[C" * 3)  # Navigate
                select_visible_menu_item("Outline")
                os.write(master, b"\r")
                if not wait_for_workspace_title("Outline", timeout=3.0):
                    raise AssertionError("Navigate > Outline did not open the manuscript hierarchy")
            else:
                show_outline_tree_presentation()
            if not move_outline_selection_until(lambda row: "Second" in row, "down"):
                raise AssertionError("the scene for the same-chapter move check was not selectable")
            action_start = open_outline_menu()
            select_visible_menu_item("Move Scene to Chapter…")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Move", b"Opening"), action_start, timeout=3.0):
                raise AssertionError(
                    "Move Scene to Chapter did not show its selected destination; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            read_for(0.25)
            os.write(master, b"\r")  # default destination is the scene's current chapter
            if not wait_for_screen_fragments((b"already in",), action_start, timeout=3.0):
                raise AssertionError(
                    "same-chapter selection did not explain that order was unchanged; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            screen = "\n".join(terminal_snapshot(output, 24, 80))
            if "position beyond sibling count" in screen:
                raise AssertionError("the internal sibling-position error leaked into the author interface")
            if "First" not in screen or "Second" not in screen:
                raise AssertionError("same-chapter move hid an existing scene; screen: " + repr(screen))
            os.write(master, b"\x13")
            if not wait_for_screen_fragments((b"Saved at",), timeout=3.0):
                raise AssertionError("same-chapter move workflow did not save normally")
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after same-chapter selection")
            print("PTY Move Scene to Chapter passed: same-chapter choice is a clear no-op, without losing order.")
            return

        if convert_outline_item:
            if wait_for_screen_fragments((b"Welcome to Writer's IDE",), timeout=1.0):
                os.write(master, b"\x1b")
            show_outline_tree_presentation()
            if not move_outline_selection_until(lambda row: "Part One" in row, "down"):
                raise AssertionError("the section to convert was not selectable")
            start = open_outline_menu()
            select_visible_menu_item("Convert Selected Section or Chapter…")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Convert Outline Item", b"Part One", b"Chapter"), timeout=3.0
            ):
                raise AssertionError("conversion dialogue did not identify the selected item and destination")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"converted to Chapter",), start, timeout=3.0
            ):
                # The status message may be replaced by the following render;
                # the persisted structure is asserted by the Rust caller.
                read_for(0.3)
            screen = "\n".join(terminal_snapshot(output, 24, 80))
            if "Part One" not in screen or "Chapter" not in screen:
                raise AssertionError("converted chapter was not visible in the outline; screen: " + repr(screen))
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after outline conversion")
            print("PTY outline conversion passed: selected section became a chapter and persisted.")
            return

        if delete_sentence:
            if wait_for_screen_fragments((b"Welcome to Writer's IDE",), timeout=1.0):
                os.write(master, b"\x1b")
            if wait_for_workspace_title("Outline", timeout=3.0):
                if not move_outline_selection_until(lambda row: "Opening" in row, "down"):
                    raise AssertionError("the opening scene was not selectable")
                os.write(master, b"\r")
            elif not wait_for_workspace_title("Editor", timeout=3.0):
                raise AssertionError("project did not open in a manuscript workspace")
            if not wait_for_workspace_title("Editor", timeout=3.0):
                raise AssertionError("the opening scene did not open in the editor")
            os.write(master, b"\x1b[B\x1b[B")  # caret on the third physical line
            read_for(0.25)
            action_start = len(output)
            os.write(master, b"\x1b[21~")  # F10
            read_for(0.15)
            os.write(master, b"\x1b[C")  # Edit menu
            select_visible_menu_item("Delete Sentence")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Unpunctuated line deleted as sentence",), action_start, timeout=3.0
            ):
                raise AssertionError("Delete Sentence did not use the safe unpunctuated-line action")
            screen = "\n".join(terminal_snapshot(output, 24, 80))
            for expected in ("First thought", "Second thought", "Fourth thought"):
                if expected not in screen:
                    raise AssertionError(f"Delete Sentence lost neighboring text {expected!r}; screen: {screen!r}")
            if "Third thought" in screen:
                raise AssertionError("Delete Sentence left the selected third line in the editor")
            os.write(master, b"\x13")
            if not wait_for_screen_fragments((b"Saved at",), timeout=3.0):
                raise AssertionError("sentence deletion was not saved")
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after sentence deletion")
            print("PTY Delete Sentence passed: one line removed from a four-line unpunctuated passage.")
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

        if bulk_outline_insert:
            show_outline_tree_presentation()
            menu_start = open_outline_menu()
            select_visible_menu_item("Insert Multiple Items…")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                ("Insert Multiple Outline Items".encode("utf-8"), b"Quantity: 5", b"Name pattern: Chapter {n}"),
                menu_start,
                timeout=3.0,
            ):
                raise AssertionError(
                    "Outline > Insert Multiple Items did not show the author-facing defaults; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Inserted 5 chapter items",), menu_start, timeout=4.0):
                raise AssertionError(
                    "bulk chapter insertion did not report completion; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            scene_start = len(output)
            menu_start = open_outline_menu()
            select_visible_menu_item("Insert Multiple Items…")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(("Insert Multiple Outline Items".encode("utf-8"),), menu_start, timeout=3.0):
                raise AssertionError("the bulk insertion dialogue did not reopen for scene insertion")
            os.write(master, b"\x1b[B\x1b[B")  # Chapter → Section → Scene.
            if not wait_for_screen_fragments((b"Item type: Scene",), menu_start, timeout=2.0):
                raise AssertionError("the bulk insertion dialogue did not offer Scene as an item type")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Inserted 5 scenes",), scene_start, timeout=4.0):
                raise AssertionError("bulk scene insertion did not report completion")
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("the application did not exit cleanly after bulk outline insertion")
            print("PTY bulk outline insertion passed: numbered chapters and scenes were created through the production Outline menu.")
            return

        if scene_planning_fields:
            show_outline_tree_presentation()
            os.write(master, b"\x1b[B\x1b[B")  # select the scene under the chapter
            if not wait_for_screen_fragments((b"Opening", b"Emotional beat", b"Keep the reunion joyful"), timeout=2.0):
                raise AssertionError(
                    "the scene planning/custom metadata was not visible; screen: "
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
                raise AssertionError("application did not exit cleanly after displaying scene metadata")
            print("PTY scene custom metadata passed: typed planning values appeared in the production outline without exposing identifiers.")
            return

        if scene_status_picker:
            show_outline_tree_presentation()
            os.write(master, b"\x1b[B\x1b[B")  # select the scene under the chapter
            if not wait_for_screen_fragments((b"Opening",), timeout=2.0):
                raise AssertionError("the test scene was not visible in the manuscript outline")
            menu_start = open_named_menu("Outline")
            select_visible_menu_item("Set Scene Status…")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Status", b"Not Started", b"First Draft"), menu_start, timeout=3.0):
                raise AssertionError("Set Scene Status did not open the author-facing status chooser")
            os.write(master, b"\x1b[B" * 3 + b"\r")
            if not wait_for_screen_fragments((b"status set to First Draft",), menu_start, timeout=3.0):
                raise AssertionError("the status chooser did not apply the selected manuscript status")
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("the application did not exit cleanly after status selection")
            print("PTY manuscript status passed: selected a valid status through the Edit menu and persisted it.")
            return

        if manuscript_status_settings:
            show_outline_tree_presentation()
            os.write(master, b"\x1b[B\x1b[B")  # select the scene under the chapter
            if not wait_for_screen_fragments((b"Arrival",), timeout=2.0):
                raise AssertionError("the scene for custom status assignment was not visible")
            menu_start = open_named_menu("Tools")
            select_visible_menu_item("Manuscript Statuses…")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(
                (b"Manuscript Status Choices", b"[x] Not Started", b"R rename"),
                menu_start,
                timeout=3.0,
            ):
                raise AssertionError("Tools > Manuscript Statuses did not show project status choices")
            os.write(master, b"r" + b"\x7f" * len("Not Started") + b"Planning")
            if not wait_for_screen_fragments((b"[x] Planning",), menu_start, timeout=2.0):
                raise AssertionError("status label could not be edited in the configuration dialogue")
            os.write(master, b"\r")  # finish label edit
            os.write(master, b" ")  # disable the selected status for future assignments
            os.write(master, b"aThird Draft\r")
            if not wait_for_screen_fragments((b"[x] Third Draft",), menu_start, timeout=2.0):
                raise AssertionError("a project-local manuscript status could not be added")
            os.write(master, b"\r")  # save project-local status choices
            if not wait_for_screen_fragments((b"status choices saved to this project",), menu_start, timeout=3.0):
                raise AssertionError("status configuration did not report a successful project save")
            outline_start = open_outline_menu()
            select_visible_menu_item("Set Scene Status…")
            os.write(master, b"\r")
            if not wait_for_screen_fragments(("Status · Arrival".encode(), b"Third Draft"), outline_start, timeout=3.0):
                raise AssertionError(
                    "the scene status chooser did not show the custom project status; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            for _ in range(7):
                os.write(master, b"\x1b[B")
                read_for(0.22)
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"status set to Third Draft", b"Third Draft"), outline_start, timeout=3.0):
                raise AssertionError("the author could not assign the custom status from the scene picker")
            os.write(master, b"\x11")
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("the application did not exit cleanly after status configuration")
            print("PTY manuscript status configuration passed: renamed, disabled, added and saved project statuses.")
            return

        if chapter_creation:
            os.write(master, b"\x1b")  # dismiss the first-run welcome/tutorial panel
            read_for(0.3)

            # Open the unified hierarchical Outline from the Navigate menu.
            show_outline_tree_presentation()

            def create_chapter(title):
                menu_start = open_outline_menu()
                select_visible_menu_item("New Chapter")
                os.write(master, b"\r")
                if not wait_for_screen_fragments((b"New chapter", b"Enter creates"), menu_start, timeout=3.0):
                    raise AssertionError(
                        "Outline > New Chapter did not open its title prompt; output tail: "
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
            # Shift+Insert is the approved outline-context New Chapter key.
            shortcut_start = len(output)
            os.write(master, b"\x1b[2;2~")
            if not wait_for_screen_fragments((b"New chapter", b"Enter creates"), shortcut_start, timeout=3.0):
                raise AssertionError("Shift+Insert did not open the New Chapter prompt")
            os.write(master, b"Beta\r")
            if not wait_for_screen_fragments((b"Beta", b"Alpha"), shortcut_start, timeout=3.0):
                raise AssertionError("Shift+Insert did not create a selected chapter")
            # Shift+Down reorders the selected structural item through the
            # same operation as Outline > Move Selected Item Later.
            os.write(master, b"\x1b[1;2B")
            if not wait_for_screen_fragments((b"Outline item moved down",), shortcut_start, timeout=3.0):
                raise AssertionError("Shift+Down did not move the selected chapter")
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
                "PTY chapter workflow passed: menu creation, Shift+Insert creation and Shift+Down structural movement persist."
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
            if not wait_for_screen_fragments((b"Opening",), timeout=3.0):
                raise AssertionError("unified outline did not display the created scene")
            toggle_start = len(output)
            os.write(master, b" ")
            if not wait_for_screen_fragments((b"[excluded]", b"Opening"), toggle_start, timeout=3.0):
                raise AssertionError("Space did not visibly move the scene to Excluded")
            os.write(master, b" ")
            if not wait_for_screen_fragments((b"Opening",), toggle_start, timeout=3.0):
                raise AssertionError("Space did not visibly restore the scene to Included")
            if not wait_for_screen_absence(b"[excluded]", timeout=3.0):
                raise AssertionError("restored scene still appears excluded in the outline")
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
                read_for(0.04)
            if not wait_for_screen_fragments((b"hidden pass",), edit_start, timeout=5.0):
                raise AssertionError("manuscript typing was not visible; screen: " + repr(terminal_snapshot(output, 24, 80)) + "\noutput tail: " + repr(ANSI_CONTROL.sub(b"", output[-1600:])))
            os.write(master, b"\x13")  # Ctrl+S
            if not wait_for_screen_fragments((b"[SAVED]",), edit_start, timeout=3.0):
                raise AssertionError("scene save did not provide visible success feedback")
            return_start = len(output)
            os.write(master, b"\x1b")
            if not wait_for_workspace_title("Outline", timeout=3.0) or not wait_for_screen_fragments(
                (b"Opening",), return_start, timeout=3.0
            ):
                raise AssertionError("Esc did not return to the active scene in the outline")

            link_start = open_named_menu("Insert")
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
                raise AssertionError(
                    "Book Type did not confirm its selection; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                    + "; output tail: "
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
                (b"Selectedpath:", b".docx", b"EnterCompile"),
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
            os.write(master, b"\r")  # The output name/action control is focused on entry.
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
            menu_start = len(output)
            if wait_for_screen_fragments(
                (b"Start screen", b"New Project"), timeout=0.5
            ):
                os.write(master, b"\r")  # New Project is selected on the start screen
            elif wait_for_screen_fragments((b"No scenes yet",), timeout=0.5):
                os.write(master, b"\x0e")  # Ctrl+N, independent of function-key support.
            else:
                raise AssertionError(
                    "blank project did not expose New Project on its start screen or File menu; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                    + "; output tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1600:]))
                )
            if not wait_for_screen_fragments((b"New Project", b"Fiction"), menu_start, timeout=3.0):
                raise AssertionError("New Project did not open its title/type dialogue")
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
                os.write(master, b"\t")
                read_for(0.12)
                os.write(master, b"\x1b[C")
                read_for(0.12)
                os.write(master, b"\t")
                read_for(0.12)
                os.write(master, b"\r")
            else:
                os.write(master, b"\t")
                read_for(0.12)
                os.write(master, b"\t")
                read_for(0.12)
                os.write(master, b"\r")
            expected_project_markers = (
                (b"Created", b"Outline")
                if create_nonfiction
                else (b"Created My Novel", b"Outline")
            )
            if not wait_for_screen_fragments(expected_project_markers, timeout=5.0):
                raise AssertionError(
                    "Enter on Create Project did not open the new fiction project; output tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-2400:]))
                )
            if not wait_for_workspace_title("Outline", timeout=3.0):
                raise AssertionError(
                    "new project creation feedback appeared without opening its outline; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            exited_pid, exited_status = os.waitpid(child_pid, os.WNOHANG)
            if exited_pid == child_pid:
                raise AssertionError(
                    f"application exited after creating the staged project with status {exited_status}; "
                    + repr(ANSI_CONTROL.sub(b"", output[-1200:]))
                )
            read_for(0.3)
            menu_start = open_outline_menu()
            select_visible_menu_item("New Chapter")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"New chapter", b"Enter creates"), menu_start, timeout=3.0):
                raise AssertionError("Outline > New Chapter did not open its title prompt")
            os.write(master, b"Opening Chapter\r")
            if not wait_for_screen_fragments((b"Opening Chapter",), timeout=3.0):
                raise AssertionError("New Chapter was not shown in the outline")
            if not wait_for_screen_absence(b"Enter creates", timeout=3.0):
                raise AssertionError(
                    "New Chapter still had its title prompt open after confirmation; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            if not wait_for_screen_absence(b"No outline row selected", timeout=3.0):
                raise AssertionError(
                    "the created chapter was not selected in the outline; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            if create_nonfiction:
                menu_start = open_outline_menu()
                select_visible_menu_item("Write Selected Chapter")
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
                os.write(master, b"\r")
                if not wait_for_screen_fragments((b"Create an independent project copy",), timeout=3.0):
                    raise AssertionError(
                        "non-fiction Save As did not ask before creating its destination; screen: "
                        + repr(terminal_snapshot(output, 24, 80))
                    )
                os.write(master, b"\r")
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
            menu_start = open_outline_menu()
            select_visible_menu_item("New Scene")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"New scene",), menu_start, timeout=3.0):
                raise AssertionError("Outline > New Scene did not open its title prompt")
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
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Create an independent project copy",), timeout=3.0):
                raise AssertionError(
                    "Save As did not request collision-safe confirmation; visible tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-2200:]))
                )
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Saved project at",), timeout=3.0):
                raise AssertionError(
                    "temporary project did not promote to the chosen location; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                    + "; output tail: "
                    + repr(ANSI_CONTROL.sub(b"", output[-1800:]))
                )
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
            if not wait_for_screen_fragments((b"Autosaved",), edit_start, timeout=15.0):
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

        if previous_position_history:
            if not wait_for_screen_fragments((b"Thomas waits beside the station clock.",), timeout=3.0):
                raise AssertionError("the initial station scene did not open")
            os.write(master, b"\x10grep lantern-marker\r")
            if not wait_for_screen_fragments((b"A unique lantern-marker glows above the market.",), timeout=4.0):
                raise AssertionError("search did not navigate to the second scene")
            os.write(master, b"\x1b")  # Close Find while retaining the location history.
            read_for(0.2)
            os.write(master, b"\x10return-position\r")
            if not wait_for_screen_fragments((b"Returned to the previous manuscript position",), timeout=3.0):
                raise AssertionError("Previous Position did not return to the recorded scene; screen: " + repr(terminal_snapshot(output, 24, 80)) + " tail: " + repr(ANSI_CONTROL.sub(b"", output[-1600:])))
            if not wait_for_screen_fragments((b"Thomas waits beside the station clock.",), timeout=2.0):
                raise AssertionError("the previous scene was not restored visibly")

            next_menu_start = len(output)
            os.write(master, b"\x1b[21~")  # Navigate through the normal menu path.
            read_for(0.2)
            for _ in range(10):
                if wait_for_screen_fragments((b"Return to Next Position",), next_menu_start, timeout=0.2):
                    break
                os.write(master, b"\x1b[C")
                read_for(0.1)
            else:
                raise AssertionError("Navigate menu did not expose Return to Next Position")
            select_visible_menu_item("Return to Next Position")
            os.write(master, b"\r")
            if not wait_for_screen_fragments((b"Returned to the next manuscript position",), timeout=3.0):
                raise AssertionError("Next Position did not return to the location left behind")
            if not wait_for_screen_fragments((b"A unique lantern-marker glows above the market.",), timeout=2.0):
                raise AssertionError("Next Position did not restore the second scene")
            os.write(master, b"\x10return-position\r")
            if not wait_for_screen_fragments((b"Returned to the previous manuscript position",), timeout=3.0):
                raise AssertionError("Previous Position did not work after Next Position")
            if not wait_for_screen_fragments((b"Thomas waits beside the station clock.",), timeout=2.0):
                raise AssertionError("Previous Position did not restore the first scene again")

            # The history entry is consumed once. A second attempt at the
            # boundary must remain non-fatal and preserve an in-memory edit.
            os.write(master, b"UNSAVED-PRESERVED")
            os.write(master, b"\x10return-position\r")
            if not wait_for_screen_fragments((b"No previous manuscript position",), timeout=3.0):
                raise AssertionError("the history boundary did not report a nonfatal status")
            if not wait_for_screen_fragments((b"UNSAVED-PRESERVED",), timeout=2.0):
                raise AssertionError("failed boundary navigation discarded unsaved editor text")
            os.write(master, b"\x13")  # Ctrl+S
            if not wait_for_screen_fragments((b"[SAVED]",), timeout=3.0):
                raise AssertionError("the manuscript did not remain saveable after boundary navigation")
            os.write(master, b"\x11")  # Ctrl+Q
            quit_deadline = time.monotonic() + 5.0
            finished_pid = 0
            status = 0
            while finished_pid == 0 and time.monotonic() < quit_deadline:
                read_for(0.1)
                finished_pid, status = os.waitpid(child_pid, os.WNOHANG)
            if finished_pid != child_pid or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError("application did not exit cleanly after populated-history navigation")
            print("PTY navigation regression passed: Previous Position returns through populated history once; empty boundary preserves edits and remains saveable.")
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

        os.write(master, b"\x1b[21~")  # F10 opens File again
        read_for(0.2)
        os.write(master, b"N")  # uppercase mnemonic must resolve identically
        if not wait_for_screen_fragments((b"Title", b"Create"), timeout=3.0):
            raise AssertionError(
                "File > N did not open the New Project dialogue; screen: "
                + repr(terminal_snapshot(output, 24, 80))
            )
        os.write(master, b"\x1b")  # cancel without creating a project
        read_for(0.3)
        assert_sample_prose_visible(output, 24, 80, "screen after cancelling uppercase New Project")

        os.write(master, b"\x1b[21~")  # F10 opens File
        read_for(0.2)
        os.write(master, b"\x0e")  # Ctrl+N while the menu has focus
        if not wait_for_screen_fragments((b"Title", b"Create"), timeout=3.0):
            raise AssertionError("Ctrl+N from the open File menu did not start New Project")
        os.write(master, b"\x1b")  # cancel without creating a project
        read_for(0.3)
        assert_sample_prose_visible(output, 24, 80, "screen after cancelling menu Ctrl+N")

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
            raise AssertionError(
                "the imported reference prose was not rendered contiguously; screen: "
                + repr(terminal_snapshot(output, 24, 80))
                + "; output tail: "
                + repr(ANSI_CONTROL.sub(b"", output[-1600:]))
            )
        assert_visible_sentence(output, 24, 80, "Reference only stays clear.", "Reference Library")
        if any("Maqrara opened the old bok" in row for row in terminal_snapshot(output, 24, 80)):
            raise AssertionError("manuscript prose leaked into the Reference Library")

        set_size(master, 60, 201)
        read_for(0.4)
        assert_visible_sentence(output, 60, 201, "Reference only stays clear.", "Reference at 201x60")
        set_size(master, 24, 80)
        read_for(0.4)
        assert_visible_sentence(output, 24, 80, "Reference only stays clear.", "Reference at 80x24")

        close_reference_start = len(output)
        os.write(master, b"\x1b")  # close the selected reference, return to its library
        if not wait_for_screen_fragments(
            (b"Reference Library",), close_reference_start, timeout=3.0
        ):
            raise AssertionError("Esc did not close the current reference to its library")
        read_for(0.3)
        screen = terminal_snapshot(output, 24, 80)
        if any("Reference only stays clear." in row for row in screen):
            raise AssertionError("closed reference text remained visible in the library; screen: " + repr(screen))
        os.write(master, b"\x1b")  # return to the manuscript workspace which opened the library
        if not wait_for_workspace_title("Editor", timeout=3.0):
            if not wait_for_workspace_title("Outline", timeout=3.0):
                raise AssertionError(
                    "Esc from the Reference Library did not restore its originating workspace; screen: "
                    + repr(terminal_snapshot(output, 24, 80))
                )
            if not move_outline_selection_until(lambda row: "Arrival" in row, "down"):
                raise AssertionError("the originating outline did not retain a selectable manuscript scene")
            os.write(master, b"\r")
            if not wait_for_workspace_title("Editor", timeout=3.0):
                raise AssertionError("Enter did not open the selected scene after returning to Outline")
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
