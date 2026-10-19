"""Exercise loaded UI data and task creation in a private Unix PTY."""
import argparse
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time


def ui_smoke(binary):
    if os.name == 'nt':
        return  # Native rendering/state regressions run in Rust tests on Windows.
    import fcntl
    import pty
    import select
    import signal
    import struct
    import termios

    binary = Path(binary).resolve()
    with tempfile.TemporaryDirectory(prefix='herdr-work-ui-smoke-') as name:
        root = Path(name).resolve()
        repo = root / 'repo'
        repo.mkdir()
        def git(*args):
            return subprocess.run(['git', '-C', str(repo), *args], check=True, capture_output=True, text=True).stdout
        git('init', '-q')
        git('config', 'user.name', 'UI Fixture')
        git('config', 'user.email', 'fixture@example.invalid')
        file = repo / 'visible.txt'
        file.write_text('original fixture\n')
        git('add', '.')
        git('commit', '-qm', 'fixture')
        file.write_text('changed preview fixture\n')
        master, slave = pty.openpty()
        def size(rows, columns):
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', rows, columns, 0, 0))
        size(36, 120)
        original = termios.tcgetattr(slave)
        env = {k: v for k, v in os.environ.items() if not k.startswith(('HERDR_',))}
        env.update(TERM='xterm-256color', HERDR_PROJECTS_NO_KEEPER='1', HERDR_PROJECTS_INLINE='1')
        def seat():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)
        # Keep the controlling tty alive briefly after the UI exits so restoration
        # can be checked without a session hangup racing the assertion.
        wrapper = 'import subprocess,sys,time; r=subprocess.run(sys.argv[1:]); time.sleep(1); sys.exit(r.returncode)'
        process = subprocess.Popen([sys.executable, '-c', wrapper, str(binary), '--home',
                                    str(root / 'profile'), 'ui', str(repo)],
                                   stdin=slave, stdout=slave, stderr=slave, env=env, preexec_fn=seat)
        def drain(seconds):
            frame = b''
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                if select.select([master], [], [], .05)[0]:
                    try:
                        frame += os.read(master, 65536)
                    except OSError:
                        break
            text = re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b'', frame).decode(errors='replace')
            return ''.join(text.split())
        redraws = 0
        def screen(key=None):
            nonlocal redraws
            if key:
                os.write(master, key)
            drain(.2)
            redraws += 1
            size(36, 120 + redraws % 2)
            os.killpg(process.pid, signal.SIGWINCH)
            return drain(.25)
        def wait_for(expected, key=None):
            text = screen(key)
            deadline = time.monotonic() + 8
            while expected not in text and time.monotonic() < deadline:
                text = screen()
            if expected not in text:
                raise AssertionError(f'UI did not render {expected!r}')
            return text
        try:
            wait_for('Git修改：1')
            text = wait_for('任务：0')
            assert '加载中' not in text and 'Loading' not in text, 'Startup remained pending'
            assert 'EXPLORER' in text and 'visible.txt' in text and 'Preview' in text, 'Default Explorer is missing'
            assert '暂无任务' not in text and text.count('Tasks') == 0, 'Tasks displaced the default sidebar'
            wait_for('changedpreviewfixture', b'\r')
            wait_for('侧栏设置', b',')
            wait_for('EXPLORER', b'\x1b')
            wait_for('文件菜单', b'm')
            wait_for('EXPLORER', b'\x1b')
            wait_for('编辑·visible.txt', b'e')
            screen(b'\x01')  # Original editor: select all.
            screen(b'\x1b[200~changed preview fixture\neditor saved UTF-8\n\x1b[201~')
            wait_for('文件已保存', b'\x13')
            assert file.read_text() == 'changed preview fixture\neditor saved UTF-8\n', 'Built-in editor did not save the actual UTF-8 file'
            wait_for('Preview', b'\x1b')
            # Click the Git activity using the terminal's SGR mouse protocol.
            text = wait_for('SOURCECONTROL', b'\x1b[<0;4;9M\x1b[<0;4;9m')
            assert 'Changes(1)' in text, 'Git changes were not shown in the sidebar'
            wait_for('Staged(1)', b's')
            assert git('diff', '--cached', '--name-only').strip() == 'visible.txt', 'UI stage did not update the actual index'
            wait_for('Changes(1)', b'u')
            assert not git('diff', '--cached', '--name-only').strip(), 'UI unstage did not restore the actual index'
            wait_for('changedpreviewfixture', b'\r')
            wait_for('Staged(1)', b's')
            assert git('diff', '--cached', '--name-only').strip() == 'visible.txt', 'Diff preview changed the SCM stage action'
            wait_for('Changes(1)', b'u')
            assert not git('diff', '--cached', '--name-only').strip()

            wait_for('Newtask', b'1n')
            wait_for('任务：1', b'UI smoke task\r')
            assert 'UIsmoketask' in screen(), 'Created task not visible'
            wait_for('EXPLORER', b'2')
            size(24, 80)
            os.killpg(process.pid, signal.SIGWINCH)
            text = drain(.3)
            assert 'EXPLORER' in text and 'Preview' in text, 'Narrow split view is missing'
            os.write(master, b'q')
            drain(.4)
            restored = termios.tcgetattr(slave)
            flags = termios.ECHO | termios.ICANON
            assert restored[3] & flags == original[3] & flags, 'Terminal flags not restored'
            assert process.wait(timeout=5) == 0, 'UI failed to exit'
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=5)
            os.close(master)
            os.close(slave)
    print('UI loaded: default Explorer, settings/menu, original editor save, mouse navigation, preview, stage/unstage, real Diff, task creation, 80-column split and clean exit')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    ui_smoke(parser.parse_args().binary)
