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
        file.write_text('changed preview fixture\n' + ''.join(f'line-{i:03d}\n' for i in range(1, 121)))
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
                raise AssertionError(f'UI did not render {expected!r}; last frame: {text[-1600:]}')
            return text
        try:
            wait_for('Git修改：1')
            text = wait_for('任务：0')
            assert '加载中' not in text and 'Loading' not in text, 'Startup remained pending'
            assert 'EXPLORER' in text and 'visible.txt' in text and 'Preview' in text, 'Default Explorer is missing'
            assert '暂无任务' not in text and text.count('Tasks') == 0, 'Tasks displaced the default sidebar'
            wait_for('changedpreviewfixture', b'\r')
            # Close at the rendered header's right edge, then reopen the same
            # tree selection. The fixture alternates 120/121-column frames.
            def close_document():
                return f'\x1b[<0;{120 + redraws % 2 - 1};1M'.encode()
            wait_for('选择文件', close_document())
            wait_for('changedpreviewfixture', b'\r')
            # A worker must refresh the current document without a refresh key.
            file.write_text(file.read_text().replace('changed preview fixture', 'externally refreshed preview'))
            wait_for('externallyrefreshedpreview')
            wait_for('侧栏设置', b',')
            wait_for('EXPLORER', b'\x1b')
            wait_for('文件菜单', b'm')
            wait_for('EXPLORER', b'\x1b')
            wait_for('编辑·visible.txt', b'e')
            # The editor's viewport follows wheel input rather than snapping back
            # to its unchanged cursor on the next draw.
            wait_for('line-060', b'\x1b[<65;90;15M' * 12)
            screen(b'\x01')  # Original editor: select all.
            screen(b'\x1b[200~changed preview fixture\neditor saved UTF-8\n\x1b[201~')
            wait_for('未保存的文本', close_document())
            wait_for('编辑·visible.txt', b'\x1b')
            wait_for('文件已保存', b'\x13')
            assert file.read_text() == 'changed preview fixture\neditor saved UTF-8\n', 'Built-in editor did not save the actual UTF-8 file'
            # A clean buffer reloads, but a dirty buffer requires an explicit
            # conflict choice and never silently overwrites external changes.
            file.write_text('clean external reload UTF-8\n')
            wait_for('cleanexternalreloadUTF-8')
            screen(b'\x01\x1b[200~unsaved buffer UTF-8\n\x1b[201~')
            external = 'changed preview fixture\nexternal conflict UTF-8\n'
            file.write_text(external)
            wait_for('磁盘已改变，保留编辑内容')
            wait_for('保存冲突', b'\x13')
            wait_for('unsavedbufferUTF-8', b'\x1b')
            assert file.read_text() == external, 'Canceling the conflict overwrote the external file'
            wait_for('保存冲突', b'\x13')
            wait_for('已重新加载磁盘文件', b'\x1b[B\r')
            wait_for('visible.txt', b'\x1b')
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
            wait_for('UIsmoketask', b'UI smoke task\r')

            target = repo / 'search-target.txt'
            target.write_text(''.join('needle80\n' if i == 80 else f'line-{i:03d}\n'
                                      for i in range(1, 121)))
            # No Enter is sent until a live result is visible. The adjacent line
            # can only appear in the preview if opening consumes the hit's line.
            wait_for('search-target.txt:80', b'\x06needle80')
            wait_for('line-081', b'\r')
            wait_for('visible.txt:1', b'\x10visible')
            wait_for('changedpreviewfixture', b'\r')
            wait_for('EXPLORER', b'2')
            size(24, 80)
            os.killpg(process.pid, signal.SIGWINCH)
            text = drain(.3)
            assert 'EXPLORER' in text and 'visible.txt' in text, 'Narrow split view is missing'
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
    print('UI loaded: compact Explorer, header close and unsaved guard, passive preview refresh, settings/menu, editor wheel/save/reload/conflict, mouse navigation, stage/unstage, real Diff, tasks, live search at line 80, live Quick Open, 80-column split and clean exit')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    ui_smoke(parser.parse_args().binary)
