#!/usr/bin/env python3
"""Unix PTY smoke test: python scripts/test-ui.py [--catalog Conan-search.json].

Uses a controlled Conan for discovery/failure cases, and real CMake/Ninja/CTest
for a build and a project script. Does not access the network or the user cache.
"""
import argparse
import codecs
import errno
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time


class Screen:
    """Decode cursor-addressed output so checks also work with terminal diffs."""
    def __init__(self):
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.pending = ""
        self.resize(120, 40)

    def resize(self, columns, rows):
        self.columns, self.rows = columns, rows
        self.cells = [[" "] * columns for _ in range(rows)]
        self.x = self.y = 0

    def feed(self, chunk):
        self.pending += self.decoder.decode(chunk)
        while self.pending:
            char = self.pending[0]
            if char == "\x1b":
                match = re.match(r"\x1b\[([0-?]*)([ -/]*)([@-~])", self.pending)
                if not match:
                    return
                args, _, code = match.groups()
                self.pending = self.pending[match.end():]
                if code in ("H", "f"):
                    coords = [int(n or "1") for n in args.split(";")]
                    self.y = min(self.rows - 1, max(0, coords[0] - 1))
                    self.x = min(self.columns - 1, max(0, (coords[1] if len(coords) > 1 else 1) - 1))
                elif (code == "J" and args in ("2", "3")) or (code in ("h", "l") and args == "?1049"):
                    self.resize(self.columns, self.rows)
                elif code == "K":
                    self.cells[self.y][self.x:] = [" "] * (self.columns - self.x)
                continue
            self.pending = self.pending[1:]
            if char == "\r":
                self.x = 0
            elif char == "\n":
                self.y += 1
            elif char >= " ":
                if self.x >= self.columns:
                    self.x = 0
                    self.y += 1
                if self.y >= self.rows:
                    self.cells.pop(0)
                    self.cells.append([" "] * self.columns)
                    self.y = self.rows - 1
                self.cells[self.y][self.x] = char
                self.x += 1
            if self.y >= self.rows:
                self.cells.pop(0)
                self.cells.append([" "] * self.columns)
                self.y = self.rows - 1

    def text(self):
        return "\n".join("".join(line) for line in self.cells)


