#!/usr/bin/env python3
"""Exercise the built terminal application through a real Linux PTY."""

import fcntl
import os
import pty
import select
import signal
import struct
import sys
import time


def set_size(fd, rows, columns):
    fcntl.ioctl(fd, 0x5414, struct.pack("HHHH", rows, columns, 0, 0))


def main():
    binary, project, isolated_home, *modes = sys.argv[1:]
    create_fiction = modes == ["create-fiction"]
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

    try:
        read_for(1.0)
        if create_fiction:
            if b"New Project" not in output:
                raise AssertionError("first-run project browser was not rendered")
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
            read_for(0.15)
            os.write(master, b"\x13")  # Ctrl+S
            read_for(0.35)
            os.write(master, b"\x11")  # Ctrl+Q
            read_for(1.0)
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
        os.write(master, b"\x1b[B\x1b[B\r")
        read_for(0.3)
        if b"Arrival" not in output:
            raise AssertionError(
                "the opening scene was not rendered in the PTY; output tail: "
                + repr(bytes(output[-1600:]))
            )

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
        read_for(1.0)
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
