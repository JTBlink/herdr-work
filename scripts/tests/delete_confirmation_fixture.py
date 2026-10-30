"""Exercise delete confirmation with real keyboard/mouse input in a private PTY."""
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
import unicodedata


# Read the cursor-addressed redraw into cells, so clicks use observed labels.
TOKENS = re.compile(r'\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b\[([0-?]*)([ -/]*)([@-~])|\x1b.|[^\x1b]')


def cells(data, width, height):
    grid = [[' '] * width for _ in range(height)]
    x = y = 0
    for token in TOKENS.finditer(data.decode(errors='replace')):
        text = token.group()
        params, _, command = token.groups()
        if params is not None:
            args = [int(n) if n else 1 for n in params.split(';')] if params.replace(';', '').isdigit() else []
            n = args[0] if args else 1
            if command in ('H', 'f'):
                y, x = (args + [1, 1])[:2]
                y -= 1
                x -= 1
            elif command == 'A': y = max(0, y - n)
            elif command == 'B': y += n
            elif command == 'C': x += n
            elif command == 'D': x = max(0, x - n)
            elif command == 'G': x = n - 1
            elif command == 'd': y = n - 1
            elif command == 'J' and n == 2: grid = [[' '] * width for _ in range(height)]
            continue
        if text.startswith('\x1b'): continue
        if text == '\r': x = 0
        elif text == '\n': y += 1
        elif text == '\b': x = max(0, x - 1)
        elif text >= ' ':
            if x >= width:
                x = 0
                y += 1
            if 0 <= y < height and 0 <= x < width:
                grid[y][x] = text
            x += 2 if unicodedata.east_asian_width(text) in ('W', 'F') else 1
    return grid


def locate(grid, label):
    for y, row in enumerate(grid):
        for x in range(len(row)):
            pos = x
            for ch in label:
                if pos >= len(row) or row[pos] != ch: break
                pos += 2 if unicodedata.east_asian_width(ch) in ('W', 'F') else 1
            else:
                return x, y
    return None


def check(binary, theme, directory, width):
    with tempfile.TemporaryDirectory(prefix='hw-delete-confirm-') as name:
        base = Path(name).resolve()
        root, profile = base / 'demo-project', base / 'profile'
        root.mkdir()
        (profile / 'sidebar').mkdir(parents=True)
        (profile / 'sidebar/state.json').write_text(json.dumps({
            'font_prompt': True, 'merged': True, 'icons': 'emoji', 'colors': theme,
            'git_deco': False, 'git_footer': False, 'follow_cwd': False,
            'sidebar_width': width,
        }))
        target = root / ('a-folder' if directory else 'a-file.txt')
        if directory:
            target.mkdir()
            (target / 'nested.txt').write_text('fixture')
        else:
            target.write_text('fixture')
        neighbor = root / 'z-keep.txt'
        neighbor.write_text('keep')
        master, slave = pty.openpty()
        height = 24
        current_width = width

        def resize(columns):
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH', height, columns, 0, 0))
        resize(current_width)
        env = {k: v for k, v in os.environ.items() if not k.startswith('HERDR_')}
        env['TERM'] = 'xterm-256color'

        def seat():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)
        process = subprocess.Popen([str(binary), '--home', str(profile), 'ui', '--sidebar', str(root)],
                                   stdin=slave, stdout=slave, stderr=slave, env=env, preexec_fn=seat)
        os.close(slave)

        def drain(seconds=.15):
            data = b''
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                if select.select([master], [], [], .02)[0]:
                    try: data += os.read(master, 65536)
                    except OSError: break
            return data

        def send(data):
            os.write(master, data)
            drain()

        def frame(label):
            nonlocal current_width
            for _ in range(24):
                current_width = width + 1 if current_width == width else width
                resize(current_width)
                grid = cells(drain(.2), current_width, height)
                if locate(grid, label) is not None: return grid
            raise AssertionError(f'{theme}: did not render {label!r}; exit={process.poll()}; frame={grid!r}')

        def open_modal():
            # Locate the painted Delete item: file and folder menus have different entries.
            send(b'gm')
            menu = frame('删除…')
            click(locate(menu, '删除…'))
            return frame('确认删除')

        def click(at):
            x, y = at
            send(f'\x1b[<0;{x + 1};{y + 1}M'.encode())

        try:
            frame('z-keep.txt')
            grid = open_modal()
            click(locate(grid, '取消'))
            assert target.exists() and neighbor.read_text() == 'keep'
            grid = frame('z-keep.txt')
            assert locate(grid, '确认删除') is None
            open_modal()
            send(b'\r')
            grid = frame('z-keep.txt')
            assert target.exists() and locate(grid, '确认删除') is None
            open_modal()
            send(b'\x10')  # Ctrl+P must remain trapped by the delete modal.
            grid = frame('确认删除')
            click(locate(grid, '×'))
            assert target.exists()
            frame('z-keep.txt')
            grid = open_modal()
            click(locate(grid, '确认删除'))
            deadline = time.monotonic() + 3
            while target.exists() and time.monotonic() < deadline:
                drain(.05)
            assert not target.exists(), f'{theme}: confirmation did not delete captured target'
            assert neighbor.read_text() == 'keep', 'deleted a different tree row'
            print(f'delete modal: {theme}, directory={directory}, mouse cancel/close/confirm, safe Enter, shortcut capture passed')
        finally:
            process.terminate()
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            os.close(master)


if __name__ == '__main__':
    executable = Path(sys.argv[1]).resolve()
    for case in [('vscode', True, 28), ('light', False, 28), ('terminal', False, 40)]:
        check(executable, *case)
