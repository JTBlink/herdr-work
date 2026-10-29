"""Verify full-height product docking in a private Herdr server, never the user session."""
import argparse
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import shlex
import socket
import struct
import subprocess
import tempfile
import termios
import threading
import time


def check(binary, output, host="herdr", exact_rail=False):
    base = Path(tempfile.mkdtemp(prefix='hw-docking-', dir='/tmp'))
    output.mkdir(parents=True, exist_ok=True)
    env = {k: v for k, v in os.environ.items() if not k.startswith('HERDR_')}
    env.update(XDG_CONFIG_HOME=str(base / 'config'), XDG_DATA_HOME=str(base / 'data'),
               XDG_STATE_HOME=str(base / 'state'), XDG_CACHE_HOME=str(base / 'cache'),
               HERDR_CONFIG_PATH=str(base / 'config/herdr/config.toml'))
    if host != 'herdr':
        env['HERDR_BIN_PATH'] = str(Path(host).resolve())
    config = Path(env['HERDR_CONFIG_PATH'])
    config.parent.mkdir(parents=True)
    config.write_text('')
    plugin = base / 'plugin'
    plugin.mkdir()
    (plugin / 'herdr-plugin.toml').write_text(
        'id="jtblink.herdr-work"\nname="HerdrWork"\nversion="0.1.6"\nmin_herdr_version="0.9.3"\n'
        '[[panes]]\nid="pane"\ntitle="HerdrWork"\nplacement="split"\ncommand='
        + json.dumps([str(binary), 'ui', '--sidebar']) + '\n')
    worker = base / 'worker.py'
    worker.write_text('import os,time\nprint("WORKER",os.getpid(),flush=True)\nwhile True: time.sleep(1)\n')
    log = (output / 'server.log').open('w')
    server = subprocess.Popen([str(host), '--session', 'dock-test', 'server'], env=env,
                              stdout=log, stderr=subprocess.STDOUT)
    client = reader = None
    stopping = threading.Event()
    master = None
    try:
        deadline = time.monotonic() + 15
        address = None
        while time.monotonic() < deadline:
            for candidate in base.rglob('herdr.sock'):
                try:
                    with socket.socket(socket.AF_UNIX) as stream:
                        stream.connect(str(candidate))
                    address = candidate
                    break
                except OSError:
                    pass
            if address:
                break
            assert server.poll() is None, 'Private Herdr server exited'
            time.sleep(.1)
        assert address, 'Private socket missing'
        assert str(address) != os.environ.get('HERDR_SOCKET_PATH')

        def rpc(method, params=None):
            with socket.socket(socket.AF_UNIX) as stream:
                stream.settimeout(15)
                stream.connect(str(address))
                stream.sendall((json.dumps({'id': 'dock-fixture', 'method': method,
                                            'params': params or {}}) + '\n').encode())
                answer = json.loads(stream.makefile().readline())
                assert 'error' not in answer, (method, answer)
                return answer['result']

        linked = subprocess.run([str(host), '--session', 'dock-test', 'plugin', 'link', str(plugin)],
                                env=env, capture_output=True, text=True, timeout=30)
        assert linked.returncode == 0, linked.stderr
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 48, 160, 0, 0))

        def seat():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        client = subprocess.Popen([str(host), '--session', 'dock-test'], env=dict(env, TERM='xterm-256color'),
                                  stdin=slave, stdout=slave, stderr=slave, preexec_fn=seat)
        os.close(slave)

        def drain():
            while not stopping.is_set():
                if select.select([master], [], [], .1)[0]:
                    try:
                        os.read(master, 65536)
                    except OSError:
                        break

        reader = threading.Thread(target=drain)
        reader.start()
        time.sleep(.5)

        def tree(node, omit=None):
            if node['type'] == 'pane':
                return None if node['pane_id'] == omit else node['pane_id']
            first, second = tree(node['first'], omit), tree(node['second'], omit)
            if first is None:
                return second
            if second is None:
                return first
            return [node['direction'], round(node['ratio'], 5), first, second]

        cases = [(r, n, False, False) for r in [False, True] for n in [False, True]]
        cases += [(r, False, True, q) for r in [False, True] for q in [False, True]]
        for right, nested, existing, quiet in cases:
            name = ('right' if right else 'left') + ('-nested' if nested else '-stacked')
            name += '-existing' if existing else ''
            name += '-ensure' if quiet else ''
            profile = base / name
            (profile / 'sidebar').mkdir(parents=True)
            (profile / 'sidebar/state.json').write_text(json.dumps({
                'auto_open': True, 'dock_right': right, 'sidebar_width': 28, 'font_prompt': True}))
            created = rpc('workspace.create', {'cwd': str(base), 'focus': False, 'label': name})
            workspace, tab = created['workspace']['workspace_id'], created['tab']['tab_id']
            top = created['root_pane']['pane_id']
            bottom = rpc('pane.split', {'target_pane_id': top, 'direction': 'down',
                                       'ratio': .62, 'focus': False, 'cwd': str(base)})['pane']['pane_id']
            work = [top, bottom]
            if nested:
                work.append(rpc('pane.split', {'target_pane_id': top, 'direction': 'right',
                                              'ratio': .7, 'focus': False, 'cwd': str(base)})['pane']['pane_id'])
            for pane in work:
                rpc('pane.send_input', {'pane_id': pane, 'text': 'python3 -u ' + shlex.quote(str(worker)),
                                        'keys': ['Enter']})
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                running = [rpc('pane.process_info', {'pane_id': p})['process_info'] for p in work]
                if all(any(str(worker) in proc.get('argv', []) for proc in info.get('foreground_processes', []))
                       for info in running):
                    break
                time.sleep(.1)
            else:
                raise AssertionError((name, 'Working processes never started', running))
            before = rpc('pane.layout', {'pane_id': top})['layout']
            original = tree(rpc('layout.export', {'tab_id': tab})['layout']['root'])
            identities = {p: rpc('pane.process_info', {'pane_id': p})['process_info']['foreground_process_group_id']
                          for p in work}
            rpc('pane.focus', {'pane_id': bottom})
            call_env = dict(env, HERDR_ENV='1', HERDR_SOCKET_PATH=str(address), HERDR_PANE_ID=bottom,
                            HERDR_TAB_ID=tab, HERDR_WORKSPACE_ID=workspace,
                            HERDR_PLUGIN_CONTEXT_JSON=json.dumps({'tab_id': tab, 'focused_pane_id': bottom}))
            if existing:
                rpc('plugin.pane.open', {'plugin_id': 'jtblink.herdr-work', 'entrypoint': 'pane',
                          'placement': 'split', 'target_pane_id': bottom, 'direction': 'right', 'focus': False,
                          'cwd': str(base), 'env': {'HERDR_WORK_HOME': str(profile), 'HERDR_WORK_ROOT': str(base)}})
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    if any(p.get('tokens', {}).get('herdr-work-ready') == 'true' for p in
                           rpc('pane.list')['panes'] if p['tab_id'] == tab):
                        break
                    time.sleep(.1)
                else:
                    raise AssertionError('Old sidebar never became ready')
                rpc('pane.focus', {'pane_id': bottom})
            command = ['sidebar-ensure'] if quiet else ['activate', 'show-explorer']
            opened = subprocess.run([str(binary), '--home', str(profile), *command],
                                    env=call_env, capture_output=True, text=True, timeout=30)
            assert opened.returncode == 0, opened.stderr
            result = json.loads(opened.stdout)
            assert not result.get('layout_warning'), result
            sidebar = result['pane_id']
            time.sleep(.4)
            after = rpc('pane.layout', {'pane_id': sidebar})['layout']
            (output / (name + '.json')).write_text(json.dumps({'before': before, 'after': after}, indent=2))
            rect = next(p['rect'] for p in after['panes'] if p['pane_id'] == sidebar)
            assert rect['y'] == after['area']['y'] and rect['height'] == after['area']['height'], (
                name, 'Sidebar must span the tab height', rect, after['area'])
            expected_edge = after['area']['x'] + (after['area']['width'] - rect['width'] if right else 0)
            assert rect['x'] == expected_edge, (name, 'Wrong docking edge')
            assert abs(rect['width'] - 28) <= 2, (name, 'Wrong sidebar width', rect)
            current = rpc('layout.export', {'tab_id': tab})['layout']['root']
            assert tree(current, sidebar) == original, (name, 'Working split tree changed')
            focused = next(p['pane_id'] for p in rpc('pane.list')['panes'] if p.get('focused'))
            assert focused == (bottom if quiet else sidebar), (name, 'Wrong final focus')
            for pane in work:
                assert identities[pane] == rpc('pane.process_info', {'pane_id': pane})['process_info']['foreground_process_group_id']
            # Exercise the actual component's mouse route and socket resize,
            # including the protection against preferred-width repair.
            def visible():
                return rpc('pane.read', {'pane_id': sidebar, 'source': 'visible'})['read']['text']

            def wait_for(predicate, message):
                deadline = time.monotonic() + 12
                while time.monotonic() < deadline:
                    if predicate():
                        return
                    time.sleep(.1)
                raise AssertionError((name, message, visible()))

            def mouse_at(symbol):
                lines = visible().splitlines()
                row, column = next((y, line.index(symbol)) for y, line in enumerate(lines) if symbol in line)
                rpc('pane.send_input', {'pane_id': sidebar, 'text': f'\x1b[<0;{column+1};{row+1}M'})

            wait_for(lambda: '«' in visible(), 'Collapse control missing')
            mouse_at('«')
            wait_for(lambda: '»' in visible(), 'Rail missing')
            collapsed = rpc('pane.layout', {'pane_id': sidebar})['layout']
            narrow = next(p['rect'] for p in collapsed['panes'] if p['pane_id'] == sidebar)
            if exact_rail:
                assert narrow['width'] <= 5, (name, 'Rail must be exactly three content columns plus pane chrome', narrow)
                wait_for(lambda: max(map(len, visible().splitlines()), default=0) <= 3, 'Rail content must fit three columns after PTY resize')
            assert narrow['width'] <= max(10, after['area']['width'] // 10) and narrow['width'] < rect['width'], (name, 'Collapsed sidebar must be one icon column', narrow)
            if exact_rail:
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH', 48, 200, 0, 0))
                wait_for(lambda: rpc('pane.layout', {'pane_id': sidebar})['layout']['area']['width'] > after['area']['width'], 'Outer terminal did not grow')
                wait_for(lambda: next(p['rect']['width'] for p in rpc('pane.layout', {'pane_id': sidebar})['layout']['panes'] if p['pane_id'] == sidebar) <= 5,
                         'Rail width changed with the outer terminal')
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH', 48, 160, 0, 0))
                wait_for(lambda: rpc('pane.layout', {'pane_id': sidebar})['layout']['area']['width'] == after['area']['width'], 'Outer terminal did not return')
                wait_for(lambda: next(p['rect']['width'] for p in rpc('pane.layout', {'pane_id': sidebar})['layout']['panes'] if p['pane_id'] == sidebar) <= 5, 'Rail width was not restored')
            time.sleep(2.2)  # Includes a product layout-recovery timer tick.
            assert '»' in visible(), (name, 'Recovery unexpectedly expanded the sidebar')
            mouse_at('»')
            wait_for(lambda: '«' in visible(), 'Expansion failed')
            expanded = rpc('pane.layout', {'pane_id': sidebar})['layout']
            restored_rect = next(p['rect'] for p in expanded['panes'] if p['pane_id'] == sidebar)
            assert abs(restored_rect['width'] - rect['width']) <= 2, (name, 'Expanded width changed', rect, restored_rect)
            mouse_at('×')
            wait_for(lambda: not any(p['pane_id'] == sidebar for p in rpc('pane.list')['panes']), 'Close control failed')
            assert (profile / 'sidebar').exists()
            restored = rpc('layout.export', {'tab_id': tab})['layout']['root']
            assert tree(restored) == original, (name, 'Closing sidebar changed the original tree')
            rpc('workspace.close', {'workspace_id': workspace})
            print(name + ': full-height edge column, original working tree and processes preserved', flush=True)
    finally:
        if client:
            client.terminate()
            client.wait(timeout=10)
        stopping.set()
        if reader:
            reader.join(timeout=2)
        if master is not None:
            os.close(master)
        server.terminate()
        server.wait(timeout=15)
        log.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--host', type=Path, help='Use an isolated test host binary')
    parser.add_argument('--exact-rail', action='store_true', help='Require the host to honor the three-column rail')
    args = parser.parse_args()
    check(args.binary.resolve(strict=True), args.output.resolve(), args.host.resolve(strict=True) if args.host else 'herdr', args.exact_rail)
