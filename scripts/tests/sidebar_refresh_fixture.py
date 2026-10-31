"""External edits refresh an unfocused Sidebar without keyboard refresh or focus calls."""
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

from preview_fixture import Layout


def check(binary):
    with tempfile.TemporaryDirectory(prefix='herdr-work-live-sidebar-') as directory:
        base = Path(directory).resolve()
        root = base / 'project'
        root.mkdir()
        (root / 'keep.txt').write_text('initial\n')
        (root / 'delete.txt').write_text('delete\n')

        def git(*args):
            subprocess.run(['git', '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                            '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=.git/no-hooks', *args],
                           cwd=root, check=True, capture_output=True)

        git('init', '-q')
        git('add', '.')
        git('commit', '-qm', 'initial fixture')
        profile = base / 'profile'
        (profile / 'sidebar').mkdir(parents=True)
        (profile / 'sidebar/state.json').write_text(json.dumps({
            'font_prompt': True, 'auto_open': False, 'merged': True, 'follow_cwd': False,
            'sidebar_width': 55, 'preview_placement': 'tab',
        }))
        address = str(base / 'host.sock')
        server = socket.socket(socket.AF_UNIX)
        server.bind(address)
        server.listen()
        server.settimeout(.1)
        stopped = threading.Event()
        requests = []
        argv = [binary, '--home', str(profile), 'ui', '--sidebar', str(root), '--initial-view', 'explorer']
        panes = [dict(pane_id=name, tab_id='w1:t1', workspace_id='w1', terminal_id='terminal-'+name,
                      cwd=str(root), foreground_cwd=str(root), focused=name == 'agent', tokens={})
                 for name in ['owner', 'agent']]
        layout = Layout(Layout.split('owner', 'agent', 55 / 150))

        def serve():
            while not stopped.is_set():
                try:
                    stream, _ = server.accept()
                except socket.timeout:
                    continue
                with stream:
                    request = json.loads(stream.makefile().readline())
                    requests.append(request)
                    method, params = request['method'], request.get('params', {})
                    result = {}
                    if method == 'pane.list':
                        result = {'panes': panes}
                    elif method == 'pane.layout':
                        result = {'layout': dict(layout.snapshot(), tab_id='w1:t1')}
                    elif method == 'workspace.list':
                        result = {'workspaces': [{'workspace_id': 'w1', 'label': 'fixture'}]}
                    elif method == 'pane.process_info':
                        command = argv if params['pane_id'] == 'owner' else ['/bin/sh']
                        result = {'process_info': {'foreground_processes': [{'argv0': command[0], 'argv': command}]}}
                    elif method == 'pane.report_metadata':
                        next(p for p in panes if p['pane_id'] == params['pane_id'])['tokens'].update(params['tokens'])
                    stream.sendall((json.dumps({'result': result}) + '\n').encode())

        worker = threading.Thread(target=serve, daemon=True)
        worker.start()
        env = {key: value for key, value in os.environ.items() if not key.startswith('HERDR_')}
        env.update(TERM='xterm-256color', HERDR_ENV='1', HERDR_SOCKET_PATH=address,
                   HERDR_PLUGIN_ID='jtblink.herdr-work', HERDR_PANE_ID='owner',
                   HERDR_TAB_ID='w1:t1', HERDR_WORKSPACE_ID='w1')
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 55, 0, 0))

        def seat():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        process = subprocess.Popen(argv, stdin=slave, stdout=slave, stderr=slave, env=env, preexec_fn=seat)
        os.close(slave)
        output = bytearray()

        def shows(text):
            plain = re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b'', bytes(output))
            return re.sub(rb'\s+', b'', text) in re.sub(rb'\s+', b'', plain)

        def wait_for(predicate, message):
            deadline = time.monotonic() + 12
            while time.monotonic() < deadline:
                if select.select([master], [], [], .03)[0]:
                    output.extend(os.read(master, 65536))
                if process.poll() is not None:
                    raise AssertionError(f'Sidebar exited: {process.returncode}; {bytes(output[-1500:])!r}')
                if predicate():
                    return
            raise AssertionError(f'{message}: {bytes(output[-2000:])!r}')

        try:
            wait_for(lambda: shows(b'keep.txt'), 'Explorer did not load')
            output.clear()
            (root / 'external-new.txt').write_text('external edit\n')
            wait_for(lambda: shows(b'external-new.txt'), 'Unfocused Explorer did not detect a new file')
            output.clear()
            (root / 'external-new.txt').rename(root / 'renamed-file.txt')
            wait_for(lambda: shows(b'renamed-file.txt'), 'Unfocused Explorer did not detect rename')
            git('add', '.')
            git('commit', '-qm', 'renamed fixture')
            output.clear()
            os.write(master, b'\x1b[23~')  # F11: switch to Source Control.
            wait_for(lambda: shows(b'Graph'), 'Source Control did not load')
            os.write(master, b'cunfinished draft')  # Focus the commit input, then type.
            wait_for(lambda: shows(b'unfinished draft'), 'Commit draft did not render')
            output.clear()
            (root / 'keep.txt').write_text('external modification\n')
            (root / 'delete.txt').unlink()
            wait_for(lambda: shows(b'keep.txt') and shows(b'delete.txt'), 'Unfocused Source Control did not detect modified/deleted files')
            output.clear()
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 56, 0, 0))
            wait_for(lambda: shows(b'unfinished draft'), 'Passive refresh lost the commit input')
            assert not any(r['method'] in ['pane.focus', 'tab.focus', 'workspace.focus'] for r in requests)
            assert next(p for p in panes if p['pane_id'] == 'agent')['focused']
            print('live Sidebar PTY: unfocused file creation/rename, Git modify/delete, draft retention and focus isolation passed')
        finally:
            os.close(master)
            if process.poll() is None:
                process.kill()
            process.wait(timeout=10)
            stopped.set()
            worker.join(timeout=5)
            server.close()


if __name__ == '__main__':
    check(str(Path(sys.argv[1]).resolve()))
