"""Click the real viewer's close button against an isolated host (macOS/Linux)."""
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


def document_token(value):
    token = 0xcbf29ce484222325
    for byte in str(value).encode():
        token = ((token ^ byte) * 0x100000001b3) & ((1 << 64) - 1)
    return f'{token:016x}'


def check(binary, neighbor=False, reject_close=False, history=False, edit=None):
    with tempfile.TemporaryDirectory(prefix='hw-viewer-close-') as directory:
        base = Path(directory).resolve()
        profile = base / 'profile'
        scratch = profile / 'sidebar/scratch'
        scratch.mkdir(parents=True, mode=0o700)
        control = scratch / 'p-fixture.ctl'
        document = base / 'example.md'
        document.write_text('viewer close fixture\n')
        control.write_text(f'file\t{document}')
        if history:
            for args in [['init', '-q'], ['add', 'example.md'],
                         ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                          '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=.git/no-hooks',
                          'commit', '-qm', 'history viewer fixture']]:
                subprocess.run(['git', *args], cwd=base, check=True, capture_output=True)
            control.write_text(f'history\t{base}\tCommits\t')
        control.chmod(0o600)
        tokens = {'herdr-work-code-preview': str(int(time.time())),
                  'herdr-work-preview-path': document_token(document),
                  'herdr-work-preview-control': control.name,
                  'herdr-work-preview-profile': document_token(profile.resolve()),
                  'herdr-work-preview-origin-tab': 'w1:t1',
                  'herdr-work-preview-dedicated': '1'}
        panes = {
            'w1:p1': {'pane_id': 'w1:p1', 'tab_id': 'w1:t1', 'workspace_id': 'w1'},
            'w1:p2': {'pane_id': 'w1:p2', 'tab_id': 'w1:t2', 'workspace_id': 'w1',
                       'tokens': tokens},
        }
        if neighbor:
            panes['w1:p3'] = {'pane_id': 'w1:p3', 'tab_id': 'w1:t2', 'workspace_id': 'w1',
                              'label': 'Explorer', 'tokens': {'herdr-sidebar-explorer': '1'}}
        requests, errors = [], []
        focus, remaining_rejections = 'w1:p2', int(reject_close)
        server = socket.socket(socket.AF_UNIX)
        address = str(base / 'host.sock')
        server.bind(address)
        server.listen()
        server.settimeout(.1)
        stopped = threading.Event()

        def serve():
            nonlocal focus, remaining_rejections
            while not stopped.is_set():
                try:
                    stream, _ = server.accept()
                except socket.timeout:
                    continue
                with stream:
                    stream.settimeout(2)
                    try:
                        request = json.loads(stream.makefile().readline())
                        requests.append(request)
                        method, params = request['method'], request.get('params', {})
                        result = {}
                        if method == 'pane.list':
                            result = {'panes': [dict(p, focused=p['pane_id'] == focus) for p in panes.values()]}
                        elif method == 'pane.report_metadata':
                            panes[params['pane_id']].setdefault('tokens', {}).update(params['tokens'])
                        elif method == 'pane.rename':
                            panes[params['pane_id']]['label'] = params['label']
                        elif method == 'pane.focus':
                            focus = params['pane_id']
                        elif method in ('pane.close', 'tab.close'):
                            if remaining_rejections:
                                remaining_rejections -= 1
                                stream.sendall(b'{"error":{"code":"busy","message":"close rejected"}}\n')
                                continue
                            if method == 'pane.close':
                                assert params['pane_id'] == 'w1:p2', 'Closed an unrelated pane'
                                panes.pop('w1:p2')
                            else:
                                assert params['tab_id'] == 'w1:t2' and not neighbor, 'Closed an unrelated tab or plugin'
                                panes.pop('w1:p2')
                        stream.sendall((json.dumps({'result': result}) + '\n').encode())
                    except Exception as error:
                        errors.append(error)
                        stream.sendall(b'{"error":{"message":"fixture assertion failed"}}\n')

        worker = threading.Thread(target=serve)
        worker.start()
        env = {k: v for k, v in os.environ.items() if not k.startswith('HERDR_')}
        env.update(TERM='xterm-256color', HERDR_ENV='1', HERDR_SOCKET_PATH=address,
                   HERDR_PANE_ID='w1:p2', HERDR_TAB_ID='w1:t2', HERDR_WORKSPACE_ID='w1')
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 90, 0, 0))

        def seat():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        process = subprocess.Popen([binary, '--home', str(profile), 'sidebar-viewer', str(control)],
                                   stdin=slave, stdout=slave, stderr=slave, env=env, preexec_fn=seat)
        os.close(slave)
        output = bytearray()

        def shows(text):
            # Ratatui may emit cursor jumps instead of spaces between spans.
            plain = re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b'', bytes(output))
            return re.sub(rb'\s+', b'', text) in re.sub(rb'\s+', b'', plain)

        def wait_for(predicate, message):
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                if select.select([master], [], [], .03)[0]:
                    try:
                        output.extend(os.read(master, 65536))
                    except OSError:
                        pass
                if predicate():
                    return
            raise AssertionError(message + ': ' + str([r['method'] for r in requests]) + '; output=' + repr(bytes(output[-800:])))

        try:
            if history:
                wait_for(lambda: shows(b'history viewer fixture'), 'History did not load')
                os.write(master, b'\r')
                wait_for(lambda: shows(b'+viewer close fixture'), 'Enter did not show the commit patch')
                assert control.read_text().startswith(f'show\t{base}\t')
                output.clear()
                os.write(master, b'\x7f')
                wait_for(lambda: control.read_text().startswith('history\t')
                         and shows(b'history viewer fixture'), 'Backspace did not return to history')
                output.clear()
                # Double-click the first entry in the history body.
                click = b'\x1b[<0;5;2M\x1b[<0;5;2m'
                os.write(master, click)
                time.sleep(.1)
                os.write(master, click)
                wait_for(lambda: shows(b'+viewer close fixture'), 'Double-click did not show the commit patch')
                # A sidebar request must replace the old navigation context.
                output.clear()
                control.write_text(f'history\t{base}\tFile History\texample.md')
                wait_for(lambda: shows(b'history viewer fixture'), 'File History did not replace Commits')
                os.write(master, b'\r')
                wait_for(lambda: control.read_text().startswith('show\t')
                         and control.read_text().endswith('\texample.md'), 'File History lost its file scope')
            else:
                wait_for(lambda: b'viewer close fixture' in output, 'Viewer did not load')
            if edit:
                output.clear()
                os.write(master, b'e')
                wait_for(lambda: shows(b'EDIT (experimental)'), 'Editor did not open')
                os.write(master, b'changed ')
                output.clear()
                os.write(master, b'\x1b[<0;1;1M\x1b[<0;1;1m')
                wait_for(lambda: shows('保存修改？'.encode()), 'Close did not open a confirmation menu')
                assert not any(r['method'] in ('pane.close', 'tab.close') for r in requests)
                assert document.read_text() == 'viewer close fixture\n'
                # The menu's top-right x cancels, retaining both the dirty editor and file.
                output.clear()
                os.write(master, b'\x1b[<0;88;1M\x1b[<0;88;1m')
                wait_for(lambda: shows(b'EDIT (experimental)'), 'Menu x did not return to editing')
                assert process.poll() is None and document.read_text() == 'viewer close fixture\n'
                output.clear()
                os.write(master, b'\x1b[<0;1;1M\x1b[<0;1;1m')
                wait_for(lambda: shows('保存修改？'.encode()), 'Close menu did not reopen')
                if edit == 'conflict':
                    document.write_text('external version\n')
                # Select the centered save/discard button with the mouse.
                row = 12 if edit == 'discard' else 11
                output.clear()
                os.write(master, f'\x1b[<0;46;{row}M\x1b[<0;46;{row}m'.encode())
                if edit == 'conflict':
                    wait_for(lambda: shows('文件已在磁盘上修改'.encode()), 'Save lost the external-change guard')
                    assert document.read_text() == 'external version\n'
                    os.write(master, b'o')
            else:
                os.write(master, b'\x1b[<0;1;1M\x1b[<0;1;1m')
            if reject_close:
                wait_for(lambda: remaining_rejections == 0, 'Close was not attempted')
                time.sleep(.2)
                assert process.poll() is None and control.exists(), 'Rejected close exited viewer or discarded its control'
                os.write(master, b'\x1b[<0;1;1M\x1b[<0;1;1m')
            wait_for(lambda: 'w1:p2' not in panes, 'Viewer did not close')
            assert focus == 'w1:p1', 'Close did not return to the original page'
            assert 'w1:p1' in panes and ('w1:p3' in panes) == neighbor
            assert not errors, errors
            wait_for(lambda: process.poll() is not None, 'Viewer process did not exit')
            assert process.returncode == 0
            if edit:
                saved = document.read_text()
                assert saved == 'viewer close fixture\n' if edit == 'discard' else 'changed ' in saved
            print(f'viewer close: original page restored; neighbor={neighbor}, retry={reject_close}, history={history}, edit={edit}')
        finally:
            os.close(master)
            if process.poll() is None:
                process.kill()
            process.wait(timeout=5)
            stopped.set()
            worker.join(timeout=2)
            server.close()


if __name__ == '__main__':
    binary = str(Path(sys.argv[1]).resolve(strict=True))
    for neighbor in (False, True):
        for retry in (False, True):
            check(binary, neighbor, retry)
    for edit in ('save', 'discard', 'conflict'):
        check(binary, edit=edit)
