"""Exercise product-only settings and VS Code file opening in a private PTY."""
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


def check(binary, fail=False):
    with tempfile.TemporaryDirectory(prefix='hw-vscode-settings-') as directory:
        base = Path(directory).resolve()
        root, profile, foreign, tools = [base / name for name in ('project', 'profile', 'foreign-sidebar', 'tools')]
        for path in (root, profile / 'sidebar', foreign, tools):
            path.mkdir(parents=True)
        file = root / 'a $(touch injected) 中文 file.txt'
        file.write_text('first line\nneedle fixture\n')
        (root / 'folder').mkdir()
        subprocess.run(['git', 'init', '-q', str(root)], check=True)
        original = b'{"auto_open":true,"strict_toggle":true,"focus_on_open":false,"follow_cwd":false,"dock_right":true,"sidebar_width":76,"preview_placement":"tab"}'
        (foreign / 'state.json').write_bytes(original)
        (foreign / 'editor-command.txt').write_text('foreign-editor {file}')
        saved = profile / 'sidebar/state.json'
        saved.write_text(json.dumps({'font_prompt': True, 'auto_open': False, 'merged': True,
                                     'preview_placement': 'replace', 'icons': 'emoji',
                                     'git_deco': False, 'git_footer': False, 'sidebar_width': 28}))
        capture = base / 'opened.json'
        captured_env = base / 'opened-env.json'
        launcher = tools / 'code'
        launcher.write_text('#!' + sys.executable + '\nimport json,os,sys\n'
                            'with open(os.environ["HW_VSCODE_ENV_CAPTURE"],"w") as f: json.dump({"herdr":{k:v for k,v in os.environ.items() if k.startswith("HERDR_")},"keep":os.environ.get("HW_VSCODE_KEEP")},f)\n'
                            'with open(os.environ["HW_VSCODE_CAPTURE"],"w") as f: json.dump(sys.argv[1:],f)\n'
                            'sys.exit(int(os.environ.get("HW_VSCODE_EXIT","0")))\n')
        launcher.chmod(0o755)
        env = {k: v for k, v in os.environ.items()
               if not k.startswith('HERDR_') and k not in ('VISUAL', 'EDITOR')}
        env.update(TERM='xterm-256color', HERDR_PLUGIN_STATE_DIR=str(foreign),
                   PATH=str(tools) + os.pathsep + os.environ['PATH'], HW_VSCODE_CAPTURE=str(capture),
                   HERDR_TEST_EXTRA='inherited-pane', HERDR_WORK_TEST_CONTEXT='inherited-profile',
                   HW_VSCODE_ENV_CAPTURE=str(captured_env), HW_VSCODE_KEEP='normal-environment',
                   HW_VSCODE_EXIT='1' if fail else '0')
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 42, 42, 0, 0))

        def seat():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        process = subprocess.Popen([binary, '--home', str(profile), 'ui', '--sidebar', str(root)],
                                   stdin=slave, stdout=slave, stderr=slave, env=env, preexec_fn=seat)
        os.close(slave)
        output = bytearray()

        def drain(seconds=.15):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                if select.select([master], [], [], .02)[0]:
                    try:
                        output.extend(os.read(master, 65536))
                    except OSError:
                        return

        def wait_for(predicate, label):
            deadline = time.monotonic() + 8
            while time.monotonic() < deadline:
                drain(.05)
                if predicate():
                    if capture.exists():
                        assert json.loads(captured_env.read_text()) == {
                            'herdr': {}, 'keep': 'normal-environment'
                        }, 'VS Code inherited Herdr environment or lost unrelated environment'
                    return
            raise AssertionError(label + '; last output: ' + text()[-1800:])

        def send(text):
            os.write(master, text)
            drain()

        def text():
            plain = re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b'', bytes(output)).decode(errors='replace')
            return ''.join(plain.split())

        def notice_expires(message):
            wait_for(lambda: message in text(), 'Launch result was not displayed')
            drain(3.3)  # No input: the footer must expire while the view is idle.
            output.clear()
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH', 43, 42, 0, 0))
            drain(.3)
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH', 42, 42, 0, 0))
            drain(.3)
            assert text(), 'PTY did not redraw after the idle deadline'
            assert message not in text(), 'Launch result stayed visible after its deadline'

        def setting(index):
            send(b's')
            send(b'\x1b[B' * index + b'\r')
            if index != 0:  # The project action closes Settings itself.
                send(b'\x1b')

        try:
            wait_for(lambda: b'file.txt' in output, 'Explorer did not load')
            send(b's')
            assert '仅HerdrWork' in text() and 'foreign-editor' not in text(), text()[-1500:]
            send(b'\x1b')
            # VS Code action first, then scope, dock, width, icons, colors, file-opening mode.
            setting(6)
            wait_for(lambda: json.loads(saved.read_text())['preview_placement'] == 'vscode', 'VS Code mode was not persisted')
            # Direct settings action opens the current project in both views.
            for view in (b'1', b'3'):
                send(view)
                output.clear()
                setting(0)
                wait_for(capture.exists, 'Settings action did not open the project in VS Code')
                assert json.loads(capture.read_text()) == ['--reuse-window', str(root)]
                capture.unlink()
                notice_expires('VSCode打开失败' if fail else '已通过VSCode打开')
            send(b'1')
            # Toggle HerdrWork auto-open only; inherited Sidebar directory stays byte-identical.
            setting(11)
            wait_for(lambda: json.loads(saved.read_text())['auto_open'] is True, 'Auto-open did not persist')
            for index, key in [(2, 'dock_right'), (10, 'hotkeys'), (12, 'strict_toggle'),
                               (13, 'focus_on_open'), (14, 'follow_cwd')]:
                previous = json.loads(saved.read_text())[key]
                setting(index)
                wait_for(lambda: json.loads(saved.read_text())[key] is not previous, key + ' did not persist')
            assert (foreign / 'state.json').read_bytes() == original
            assert (foreign / 'editor-command.txt').read_text() == 'foreign-editor {file}'
            # The folder sorts first. Select the file without invoking a host preview.
            send(b'g\x1b[B\r')
            wait_for(capture.exists, 'File did not reach VS Code')
            assert json.loads(capture.read_text()) == ['--reuse-window', str(file)], capture.read_text()
            assert not (root / 'injected').exists()
            if fail:
                wait_for(lambda: 'VSCode启动失败' in text(), 'Launcher failure was not shown')
                send(b'q')
                wait_for(lambda: process.poll() is not None, 'UI did not exit after launcher failure')
                assert process.returncode == 0
                print('VS Code failure: visible notice; file and independent Sidebar settings preserved')
                return
            capture.unlink()
            # Full-text search preserves the hit line in --goto.
            send(b'\x06needle')
            wait_for(lambda: b'needle fixture' in output or b'2:' in output, 'Search did not return a result')
            send(b'\t\t\r')  # Search -> replacement -> results -> open.
            wait_for(capture.exists, 'Search hit did not reach VS Code')
            assert json.loads(capture.read_text()) == ['--reuse-window', '--goto', str(file) + ':2:1'], capture.read_text()
            assert (foreign / 'state.json').read_bytes() == original
            send(b'\x1b')
            capture.unlink()
            send(b'gm\r')
            wait_for(capture.exists, 'Directory context menu did not open VS Code')
            assert json.loads(capture.read_text()) == ['--reuse-window', str(root / 'folder')]
            send(b'q')
            wait_for(lambda: process.poll() is not None, 'UI did not exit')
            assert process.returncode == 0
            print('VS Code: file path and search line preserved; HerdrWork settings isolated from inherited Sidebar configuration')
        finally:
            os.close(master)
            if process.poll() is None:
                process.kill()
            process.wait(timeout=5)


if __name__ == '__main__':
    binary = str(Path(sys.argv[1]).resolve(strict=True))
    check(binary)
    check(binary, fail=True)
