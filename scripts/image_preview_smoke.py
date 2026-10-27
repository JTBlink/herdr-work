"""Check native image preview and fallback in an isolated Unix PTY.

The fixture answers terminal capability queries; it never uses the live Herdr
session. This checks the application output, not an outer terminal's display.
"""
import argparse
import os
from pathlib import Path
import re
import struct
import subprocess
import tempfile
import time
import zlib


def check(binary):
    import fcntl
    import pty
    import select
    import signal
    import termios

    def png(width, height):
        def chunk(kind, data):
            return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind + data))
        # Fine vertical lines expose loss of resolution in character previews.
        row = b'\0' + bytes([0, 0, 0, 255, 255, 255]) * (width // 2)
        return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!2I5B', width, height, 8, 2, 0, 0, 0))
                + chunk(b'IDAT', zlib.compress(row * height)) + chunk(b'IEND', b''))

    for capability in ('kitty', 'disabled', 'timeout'):
        with tempfile.TemporaryDirectory(prefix='hw-image-smoke-') as name:
            root = Path(name)
            repo = root / 'repo'
            repo.mkdir()
            (repo / 'fine-lines.png').write_bytes(png(1600, 1004))
            subprocess.run(['git', 'init', '-q', str(repo)], check=True)
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 120, 1200, 800))
            env = {k: v for k, v in os.environ.items() if not k.startswith('HERDR_')}
            # Herdr's own replies must win over inherited outer-terminal hints.
            env.update(TERM='xterm-256color', TERM_PROGRAM='herdr', HERDR_ENV='1',
                       WEZTERM_EXECUTABLE='/fixture/wezterm', HERDR_PROJECTS_NO_KEEPER='1',
                       HERDR_PROJECTS_INLINE='1')

            def seat():
                os.setsid()
                fcntl.ioctl(0, termios.TIOCSCTTY, 0)

            process = subprocess.Popen([str(binary), '--home', str(root / 'profile'),
                                        'ui', '--standalone', str(repo)],
                                       stdin=slave, stdout=slave, stderr=slave,
                                       env=env, preexec_fn=seat)
            stream = bytearray()
            answered = False

            def wait_for(predicate, timeout=15):
                nonlocal answered
                deadline = time.monotonic() + timeout
                while time.monotonic() < deadline:
                    if select.select([master], [], [], .05)[0]:
                        data = os.read(master, 65536)
                        stream.extend(data)
                        if not answered and b'\x1b[5n' in stream:
                            if capability != 'timeout':
                                reply = b'\x1b_Gi=31;OK\x1b\\' if capability == 'kitty' else b''
                                os.write(master, reply + b'\x1b[6;20;10t\x1b[0n')
                            answered = True
                    if predicate(bytes(stream)):
                        return
                    assert process.poll() is None, f'{capability}: UI exited'
                raise AssertionError(f'{capability}: preview timed out; tail={bytes(stream[-400:])!r}')

            try:
                wait_for(lambda data: answered and b'fine-lines.png' in data)
                stream.clear()
                os.write(master, b'\r')
                if capability == 'kitty':
                    pattern = rb'a=T,U=1,f=32,t=d,s=(\d+),v=(\d+)'
                    wait_for(lambda data: re.search(pattern, data) is not None)
                    before = tuple(map(int, re.search(pattern, stream).groups()))
                    assert before[0] > 120 * 5, f'Image reduced to character resolution: {before}'
                    stream.clear()
                    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 80, 800, 480))
                    os.killpg(process.pid, signal.SIGWINCH)
                    wait_for(lambda data: re.search(pattern, data) is not None)
                    after = tuple(map(int, re.search(pattern, stream).groups()))
                    assert after[0] < before[0] and after[1] < before[1], (before, after)
                    print(f'kitty: native pixels {before} -> {after}; inherited WezTerm hint ignored')
                else:
                    wait_for(lambda data: '▀'.encode() in data)
                    assert b'a=T,U=1' not in stream
                    print(f'{capability}: portable fallback and keyboard input work')
            finally:
                # Release PTY flow control before reaping a process that may
                # still be writing a multi-chunk native image transmission.
                os.close(master)
                os.close(slave)
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    args = parser.parse_args()
    check(args.binary.resolve())
