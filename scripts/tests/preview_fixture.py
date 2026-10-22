"""Click the real Sidebar TUI against a private host with a protected plugin neighbor."""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import socket
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time


class Layout:
    """Small host split tree, so assertions measure occupied space after clicks."""
    def __init__(self, root):
        self.root = root

    @staticmethod
    def split(target, incoming, ratio=.5, direction='right'):
        return (direction, ratio, target, incoming)

    def replace(self, target, replacement):
        def visit(node):
            if isinstance(node, str):
                return replacement if node == target else node
            direction, ratio, first, second = node
            first, second = visit(first), visit(second)
            return (direction, ratio, first, second) if first and second else first or second
        self.root = visit(self.root)

    def snapshot(self):
        panes, splits = [], []

        def visit(node, rect):
            if isinstance(node, str):
                panes.append({'pane_id': node, 'rect': rect})
                return
            direction, ratio, first, second = node
            splits.append({'direction': direction, 'ratio': ratio, 'rect': rect})
            a, b = rect.copy(), rect.copy()
            axis, size = ('x', 'width') if direction == 'right' else ('y', 'height')
            a[size] = round(rect[size] * ratio)
            b[axis] += a[size]
            b[size] -= a[size]
            visit(first, a)
            visit(second, b)

        area = {'x': 0, 'y': 0, 'width': 120, 'height': 40}
        visit(self.root, area)
        return {'area': area, 'panes': panes, 'splits': splits}


