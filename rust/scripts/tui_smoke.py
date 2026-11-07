#!/usr/bin/env python3
"""Drive `jv -i` in a pseudo-terminal and compare the screens of two builds.

usage: tui_smoke.py JV_BIN [OTHER_JV_BIN]

Requires the `pyte` terminal emulator (pip install pyte). For each scenario
the given keys are typed into the viewer and the rendered screen (text only,
no colors) is captured. With two binaries, their screens are compared line
by line. Each run also checks that the viewer exits with status 0 and leaves
the alternate screen. Input is piped through stdin (`cat FILE | jv -i`), so
keys must be read from the controlling terminal.
"""
import os
import pty
import select
import sys
import time

import pyte

ROWS, COLS = 20, 70
SAMPLE = os.path.join(os.path.dirname(__file__), "..", "..", "testdata", "sample.json")

SCENARIOS = [
    ("initial view", [], []),
    ("navigate and expand", ["j", "j", "j", "j", "j", "l", "j"], []),
    ("collapse to parent", ["j"] * 6 + ["h", "h"], []),
    ("search text", ["/", "Tok", "\r"], []),
    ("search next", ["/", "e", "\r", "n", "n"], []),
    ("search path", ["/", "$.projects[1].status", "\r"], []),
    ("search not found", ["/", "zzz", "\r"], []),
    ("depth keys", ["1"], []),
    ("open all + bottom", ["o", "G"], []),
    ("close all", ["O"], []),
    ("type hints", ["t"], []),
    ("help", ["?"], []),
    ("detail view", ["/", "$.misc", "\r", "v"], []),
    ("page down", ["o", "\x04"], []),
    ("focus option", [], ["-p", ".projects[*].name"]),
    ("depth option", [], ["-d", "0"]),
]


def run(binary, keys, extra_args):
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.environ.pop("NO_COLOR", None)
        import fcntl, struct, termios

        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        args = " ".join(["'%s'" % a for a in extra_args])
        os.execvp("sh", ["sh", "-c", f"cat '{SAMPLE}' | '{binary}' -i {args}"])
    screen = pyte.Screen(COLS, ROWS)
    stream = pyte.ByteStream(screen)
    raw = bytearray()

    def pump(timeout):
        end = time.time() + timeout
        while time.time() < end:
            r, _, _ = select.select([fd], [], [], 0.05)
            if r:
                try:
                    data = os.read(fd, 65536)
                except OSError:
                    return False
                if not data:
                    return False
                raw.extend(data)
                stream.feed(data)
                # answer terminal queries like a real terminal would (the Go
                # build asks for the background color and cursor position)
                if b"\x1b]11;?" in data:
                    os.write(fd, b"\x1b]11;rgb:0000/0000/0000\x1b\\")
                if b"\x1b[6n" in data:
                    os.write(fd, b"\x1b[1;1R")
        return True

    pump(1.5)
    for k in keys:
        os.write(fd, k.encode())
        pump(0.25)
    pump(0.3)
    lines = [line.rstrip() for line in screen.display]
    # Esc leaves the detail/help/search views (where q does not quit)
    os.write(fd, b"\x1b")
    pump(0.4)
    os.write(fd, b"q")
    alive = pump(1.5)
    done, status = os.waitpid(pid, os.WNOHANG)
    if done == 0:  # did not quit
        os.kill(pid, 9)
        _, status = os.waitpid(pid, 0)
    code = os.waitstatus_to_exitcode(status)
    left_alt = b"\x1b[?1049l" in raw
    return lines, code, left_alt, alive


def main():
    bins = sys.argv[1:]
    failures = 0
    for name, keys, extra in SCENARIOS:
        results = [run(b, keys, extra) for b in bins]
        for b, (lines, code, left_alt, _) in zip(bins, results):
            if code != 0 or not left_alt:
                print(f"FAIL {name}: {b} exited {code}, left alt screen: {left_alt}")
                failures += 1
        if len(results) == 2:
            a, b = results[0][0], results[1][0]
            diffs = [(i, x, y) for i, (x, y) in enumerate(zip(a, b)) if x != y]
            status = "same" if not diffs else f"{len(diffs)} line(s) differ"
            print(f"{'ok  ' if not diffs else 'DIFF'} {name}: {status}")
            for i, x, y in diffs:
                print(f"     row {i:2}  A: {x!r}\n             B: {y!r}")
        else:
            print(f"--- {name}")
            print("\n".join(results[0][0]))
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
