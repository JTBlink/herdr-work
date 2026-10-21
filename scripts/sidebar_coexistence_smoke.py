"""Run real Sidebar/HerdrWork hooks in disposable Herdr servers (macOS/Linux).

Only the supplied Sidebar executable is read, copied and hashed. Its installed
configuration, registry and running panes are never used. The copied plugins
are linked without build/update hooks into a fresh XDG configuration.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pty
import select
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def check(binary, sidebar, side, evidence):
    base = Path(tempfile.mkdtemp(prefix='hw-coexist-', dir='/tmp'))
    env = {key: value for key, value in os.environ.items() if not key.startswith('HERDR_')}
    env.update(XDG_CONFIG_HOME=str(base / 'config'), XDG_DATA_HOME=str(base / 'data'),
               XDG_CACHE_HOME=str(base / 'cache'), XDG_STATE_HOME=str(base / 'state'),
               HERDR_CONFIG_PATH=str(base / 'config/herdr/config.toml'))
    config = Path(env['HERDR_CONFIG_PATH'])
    config.parent.mkdir(parents=True)
    config.write_text('')
    evidence.mkdir(parents=True, exist_ok=True)
    # sidebar_smoke creates its output directory itself.
    output = evidence / side
    profile = base / 'work-profile'
    (profile / 'sidebar').mkdir(parents=True)
    (profile / 'sidebar/state.json').write_text('{"auto_open":false,"font_prompt":true}')
    independent = base / 'sidebar'
    independent.mkdir()
    copied = independent / 'herdr-sidebar'
    shutil.copy2(sidebar, copied)
    copied.chmod(0o755)
    independent_profile = independent / 'profile'
    independent_profile.mkdir()
    (independent_profile / 'state.json').write_text(json.dumps({
        'merged': True, 'font_prompt': True, 'auto_open': False,
        'dock_right': side == 'right', 'sidebar_width': 28,
    }))
    launcher = independent / 'launch.py'
    launcher.write_text('#!' + sys.executable + '\nimport os, sys\n'
        + 'os.environ["HERDR_PLUGIN_STATE_DIR"] = ' + repr(str(independent_profile)) + '\n'
        + 'os.execv(' + repr(str(copied)) + ', [' + repr(str(copied)) + '] + sys.argv[1:])\n')
    launcher.chmod(0o755)
    command = json.dumps([str(launcher)])
    ensure = json.dumps([str(launcher), '--ensure'])
    manifest = ('id = "herdr-sidebar"\nname = "Sidebar coexistence fixture"\n'
        'version = "0.15.1"\nmin_herdr_version = "0.9.3"\n'
        '[[panes]]\nid = "sidebar"\ntitle = "Sidebar"\nplacement = "split"\ncommand = '
        + command + '\n[[panes]]\nid = "source-control"\ntitle = "Source Control"\n'
        'placement = "split"\ncommand = ' + json.dumps([str(launcher), '--view', 'git']) + '\n')
    for event in ['pane.focused', 'tab.created', 'tab.focused', 'workspace.created', 'workspace.focused']:
        manifest += '[[events]]\non = ' + json.dumps(event) + '\ncommand = ' + ensure + '\n'
    (independent / 'herdr-plugin.toml').write_text(manifest)
    own = base / 'herdr-work'
    own.mkdir()
    own_command = [str(binary), '--home', str(profile)]
    manifest = ('id = "jtblink.herdr-work"\nname = "HerdrWork coexistence fixture"\n'
        'version = "0.1.6"\nmin_herdr_version = "0.9.3"\n'
        '[[panes]]\nid = "pane"\ntitle = "HerdrWork"\nplacement = "split"\ncommand = '
        + json.dumps(own_command + ['ui', '--sidebar']) + '\n')
    for event in ['tab.created', 'workspace.created']:
        manifest += '[[events]]\non = ' + json.dumps(event) + '\ncommand = ' + json.dumps(own_command + ['sidebar-ensure']) + '\n'
    (own / 'herdr-plugin.toml').write_text(manifest)
    log = (evidence / ('server-' + side + '.log')).open('w')
    server = subprocess.Popen(['herdr', '--session', 'coexistence-test', 'server'], env=env,
                              stdout=log, stderr=subprocess.STDOUT)
    client = None
    stopping = threading.Event()
    try:
        deadline = time.monotonic() + 15
        address = None
        while time.monotonic() < deadline:
            candidates = list(base.rglob('herdr.sock'))
            if candidates:
                address = candidates[0]
                try:
                    with socket.socket(socket.AF_UNIX) as stream:
                        stream.connect(str(address))
                    break
                except OSError:
                    pass
            if server.poll() is not None:
                raise RuntimeError('Disposable Herdr server exited; see server log')
            time.sleep(.1)
        assert address, 'Disposable Herdr socket missing'
        for plugin in [own, independent]:
            linked = subprocess.run(['herdr', '--session', 'coexistence-test', 'plugin', 'link', str(plugin)],
                env=env, capture_output=True, text=True, timeout=30)
            assert linked.returncode == 0, linked.stderr
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 48, 160, 0, 0))

        def seat():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        client = subprocess.Popen(['herdr', '--session', 'coexistence-test'],
            env=dict(env, TERM='xterm-256color'), stdin=slave, stdout=slave, stderr=slave, preexec_fn=seat)
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
        time.sleep(1)
        checked = subprocess.run([sys.executable, str(ROOT / 'scripts/sidebar_smoke.py'),
            '--socket', str(address), '--binary', str(binary), '--profile', str(profile), '--independent-side', side,
            '--independent-binary', str(copied), '--independent-profile', str(independent_profile),
            '--coexistence-hooks', '--output', str(output)], env=env, timeout=180)
        assert checked.returncode == 0, 'Real Sidebar coexistence failed: ' + side
    finally:
        if client:
            client.terminate()
            client.wait(timeout=10)
            stopping.set()
            reader.join(timeout=2)
            os.close(master)
        server.terminate()
        server.wait(timeout=15)
        log.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--sidebar-binary', required=True, type=Path)
    parser.add_argument('--side', choices=['left', 'right'])
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    binary, sidebar = args.binary.resolve(strict=True), args.sidebar_binary.resolve(strict=True)
    before = digest(sidebar)
    try:
        for side in [args.side] if args.side else ['right', 'left']:
            check(binary, sidebar, side, args.output.resolve())
    finally:
        assert digest(sidebar) == before, 'Installed Sidebar executable changed'
    print('Real Sidebar hooks coexist with HerdrWork; source executable unchanged')