def check(binary, placement, blocked=False, history=False):
    with tempfile.TemporaryDirectory(prefix='herdr-work-preview-') as directory:
        base = Path(directory).resolve()
        root = base / 'project'
        root.mkdir()
        (root / 'example-a.txt').write_text('first fixture\n')
        (root / 'example-b.txt').write_text('second fixture\n')
        if history:
            for args in [['init', '-q'], ['add', '.'],
                         ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                          '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=.git/no-hooks',
                          'commit', '-qm', 'history fixture']]:
                subprocess.run(['git', *args], cwd=root, check=True, capture_output=True)
            (root / 'example-a.txt').write_text('modified fixture\n')
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
                   'terminal_id': 'terminal-' + name,
                   'tokens': original_tokens.copy() if name == 'original' else {},
                   'cwd': str(root), 'foreground_cwd': str(root)}
            for name in ['owner', 'original', 'working']
        }
        # The independent Sidebar sits outside owner + working, as in the
        # reported layout. Replace must fill that working region, not split
        # another narrow preview into it.
        layouts = {'w1:t1': Layout(
            Layout.split('owner', Layout.split('original', 'working', 28 / 92), 28 / 120)
            if blocked else Layout.split(
                Layout.split('owner', 'working', 28 / 92), 'original', 92 / 120))}
        before = layouts['w1:t1'].snapshot()
        original_rect = next(p['rect'] for p in before['panes'] if p['pane_id'] == 'original')
        next_tab = 1
        argv = [binary, '--home', str(profile), 'ui', '--sidebar', str(root)]
        if history:
            argv.extend(['--initial-view', 'source-control'])

        def serve():
            nonlocal next_tab
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
                        tab = panes[params['pane_id']]['tab_id']
                        result = {'layout': dict(layouts[tab].snapshot(), tab_id=tab)}
                    elif method == 'tab.create':
                        next_tab += 1
                        tab = f'w1:t{next_tab}'
                        name = 'parking' if params['label'] == 'Working panes (temporary preview)' else 'viewer'
                        panes[name] = {'pane_id': name, 'tab_id': tab, 'workspace_id': 'w1',
                                       'terminal_id': 'terminal-' + name, 'tokens': {}}
                        layouts[tab] = Layout(name)
                        result = {'tab': {'tab_id': tab}, 'root_pane': {'pane_id': name}}
                    elif method == 'pane.split':
                        target = params['target_pane_id']
                        tab = panes[target]['tab_id']
                        panes['viewer'] = {'pane_id': 'viewer', 'tab_id': tab,
                                           'workspace_id': 'w1', 'terminal_id': 'terminal-viewer', 'tokens': {}}
                        layouts[tab].replace(target, Layout.split(target, 'viewer', params['ratio']))
                        result = {'pane': {'pane_id': 'viewer'}}
                    elif method == 'pane.move':
                        name, dest = params['pane_id'], params['destination']
                        layouts[panes[name]['tab_id']].replace(name, None)
                        panes[name]['tab_id'] = dest['tab_id']
                        target = dest['target_pane_id']
                        layouts[dest['tab_id']].replace(target, Layout.split(
                            target, name, dest['ratio'], dest['split']))
                    elif method == 'pane.swap':
                        for layout in layouts.values():
                            layout.replace(params['source_pane_id'], '__swap__')
                            layout.replace(params['target_pane_id'], params['source_pane_id'])
                            layout.replace('__swap__', params['target_pane_id'])
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
            raise AssertionError('Preview routing did not complete: ' + str([r['method'] for r in requests])
                                 + '; output=' + repr(bytes(output[:7000])))

        try:
            if history:
                wait_for(lambda: b'File History' in output)
                # Select the changed file without opening its diff.
                os.write(master, b'gj')
                for title, y, file in [('Graph', 10, ''), ('Commits', 11, ''),
                                       ('File History', 12, 'example-a.txt')]:
                    click = f'\x1b[<0;5;{y}M\x1b[<0;5;{y}m'.encode()
                    os.write(master, click)
                    payload = f'history\t{root}\t{title}\t{file}'
                    wait_for(lambda: any(path.read_text() == payload
                                        for path in (profile / 'sidebar/scratch').glob('*.ctl')))
                # Enter uses the same Preview action; rows never expanded above it.
                os.write(master, b'\r')
                deadline = time.monotonic() + .3
                while time.monotonic() < deadline:
                    drain()
                assert not any('git show' in r.get('params', {}).get('text', '') for r in requests)
            else:
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
            current = layouts['w1:t1'].snapshot()
            if placement == 'replace' and not blocked:
                assert panes['working']['tab_id'] != 'w1:t1', 'Replace left the working terminal visible behind the preview'
                assert len(splits) == 1, ('viewer not reused', splits)
                assert splits[0]['params']['target_pane_id'] == 'owner'
                viewer_rect = next(p['rect'] for p in current['panes'] if p['pane_id'] == 'viewer')
                assert viewer_rect == {'x': 28, 'y': 0, 'width': 64, 'height': 40}, viewer_rect
                assert {p['pane_id'] for p in current['panes']} == {'owner', 'viewer', 'original'}
            elif placement == 'tab' or blocked:
                assert panes['viewer']['tab_id'] != 'w1:t1', 'Tab preview silently became a split'
                assert not splits and current == before
                assert len([r for r in requests if r['method'] == 'tab.create']) == 1, 'Second click opened a duplicate preview tab'
            else:
                assert len(splits) == 1 and panes['working']['tab_id'] == 'w1:t1'
                assert not any(r['method'] in ['tab.create', 'pane.move'] for r in requests)
            assert next(p['rect'] for p in current['panes'] if p['pane_id'] == 'original') == original_rect
            mutations = [r for r in requests if r['method'] not in ['pane.list', 'pane.layout', 'pane.process_info', 'workspace.list', 'plugin.list']]
            assert not any(r['params'].get(key) == 'original' for r in mutations
                           for key in ['pane_id', 'source_pane_id', 'target_pane_id'])
            assert panes['original']['tokens'] == original_tokens
            print(f'{placement} (blocked={blocked}, history={history}): mouse clicks honor preview placement and reuse; independent plugin geometry untouched')
        finally:
            os.close(master)
            stopped.set()
            worker.join(timeout=5)
            server.close()
            if process.poll() is None:
                process.kill()
            process.wait(timeout=15)


if __name__ == '__main__':
    for mode in ['replace', 'tab', 'pane']:
        check(str(Path(sys.argv[1]).resolve()), mode)
    check(str(Path(sys.argv[1]).resolve()), 'replace', blocked=True)
