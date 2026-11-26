#!/usr/bin/env python3
"""Verify persistent staged-project recovery through the production terminal."""

import fcntl
import os
import pty
import select
import signal
import struct
import sys
import time
from pty_smoke import terminal_snapshot


def spawn(binary, home):
    pid, master = pty.fork()
    if pid == 0:
        environment = os.environ.copy()
        environment.update(
            {
                "TERM": "xterm-256color",
                "HOME": home,
                "XDG_CONFIG_HOME": os.path.join(home, "config"),
                "XDG_DATA_HOME": os.path.join(home, "data"),
                "TMPDIR": os.path.join(home, "tmp"),
            }
        )
        os.execve(binary, [binary], environment)
    fcntl.ioctl(master, 0x5414, struct.pack("HHHH", 24, 80, 0, 0))
    return pid, master


def wait_for(master, expected, timeout=12):
    data = bytearray()
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        ready, _, _ = select.select([master], [], [], 0.2)
        if not ready:
            continue
        try:
            chunk = os.read(master, 65536)
        except OSError:
            break
        if not chunk:
            break
        data.extend(chunk)
        visible = "\n".join(terminal_snapshot(bytes(data), 24, 80)).encode()
        if expected in visible:
            return bytes(data)
    raise AssertionError(
        f"did not see {expected!r}; terminal output tail: {bytes(data[-3000:])!r}"
    )


def finish(pid, master):
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        done, status = os.waitpid(pid, os.WNOHANG)
        if done == pid:
            os.close(master)
            if not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                raise AssertionError(f"application exited with status {status}")
            return
        time.sleep(0.05)
    os.kill(pid, signal.SIGKILL)
    os.waitpid(pid, 0)
    os.close(master)
    raise AssertionError("application did not exit after explicit discard")


def main():
    binary, home = sys.argv[1:]
    os.makedirs(os.path.join(home, "tmp"), exist_ok=True)
    pid, master = spawn(binary, home)
    wait_for(master, b"checkpoint.")
    os.kill(pid, signal.SIGKILL)
    os.waitpid(pid, 0)
    os.close(master)

    pid, master = spawn(binary, home)
    wait_for(master, b"checkpoint.")
    os.write(master, b"\x11")
    wait_for(master, b"Save Project and Quit")
    os.write(master, b"d")
    finish(pid, master)
    print("PTY staging recovery passed: edit → abrupt termination → automatic project recovery → explicit discard.")


if __name__ == "__main__":
    main()
