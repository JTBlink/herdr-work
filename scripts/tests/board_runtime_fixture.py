"""Exercise the actual board's first frame, typing and close in an isolated PTY."""
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time


def check(binary, cached=False):
    binary = Path(binary).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='hw-board-runtime-') as directory:
        base = Path(directory)
        profile = base / 'profile'
        data = profile / 'projects/data'
        data.mkdir(parents=True)
        # Avoid starting a keeper in this fixture: credential recovery belongs
        # to the keeper/worker and must not delay the board's first frame.
        (data / 'keeper.json').write_text('{}')
        marker = base / 'shell-called'
        shell = base / 'slow-shell'
        shell.write_text('#!/bin/sh\n: > "' + str(marker) + '"\nsleep 2\nexec /usr/bin/env\n')
        shell.chmod(0o755)
        host = base / 'host'
        host.write_text('#!' + sys.executable + '\nimport json\n'
                        'print(json.dumps({"result":{"panes":[],"workspaces":[],"machines":[],"tabs":[]}}))\n')
        host.chmod(0o755)
        env = {key: value for key, value in os.environ.items() if not key.startswith('HERDR_')}
        env.update(TERM='xterm-256color', HERDR_WORK_HOME=str(profile),
                   HERDR_PROJECTS_NO_KEEPER='1', HERDR_PROJECTS_NO_MODEL_LIST='1',
                   HERDR_PROJECTS_NO_HOOK='1', HERDR_PLUGIN_ID='jtblink.herdr-work',
                   HERDR_BIN_PATH=str(host), SHELL=str(shell))
        if cached:
            # Even a remembered credential name must not start a shell in UI startup.
            (data / 'env.json').write_text(json.dumps({
                'at': int(time.time() * 1000), 'PATH': env.get('PATH', ''),
                'gh': ['GH_TOKEN'], 'ssh': [],
            }))
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 32, 100, 0, 0))

        def seat():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        start = time.monotonic()
        process = subprocess.Popen([str(binary), 'board'], cwd=base, env=env,
                                   stdin=slave, stdout=slave, stderr=slave, preexec_fn=seat)
        os.close(slave)
        output = bytearray()

        def plain():
            return re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b'', output).replace(b' ', b'')

        def wait_for(predicate, timeout, message):
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                if select.select([master], [], [], .01)[0]:
                    try:
                        output.extend(os.read(master, 65536))
                    except OSError:
                        pass
                if predicate():
                    return
            raise AssertionError(message)

        try:
            wait_for(lambda: b'Nothinghereyet' in plain(), 4, 'Board first frame did not appear')
            first_ms = round((time.monotonic() - start) * 1000)
            assert '✕'.encode() in output, 'Actual board executable has no close button'
            assert not marker.exists(), 'Board startup synchronously launched a login shell'
            # A generous timing guard complements the deterministic shell-call assertion.
            assert first_ms < 1500, f'Board first frame took {first_ms} ms'
            output.clear()
            start = time.monotonic()
            os.write(master, b'\x1b[200~BOARD_LATENCY_MARKER\x1b[201~')
            wait_for(lambda: b'BOARD_LATENCY_MARKER' in plain(), 1, 'Board input stalled')
            typing_ms = round((time.monotonic() - start) * 1000)
            assert typing_ms < 300, f'Board input took {typing_ms} ms'
            os.write(master, b'\x1b[<0;2;1M\x1b[<0;2;1m')
            wait_for(lambda: process.poll() is not None, 2, 'Displayed close button did not exit the board')
            assert process.returncode == 0
            saved = json.loads((data / 'view.json').read_text())
            assert saved['draft'] == 'BOARD_LATENCY_MARKER', 'Closing lost the draft'
            print(f'board runtime: first frame {first_ms} ms, input {typing_ms} ms, '
                  f'close and draft preserved; cached={cached}')
        finally:
            os.close(master)
            # Closing our private PTY supplies SIGHUP; no user terminal is touched.
            process.wait(timeout=3)


if __name__ == '__main__':
    for cached in (False, True):
        check(sys.argv[1], cached)
