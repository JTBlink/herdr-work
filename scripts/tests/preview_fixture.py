"""Click the real Sidebar TUI against a private host with a protected plugin neighbor."""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time


def check(binary, placement):
    with tempfile.TemporaryDirectory(prefix='herdr-work-preview-') as directory:
        base = Path(directory)
        root = base / 'project'
        root.mkdir()
        (root / 'example-a.txt').write_text('first fixture\n')
        (root / 'example-b.txt').write_text('second fixture\n')
        profile = base / 'profile'
        (profile / 'sidebar').mkdir(parents=True)
        (profile / 'sidebar/state.json').write_text(json.dumps({
            'font_prompt': True, 'auto_open': False, 'merged': True,
            'sidebar_width': 28, 'preview_placement': placement,
        }))
        address = str(base / 'host.sock')
        server = socket.socket(socket.AF_UNIX)
        server.bind(address)
        server.listen()
        server.settimeout(.1)
        stopped = threading.Event()
        requests = []
        original_tokens = {'herdr-sidebar-explorer': '1', 'herdr-sidebar-git': '1'}
        panes = {
            name: {'pane_id': name, 'tab_id': 'w1:t1', 'workspace_id': 'w1',
                   'tokens': original_tokens.copy() if name == 'original' else {},
                   'cwd': str(root), 'foreground_cwd': str(root)}
            for name in ['owner', 'original', 'working']
        }
        argv = [binary, '--home', str(profile), 'ui', '--sidebar', str(root)]

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
                        result = {'panes': list(panes.values())}
                    elif method == 'plugin.list':
                        result = {'plugins': [{'plugin_id': 'herdr-sidebar', 'enabled': True}]}
                    elif method == 'workspace.list':
                        result = {'workspaces': [{'workspace_id': 'w1', 'label': 'fixture'}]}
                    elif method == 'pane.process_info':
                        pane = params['pane_id']
                        command = argv if pane == 'owner' else ['/bin/herdr-sidebar' if pane == 'original' else '/bin/sh']
                        result = {'process_info': {'pane_id': pane, 'foreground_processes': [
                            {'argv0': command[0], 'argv': command}]}}
                    elif method == 'pane.layout':
                        result = {'layout': {'tab_id': 'w1:t1',
                            'area': {'x': 0, 'y': 0, 'width': 120, 'height': 40},
                            'panes': [{'pane_id': name, 'rect': {'x': x, 'y': 0, 'width': width, 'height': 40}}
                                      for name, x, width in [('owner', 0, 28), ('original', 28, 28), ('working', 56, 64)]],
                            'splits': []}}
                    elif method == 'pane.split':
                        panes['viewer'] = {'pane_id': 'viewer', 'tab_id': 'w1:t1',
                                           'workspace_id': 'w1', 'tokens': {}}
                        result = {'pane': {'pane_id': 'viewer'}}
                    elif method == 'pane.report_metadata':
                        panes[params['pane_id']]['tokens'].update(params.get('tokens', {}))
                    stream.sendall((json.dumps({'result': result}) + '\n').encode())

        worker = threading.Thread(target=serve)
        worker.start()
        env = {k: v for k, v in os.environ.items() if not k.startswith('HERDR_')}
        env.update(TERM='xterm-256color', HERDR_ENV='1', HERDR_SOCKET_PATH=address,
                   HERDR_PLUGIN_ID='jtblink.herdr-work', HERDR_PANE_ID='owner',
                   HERDR_TAB_ID='w1:t1', HERDR_WORKSPACE_ID='w1')
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 28, 0, 0))

        def seat():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        process = subprocess.Popen(argv, stdin=slave, stdout=slave, stderr=slave, env=env, preexec_fn=seat)
        os.close(slave)
        output = bytearray()

        def drain():
            if select.select([master], [], [], .03)[0]:
                try:
                    output.extend(os.read(master, 65536))
                except OSError:
                    pass

        def wait_for(predicate):
            deadline = time.monotonic() + 12
            while time.monotonic() < deadline:
                drain()
                if predicate():
                    return
            raise AssertionError('Preview routing did not complete: ' + str([r['method'] for r in requests]))

        try:
            wait_for(lambda: b'example-b.txt' in output)
            # First tree row: x=5, y=5 in the TUI's SGR mouse coordinates.
            os.write(master, b'\x1b[<0;5;5M\x1b[<0;5;5m')
            wait_for(lambda: any(r['method'] == 'pane.send_input' and r['params'].get('pane_id') == 'viewer' for r in requests))
            deadline = time.monotonic() + .5
            while time.monotonic() < deadline:
                drain()
            os.write(master, b'\x1b[<0;5;6M\x1b[<0;5;6m')
            wait_for(lambda: any(b'example-b.txt' in path.read_bytes()
                                for path in (profile / 'sidebar/scratch').glob('*') if path.is_file()))
            splits = [r for r in requests if r['method'] == 'pane.split']
            assert len(splits) == 1, ('viewer not reused', splits)
            assert splits[0]['params']['target_pane_id'] == 'owner'
            assert not any(r['method'] in ['tab.create', 'pane.move'] for r in requests)
            mutations = [r for r in requests if r['method'] not in ['pane.list', 'pane.layout', 'pane.process_info', 'workspace.list', 'plugin.list']]
            assert not any(r['params'].get(key) == 'original' for r in mutations
                           for key in ['pane_id', 'source_pane_id', 'target_pane_id'])
            assert panes['original']['tokens'] == original_tokens
            print(f'{placement}: real mouse clicks reuse one inline viewer; independent plugin untouched')
        finally:
            os.close(master)
            stopped.set()
            worker.join(timeout=5)
            server.close()
            if process.poll() is None:
                process.kill()
            process.wait(timeout=15)


if __name__ == '__main__':
    for mode in ['replace', 'tab']:
        check(str(Path(sys.argv[1]).resolve()), mode)
