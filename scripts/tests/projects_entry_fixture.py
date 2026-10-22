"""Click Projects in the actual Sidebar and capture the isolated backend request."""
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import socket
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time


def check(binary, fail=False):
    with tempfile.TemporaryDirectory(prefix='hw-projects-entry-') as directory:
        base = Path(directory).resolve()
        root, profile, foreign = [base / name for name in ('demo repo', 'profile', 'foreign')]
        for path in (root, profile / 'sidebar', foreign):
            path.mkdir(parents=True)
        (root / 'visible.txt').write_text('fixture\n')
        original = b'{"board.open":"popup","foreign":true}'
        (foreign / 'settings.json').write_bytes(original)
        (profile / 'sidebar/state.json').write_text(json.dumps({
            'font_prompt': True, 'merged': True, 'icons': 'emoji', 'git_deco': False,
        }))
        tools = base / 'herdr'
        tools.write_text('#!' + sys.executable + '\nimport json\n'
                         'print(json.dumps({"pane":{"pane_id":"w1:p2",'
                         '"tab_id":"w1:t2","workspace_id":"w1"}}))\n')
        tools.chmod(0o755)
        server = socket.socket(socket.AF_UNIX)
        address = str(base / 'host.sock')
        server.bind(address)
        server.listen()
        server.settimeout(.1)
        requests = []
        stop = threading.Event()

        def host():
            while not stop.is_set():
                try:
                    connection, _ = server.accept()
                except socket.timeout:
                    continue
                with connection:
                    request = json.loads(connection.makefile('rb').readline())
                    requests.append(request)
                    response = ({'error': {'code': 'fixture_failure', 'message': 'host refused'}}
                                if fail else {'result': {'plugin_pane': {'pane': 'w1:p2'}}})
                    stop.wait(.15)  # Keep the first request pending while queued clicks are read.
                    connection.sendall(json.dumps(response).encode() + b'\n')

        thread = threading.Thread(target=host)
        thread.start()
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 32, 38, 0, 0))
        env = {key: value for key, value in os.environ.items() if not key.startswith('HERDR_')}
        env.update(TERM='xterm-256color', HERDR_SOCKET_PATH=address, HERDR_BIN_PATH=str(tools),
                   HERDR_PROJECTS_DATA_DIR=str(foreign), HERDR_PROJECTS_CONFIG_DIR=str(foreign),
                   HERDR_PLUGIN_STATE_DIR=str(foreign))

        def seat():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        process = subprocess.Popen([str(binary), '--home', str(profile), 'ui', '--sidebar', str(root)],
                                   stdin=slave, stdout=slave, stderr=slave, env=env, preexec_fn=seat)
        os.close(slave)
        output = bytearray()

        def wait_for(predicate):
            deadline = time.monotonic() + 12
            while time.monotonic() < deadline:
                if predicate():
                    return
                if select.select([master], [], [], .05)[0]:
                    try:
                        output.extend(os.read(master, 65536))
                    except OSError:
                        if predicate():
                            return
                        break
                if predicate():
                    return
            raise AssertionError('Projects entry timed out: ' + re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b'', output).decode(errors='replace')[-2000:] + repr(requests))

        try:
            wait_for(lambda: b'visible.txt' in output)
            # At 38 columns the Projects chip occupies columns 31..35, all three rows.
            os.write(master, b'\x1b[<0;32;2M' * 3)
            expected = 'Projects 打开失败' if fail else '已打开 HerdrWork Projects'
            wait_for(lambda: expected.replace(' ', '').encode() in re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b'', output).replace(b' ', b''))
            assert len(requests) == 1, 'Repeated clicks must share the pending request'
            request = requests[0]
            assert request['method'] == 'plugin.pane.open'
            params = request['params']
            assert params['plugin_id'] == 'jtblink.herdr-work'
            assert params['entrypoint'] == 'board' and params['placement'] == 'tab'
            assert params['cwd'] == str(root)
            assert params['env']['HERDR_WORK_HOME'] == str(profile)
            assert params['env']['HERDR_PROJECTS_DATA_DIR'] == str(profile / 'projects/data')
            assert (foreign / 'settings.json').read_bytes() == original
            assert sorted(path.name for path in foreign.iterdir()) == ['settings.json']
            if fail:
                # A failed request leaves the entry available for retry.
                os.write(master, b'\x1b[<0;32;2M')
                wait_for(lambda: len(requests) == 2)
            os.write(master, b'q')
            wait_for(lambda: process.poll() is not None)
            assert process.returncode == 0
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=5)
            os.close(master)
            stop.set()
            thread.join(timeout=2)
            server.close()


if __name__ == '__main__':
    for fail in (False, True):
        check(Path(sys.argv[1]).resolve(), fail)
    print('Projects entry: actual PTY, own backend/profile, duplicate clicks and retry passed')
