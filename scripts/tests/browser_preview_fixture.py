"""Browser mode, one-shot menu/header actions and launch failures without opening a real browser."""
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
    with tempfile.TemporaryDirectory(prefix='hw-browser-preview-') as directory:
        base = Path(directory).resolve()
        root, profile, tools = [base / name for name in ('project', 'profile', 'tools')]
        for path in [root, profile / 'sidebar', tools]:
            path.mkdir(parents=True)
        page = root / 'a $(touch injected) 中文 #?.HTML'
        page.write_text('<!doctype html>\n<title>browser fixture</title>\n<p>needle fixture</p>\n')
        (root / 'folder.html').mkdir()
        (root / 'readme.txt').write_text('plain fixture\n')
        state = profile / 'sidebar/state.json'
        state.write_text(json.dumps({'font_prompt': True, 'auto_open': False, 'merged': True,
                                    'preview_placement': 'replace', 'icons': 'emoji',
                                    'git_deco': False, 'git_footer': False, 'sidebar_width': 46}))
        capture = base / 'browser.json'
        resolver = 'osascript' if sys.platform == 'darwin' else 'xdg-settings'
        launcher = 'open' if sys.platform == 'darwin' else 'gtk-launch'
        browser = '/fixture/Default Browser.app' if sys.platform == 'darwin' else 'fixture-browser.desktop'
        (tools / resolver).write_text('#!' + sys.executable + '\nprint(' + repr(browser) + ')\n')
        (tools / resolver).chmod(0o755)
        (tools / launcher).write_text('#!' + sys.executable + '\nimport json,os,sys\n'
                                     'with open(os.environ["HW_BROWSER_CAPTURE"],"w") as f: json.dump({"args":sys.argv[1:],"herdr":[k for k in os.environ if k.startswith("HERDR_")]},f)\n'
                                     'sys.exit(int(os.environ.get("HW_BROWSER_EXIT","0")))\n')
        (tools / launcher).chmod(0o755)
        env = {key: value for key, value in os.environ.items() if not key.startswith('HERDR_')}
        env.update(TERM='xterm-256color', PATH=str(tools)+os.pathsep+os.environ['PATH'],
                   HERDR_TEST_PANE_CONTEXT='must-not-leak', HW_BROWSER_CAPTURE=str(capture),
                   HW_BROWSER_EXIT='1' if fail else '0')
        children = []

        def start(args, width=100):
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 38, width, 0, 0))

            def seat():
                os.setsid()
                fcntl.ioctl(0, termios.TIOCSCTTY, 0)

            process = subprocess.Popen([binary, '--home', str(profile), *args], stdin=slave,
                                       stdout=slave, stderr=slave, env=env, preexec_fn=seat)
            os.close(slave)
            children.append((process, master))
            return process, master, bytearray()

        def wait(session, predicate, message):
            process, master, output = session
            deadline = time.monotonic() + 8
            while time.monotonic() < deadline:
                if select.select([master], [], [], .03)[0]:
                    output.extend(os.read(master, 65536))
                if predicate():
                    return
                if process.poll() is not None:
                    break
            raise AssertionError(message + ': ' + plain(session)[-1200:])

        def plain(session):
            return ''.join(re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b'', bytes(session[2])).decode(errors='replace').split())

        def send(session, keys):
            os.write(session[1], keys)

        def captured():
            if not capture.exists():
                return False
            data = json.loads(capture.read_text())
            assert data['herdr'] == [], data
            expected = ['-a', browser, page.as_uri()] if sys.platform == 'darwin' else [browser, page.as_uri()]
            assert data['args'] == expected, data
            assert not (root / 'injected').exists()
            return True

        try:
            sidebar = start(['ui', '--sidebar', str(root)], width=46)
            wait(sidebar, lambda: 'readme.txt' in plain(sidebar), 'Explorer did not load')
            send(sidebar, b's' + b'\x1b[B' * 6 + b'\r\r\x1b')
            wait(sidebar, lambda: json.loads(state.read_text())['preview_placement'] == 'browser', 'Browser setting did not persist')
            send(sidebar, b'g\x1b[B\r')
            wait(sidebar, captured, 'Browser mode did not open the selected HTML file')
            if fail:
                wait(sidebar, lambda: '浏览器打开失败' in plain(sidebar), 'Launcher failure was hidden')
                print('browser preview: launch failure shown; page and selection preserved')
                return
            wait(sidebar, lambda: '已通过浏览器打开网页' in plain(sidebar), 'Browser launch result did not render')
            capture.unlink()
            send(sidebar, b'\x1b[B\r')  # A text file must use the internal viewer path.
            wait(sidebar, lambda: 'previewneedsaherdrpane' in plain(sidebar), 'Text file did not retain internal preview')
            assert not capture.exists()
            send(sidebar, b'\x1b[A')
            send(sidebar, b's' + b'\x1b[B' * 6 + b'\r\x1b')
            wait(sidebar, lambda: json.loads(state.read_text())['preview_placement'] == 'tab', 'Internal preview mode was not restored')
            send(sidebar, b'm\x1b[B\r')
            wait(sidebar, captured, 'One-shot browser menu action did not open the HTML file')
            capture.unlink()
            scratch = profile / 'sidebar/scratch'
            scratch.mkdir(mode=0o700)
            control = scratch / 'p-browser-fixture.ctl'
            control.write_text(f'file\t{page}')
            viewer = start(['sidebar-viewer', str(control)])
            wait(viewer, lambda: '浏览器B' in plain(viewer), 'HTML preview header omitted the browser action')
            send(viewer, b'B')
            wait(viewer, captured, 'HTML preview B shortcut did not open the browser')
            wait(viewer, lambda: '已通过浏览器打开网页' in plain(viewer), 'Preview launch did not finish')
            capture.unlink()
            # At 100 columns, the action sits before the 18-column Find entry.
            send(viewer, b'\x1b[<0;73;1M\x1b[<0;73;1m')
            wait(viewer, captured, 'HTML preview browser button did not open the browser')
            assert json.loads(state.read_text())['preview_placement'] == 'tab'
            print('browser preview: mode, Unicode/space/shell-special path, menu, header, B shortcut, environment isolation and text fallback passed')
        finally:
            for process, master in children:
                os.close(master)
                if process.poll() is None:
                    process.kill()
                process.wait(timeout=10)


if __name__ == '__main__':
    for fail in [False, True]:
        check(str(Path(sys.argv[1]).resolve()), fail=fail)
