"""Install HerdrWork's default host shortcut without replacing user bindings."""
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import shlex
import stat
import subprocess
import tempfile
import tomllib

PLUGIN = 'jtblink.herdr-work'
KEY = 'prefix+f'
COMMAND = f'herdr plugin action invoke show-explorer --plugin {PLUGIN}'
BLOCK = ('\n# HerdrWork: open/focus Explorer\n[[keys.command]]\n'
         f'key = "{KEY}"\ntype = "shell"\ncommand = "{COMMAND}"\n')


def config_path():
    override = os.environ.get('HERDR_CONFIG_PATH')
    if override:
        return Path(override).expanduser().resolve()
    base = Path(os.environ.get('XDG_CONFIG_HOME') or Path.home() / '.config')
    return (base / 'herdr/config.toml').expanduser().resolve()


@contextmanager
def install_lock(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('a+b') as stream:
        if os.name == 'nt':
            import msvcrt
            stream.write(b'\0')
            stream.flush()
            stream.seek(0)
            msvcrt.locking(stream.fileno(), msvcrt.LK_LOCK, 1)
        else:
            import fcntl
            fcntl.flock(stream, fcntl.LOCK_EX)
        try:
            yield
        finally:
            if os.name == 'nt':
                stream.seek(0)
                msvcrt.locking(stream.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(stream, fcntl.LOCK_UN)


def normalized(key):
    return ''.join(key.lower().split()).replace('control+', 'ctrl+')


def configure(config, profile):
    config = Path(config).expanduser().resolve()
    key = hashlib.sha256(str(config).encode()).hexdigest()
    receipt = Path(profile) / 'install' / f'host-keys-{key}.json'
    with install_lock(receipt.with_suffix('.lock')):
        original = config.read_bytes() if config.exists() else b''
        data = tomllib.loads(original.decode('utf-8'))
        keys = data.get('keys', {})
        prefix = keys.get('prefix', 'ctrl+b')
        if receipt.exists():
            return 'preserved', prefix
        commands = keys.get('command', [])
        for entry in commands:
            try:
                own = PLUGIN in shlex.split(entry.get('command', ''))
            except ValueError:
                own = False
            if own:
                receipt.write_text(json.dumps({'config': str(config), 'existing': True}))
                return 'preserved', prefix
        bindings = [entry.get('key', '') for entry in commands]
        bindings += [value for name, value in keys.items()
                     if name != 'prefix' and isinstance(value, str)]
        if any(normalized(binding) in {KEY, 'f'} for binding in bindings):
            return 'conflict', prefix
        updated = original + BLOCK.encode('utf-8')
        # Inline TOML tables may be sealed against appending nested arrays.
        tomllib.loads(updated.decode('utf-8'))
        config.parent.mkdir(parents=True, exist_ok=True)
        backup = None
        if original:
            with tempfile.NamedTemporaryFile(prefix=config.name + '.herdr-work-backup-',
                                             dir=config.parent, delete=False) as saved:
                saved.write(original)
                backup = saved.name
        temp_path = None
        try:
            with tempfile.NamedTemporaryFile(dir=config.parent, delete=False) as pending:
                temp_path = Path(pending.name)
                pending.write(updated)
                pending.flush()
                os.fsync(pending.fileno())
            mode = stat.S_IMODE(config.stat().st_mode) if config.exists() else 0o600
            temp_path.chmod(mode)
            if (config.read_bytes() if config.exists() else b'') != original:
                raise OSError('Herdr 配置已被其他进程修改，请重新运行配置步骤')
            os.replace(temp_path, config)
            receipt.write_text(json.dumps({'config': str(config), 'key': KEY, 'backup': backup}))
        finally:
            if temp_path:
                temp_path.unlink(missing_ok=True)
        return 'added', prefix


def setup(profile=None):
    try:
        if profile is None:
            binary = Path(__file__).resolve().parents[1] / 'bin/herdr-work'
            profile = Path(json.loads(subprocess.check_output(
                [str(binary), 'profile-path'], text=True, timeout=30))['profile'])
        status, prefix = configure(config_path(), profile)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f'HerdrWork 快捷键未配置，保留现有配置：{error}')
        return
    if status == 'conflict':
        print(f'HerdrWork 快捷键 {prefix} → F 已被占用，保留原绑定。')
    elif status == 'preserved':
        print('HerdrWork 快捷键已配置过，保留用户当前设置。')
    else:
        print(f'HerdrWork 默认快捷键：{prefix} → F（打开或聚焦文件树）。')
        try:
            reloaded = subprocess.run(['herdr', 'server', 'reload-config'],
                                      capture_output=True, timeout=15)
            if reloaded.returncode:
                print('Herdr 配置暂未重载，下次启动时生效。')
        except (OSError, subprocess.SubprocessError):
            print('Herdr 配置暂未重载，下次启动时生效。')


if __name__ == '__main__':
    setup()
