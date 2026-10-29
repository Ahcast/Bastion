#!/usr/bin/env python3
"""Remove only abandoned root-owned private tender jobs.

The shared lock prevents cleanup while a launcher is active. Systemd state is
checked again because an isolated unit may survive its launcher. A mounted
volume is unmounted only if its loop device is backed by that job's image.
"""
import fcntl
import os
import pathlib
import re
import shutil
import stat
import subprocess
import sys

PRIVATE = pathlib.Path('/var/lib/bastion-tender/private')
INCOMING = pathlib.Path('/var/lib/bastion-tender/incoming')
LOCK = pathlib.Path('/run/lock/bastion-tender-worker.lock')
ENV = {'PATH': '/usr/sbin:/usr/bin:/sbin:/bin', 'LANG': 'C'}


def call(args):
    return subprocess.run(args, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                          stderr=subprocess.DEVNULL, timeout=30, env=ENV, text=True)


def recover():
    if os.geteuid() != 0:
        raise PermissionError('root required')
    info = PRIVATE.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o077:
        raise ValueError('invalid private directory')
    fd = os.open(LOCK, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_nlink != 1 or info.st_mode & 0o022:
            raise ValueError('invalid lock')
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        with os.scandir(PRIVATE) as entries:
            for item in entries:
                if not re.fullmatch(r'[0-9a-f]{48}', item.name):
                    continue
                info = item.stat(follow_symlinks=False)
                if not stat.S_ISDIR(info.st_mode) or info.st_uid != 0:
                    continue
                job = PRIVATE / item.name
                unit = 'bastion-tender-' + item.name + '.service'
                state = call(['/usr/bin/systemctl', 'show', unit, '--property=ActiveState', '--value'])
                if state.returncode not in (0, 4) or state.stdout.strip() not in ('inactive', 'failed', ''):
                    continue
                volume = job / 'volume'
                if volume.is_mount():
                    source = call(['/usr/bin/findmnt', '-n', '-o', 'SOURCE', '--mountpoint', str(volume)])
                    device = source.stdout.strip()
                    if source.returncode or not re.fullmatch(r'/dev/loop[0-9]+', device):
                        continue
                    backing = call(['/usr/sbin/losetup', '-n', '-O', 'BACK-FILE', device])
                    if backing.returncode or backing.stdout.strip() != str(job / 'image'):
                        continue
                    if call(['/usr/bin/umount', str(volume)]).returncode or volume.is_mount():
                        continue
                shutil.rmtree(job)
                # A crash while copying the result may leave an incomplete,
                # root-owned output. Never remove a Bastion-owned job or PDF.
                output = INCOMING / item.name
                try:
                    output_info = output.lstat()
                except FileNotFoundError:
                    continue
                if stat.S_ISDIR(output_info.st_mode) and output_info.st_uid == 0:
                    shutil.rmtree(output)
    finally:
        os.close(fd)


if __name__ == '__main__':
    try:
        if len(sys.argv) != 1:
            raise ValueError('unexpected arguments')
        recover()
    except BlockingIOError:
        pass
    except Exception:
        print('No se pudo limpiar un trabajo PDF interrumpido.', file=sys.stderr)
        sys.exit(1)
