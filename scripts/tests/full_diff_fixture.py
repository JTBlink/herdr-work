"""Verify the full diff and top folding controls in a private terminal."""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time
from delete_confirmation_fixture import cells, locate


class Viewer:
    def __init__(self, command, width=120):
        self.master, slave = pty.openpty()
        self.width = width
        self.base_width = width
        self.height = 24
        self.resize(width)
        env = {k: v for k, v in os.environ.items() if not k.startswith('HERDR_')}
        env['TERM'] = 'xterm-256color'
        def seat():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)
        self.process = subprocess.Popen(command, stdin=slave, stdout=slave, stderr=slave,
                                        env=env, preexec_fn=seat)
        os.close(slave)

    def resize(self, width):
        self.width = width
        fcntl.ioctl(self.master, termios.TIOCSWINSZ, struct.pack('HHHH', self.height, width, 0, 0))

    def drain(self, seconds=.15):
        data = bytearray()
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            if select.select([self.master], [], [], .02)[0]:
                try: data.extend(os.read(self.master, 65536))
                except OSError: break
        return bytes(data)

    def send(self, data):
        os.write(self.master, data)
        self.drain()

    def click(self, x, y):
        self.send(f'\x1b[<0;{x+1};{y+1}M'.encode())

    def frame(self, label):
        for _ in range(24):
            self.resize(self.base_width + int(self.width == self.base_width))
            grid = cells(self.drain(.2), self.width, self.height)
            if locate(grid, label) is not None: return grid
        raise AssertionError(f'did not render {label!r}, exit={self.process.poll()}, tail={grid[-4:]}')

    def close(self):
        self.process.terminate()
        try: self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
        os.close(self.master)


def check(binary, theme):
    with tempfile.TemporaryDirectory(prefix='hw-full-diff-') as name:
        base = Path(name).resolve()
        root = base / 'demo'
        root.mkdir()
        profile = base / 'profile'
        scratch = profile / 'sidebar/scratch'
        scratch.mkdir(parents=True, mode=0o700)
        (profile / 'sidebar/state.json').write_text(json.dumps({
            'font_prompt': True, 'merged': True, 'icons': 'emoji', 'colors': theme,
            'git_deco': False, 'git_footer': False, 'follow_cwd': False,
            'sidebar_width': 40,
        }))
        original = ''.join(f'let unchanged_{n} = {n};\n' for n in range(1, 241))
        target = root / 'example.rs'
        target.write_text(original)
        for args in [['init', '-q'], ['add', '.'],
                     ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                      '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=.git/no-hooks',
                      'commit', '-qm', 'fixture']]:
            subprocess.run(['git', *args], cwd=root, check=True, capture_output=True)
        target.write_text(original.replace('unchanged_30 = 30', 'changed_30 = 300')
                         .replace('unchanged_190 = 190', 'changed_190 = 1900'))
        control = scratch / 'full-diff.ctl'
        control.write_text(f'diff\t{root}\texample.rs\tworktree')
        control.chmod(0o600)
        command = [str(binary), '--home', str(profile)]
        viewer = Viewer(command + ['sidebar-viewer', str(control)])
        try:
            grid = viewer.frame('unchanged_1 = 1;')
            assert locate(grid, '原始') and locate(grid, '修改后')
            assert sum('unchanged_1 = 1;' in ''.join(row) for row in grid) == 1
            first = ''.join(grid[locate(grid, 'unchanged_1 = 1;')[1]])
            assert first.count('unchanged_1 = 1;') == 2, 'both sides must show unchanged prefix'
            viewer.send(b'\x1b[18~')  # F7
            grid = viewer.frame('changed_30 = 300;')
            assert locate(grid, 'unchanged_30 = 30;'), 'old and new side must align'
            viewer.send(b'\x1b[18~')
            viewer.frame('changed_190 = 1900;')
            viewer.send(b'\x1b[F')  # End
            grid = viewer.frame('unchanged_240 = 240;')
            assert locate(grid, 'unchanged_220'), 'unchanged suffix must stay reachable'
            markers = [(viewer.width - 1, y) for y, row in enumerate(grid)
                       if row[-1] in ('~', '+', '−')]
            assert markers, 'missing whole-document change indicators'
            viewer.click(*markers[0])
            viewer.frame('changed_30 = 300;')
            viewer.base_width = 60
            viewer.resize(60)
            viewer.send(b'\x1b[F')
            grid = viewer.frame('unchanged_240 = 240;')
            assert locate(grid, '原始') is None, 'narrow view uses complete inline text'
            assert viewer.process.poll() is None
            print(f'full diff PTY: {theme}, unchanged prefix/suffix, split alignment, F7, overview click and narrow resize passed')
        finally:
            viewer.close()
        sidebar = Viewer(command + ['ui', '--sidebar', str(root)], width=40)
        try:
            grid = sidebar.frame('example.rs')
            assert grid[0][sidebar.width-2] == '«', 'top close button must become fold'
            assert all('«' not in row for row in grid[1:]), 'remove the bottom fold button'
            sidebar.click(sidebar.width-2, 0)
            sidebar.base_width = 4
            sidebar.resize(4)
            grid = sidebar.frame('»')
            assert sidebar.process.poll() is None, 'fold must retain the running sidebar'
            assert grid[0][1] == '»', 'expand stays at the top'
            sidebar.click(1, 0)
            sidebar.base_width = 40
            sidebar.resize(40)
            sidebar.frame('example.rs')
            assert sidebar.process.poll() is None
            print(f'top fold PTY: {theme}, fold/expand at top, no footer fold, process and tree preserved')
        finally:
            sidebar.close()


if __name__ == '__main__':
    for color in ['vscode', 'light', 'terminal']:
        check(Path(sys.argv[1]).resolve(), color)