class Terminal:
    def __init__(self, binary, project, env):
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.execve(binary, [str(binary), "-C", str(project), "--store-dir", str(project / "cache"), "--offline", "ui"], env)
        self.pending = b""
        self.history = b""
        self.closed = False
        self.screen = Screen()
        self.resize(120, 40)

    def resize(self, columns, rows):
        self.screen.resize(columns, rows)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        os.kill(self.pid, signal.SIGWINCH)

    def send(self, text):
        os.write(self.fd, text.encode())

    def expect(self, text, timeout=20):
        needle = text.encode()
        deadline = time.monotonic() + timeout
        while needle not in self.pending and text not in self.screen.text():
            if time.monotonic() >= deadline:
                raise AssertionError(f"Timed out waiting for {text!r}\n{self.screen.text()}")
            if select.select([self.fd], [], [], 0.1)[0]:
                try:
                    chunk = os.read(self.fd, 65536)
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
                    chunk = b""
                if not chunk:
                    raise AssertionError(f"Terminal exited while waiting for {text!r}\n{self.history[-4500:].decode(errors='replace')}")
                self.pending += chunk
                self.history += chunk
                self.screen.feed(chunk)
        if needle in self.pending:
            end = self.pending.index(needle) + len(needle)
            self.pending = self.pending[end:]
        else:
            self.pending = b""

    def quit(self):
        self.send("q")
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            pid, status = os.waitpid(self.pid, os.WNOHANG)
            if pid:
                assert os.waitstatus_to_exitcode(status) == 0
                flags = termios.tcgetattr(self.fd)[3]
                assert flags & termios.ECHO and flags & termios.ICANON, "terminal was not restored"
                os.close(self.fd)
                self.closed = True
                return
            time.sleep(0.02)
        raise AssertionError("UI did not exit")

    def close(self):
        if not self.closed:
            os.kill(self.pid, signal.SIGKILL)
            os.waitpid(self.pid, 0)
            os.close(self.fd)
            self.closed = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/cppm"))
    parser.add_argument("--catalog", type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    fixture = args.catalog.read_text() if args.catalog else json.dumps({"conancenter": {"novel-package/1.0": {}, "novel-package/2.0": {}, "libpq/17.11": {}}})
    with tempfile.TemporaryDirectory(prefix="cppm ui qa ") as directory:
        root = Path(directory)
        project = root / "project with spaces"
        subprocess.run([binary, "init", project, "--name", "ui-qa"], check=True, capture_output=True)
        manifest = project / "cppm.toml"
        with manifest.open("a") as stream:
            stream.write('\nbroken = ["python3", "-c", "import sys; sys.exit(7)"]\n')
        original = manifest.read_bytes()
        tools = root / "tools"
        tools.mkdir()
        catalog = root / "catalog.json"
        catalog.write_text(fixture)
        mock = tools / "conan"
        mock.write_text("#!/usr/bin/env python3\n" + '''import json, os, pathlib, sys, time
args = sys.argv[1:]
if args[0] in ("search", "list"):
    if os.environ.get("UI_QA_SLEEP"):
        pathlib.Path(os.environ["UI_QA_PID"]).write_text(str(os.getpid()))
        time.sleep(60)
    print(pathlib.Path(os.environ["UI_QA_FIXTURE"]).read_text())
elif args == ["--version"]:
    print("Conan version 2.34.0")
elif args == ["config", "home"]:
    home = pathlib.Path(os.environ["CONAN_HOME"])
    home.mkdir(parents=True, exist_ok=True)
    print(home)
elif args[0] == "profile":
    pass
else:
    print("controlled installation failure", file=sys.stderr)
    sys.exit(7)
''')
        mock.chmod(0o755)
        env = {k: v for k, v in os.environ.items() if not k.startswith("CPPM_") and k not in ("NO_COLOR", "CONAN_HOME")}
        env.update(PATH=str(tools) + os.pathsep + env.get("PATH", ""), TERM="xterm-256color", COLORTERM="truecolor", UI_QA_FIXTURE=str(catalog))
        ui = Terminal(binary, project, env)
        try:
            ui.expect("Explorar bibliotecas")
            ui.send("2")
            ui.expect("Catálogo atualizado:")
            ui.send("/postgres\r")
            ui.expect("libpq")
            ui.send(" \r")
            ui.expect("EXECUTAR AÇÃO")
            ui.send("nx")
            ui.send("a")
            ui.expect("ADICIONAR À SELEÇÃO")
            ui.send("json sdl2\r")
            ui.expect("pacote(s) na seleção")
            ui.send("\r")
            ui.expect("cppm add nlohmann_json@3.11.3 sdl@2.32.10")
            ui.send("\r")
            ui.expect("Pressione Enter")
            assert termios.tcgetattr(ui.fd)[3] & termios.ICANON
            assert manifest.read_bytes() == original, "failed install changed the manifest"
            assert not (project / "cppm.lock").exists(), "failed install created a lockfile"
            ui.send("\r")
            ui.expect("Falhou:")
            assert not termios.tcgetattr(ui.fd)[3] & termios.ICANON
            # Search the palette, build a real C++ application, and return.
            ui.send("\x10release\r\r")
            ui.expect("EXECUTAR AÇÃO")
            ui.send("\r")
            ui.expect("Pressione Enter", timeout=40)
            assert (project / ".cppm/build/release/bin/ui-qa").is_file()
            ui.send("\r")
            ui.expect("Concluído: cppm run build")
            ui.send("\x10broken\r\r")
            ui.expect("EXECUTAR AÇÃO")
            ui.send("\r")
            ui.expect("status 7")
            ui.expect("Pressione Enter")
            ui.send("\r")
            ui.expect("Falhou:")
            ui.send("\x10run test\r\r")
            ui.expect("EXECUTAR AÇÃO")
            ui.send("\r")
            ui.expect("100% tests passed", timeout=40)
            ui.expect("Pressione Enter")
            ui.send("\r")
            ui.expect("Concluído: cppm run test")
            ui.send("\x10obf-basic\r\r")
            ui.expect("EXECUTAR AÇÃO")
            ui.send("\r")
            ui.expect("No LLVM obfuscation plugin", timeout=40)
            ui.expect("Pressione Enter")
            assert (project / ".cppm/build/obf/bin/ui-qa").is_file()
            ui.send("\r")
            ui.expect("Concluído: cppm build obf --obf-basic")
            ui.resize(80, 24)
            ui.send("1")
            ui.expect("COMECE POR AQUI")
            ui.expect("Explorar bibliotecas")
            assert b"38;2;" in ui.history, "truecolor theme was not rendered"
            ui.quit()
        finally:
            ui.close()
        # A slow remote never blocks navigation, and quitting reaps its process.
        pid_file = root / "conan.pid"
        slow = Terminal(binary, project, dict(env, UI_QA_SLEEP="1", UI_QA_PID=str(pid_file)))
        try:
            slow.expect("Explorar bibliotecas")
            slow.send("2")
            slow.expect("Consultando receitas")
            deadline = time.monotonic() + 5
            while not pid_file.exists() and time.monotonic() < deadline:
                time.sleep(0.02)
            assert pid_file.exists()
            child_pid = int(pid_file.read_text())
            slow.send("4")
            slow.expect("Menos memória. Mais código.")
            slow.quit()
            try:
                os.kill(child_pid, 0)
            except ProcessLookupError:
                pass
            else:
                raise AssertionError("catalog subprocess survived UI exit")
        finally:
            slow.close()
        print("PTY OK: discovery, alias search, batch preview/cancel, failed install rollback, Release/obf builds, script failure, CTest, resume, resize, theme, terminal restoration and child cleanup.")


if __name__ == "__main__":
    if sys.platform == "win32":
        sys.exit("This PTY check requires Unix; use cargo test for portable state/render tests.")
    main()
