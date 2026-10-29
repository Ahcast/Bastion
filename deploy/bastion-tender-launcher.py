#!/usr/bin/env python3
"""Privileged launcher for a single isolated tender PDF job on Lightsail.

Only the Bastion service account may call this fixed executable through sudo.
It accepts one random 48-character job identifier; no command, path, flags or
credentials come from the request. All untrusted input runs in the worker.
"""
import fcntl
import os
import pathlib
import pwd
import re
import shutil
import stat
import subprocess
import sys

BASE = pathlib.Path('/var/lib/bastion-tender')
INCOMING = BASE / 'incoming'
PRIVATE = BASE / 'private'
BIND_TARGET = pathlib.Path('/run/bastion-tender-job')
MARKER = pathlib.Path('/run/bastion-tender-host-netns')
WORKER = '/usr/local/libexec/bastion-tender-pdf-worker'
QPDF = '/opt/bastion-pdf/qpdf-12.4.2/bin/qpdf'
PACKAGES = '/opt/bastion-pdf/typst-packages'
CAPACITY = 5 * 1024**3
MAX_INPUT = 1_200_000_000
MAX_OUTPUT = 1_000_000_000


def run(args, *, timeout=60):
    return subprocess.run(args, check=True, stdin=subprocess.DEVNULL,
                          stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                          timeout=timeout, env={'PATH': '/usr/sbin:/usr/bin:/sbin:/bin', 'LANG': 'C'})


def validate_name(name):
    return (len(name) <= 128 and re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]*', name)
            and pathlib.PurePath(name).suffix in {'.typ', '.json', '.pdf', '.bin'})


def check_staged(directory, uid):
    info = directory.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != uid or info.st_mode & 0o077:
        raise ValueError('invalid directory')
    total = 0
    files = []
    with os.scandir(directory) as entries:
        for entry in entries:
            if len(files) >= 10000 or not validate_name(entry.name):
                raise ValueError('invalid filename')
            item = entry.stat(follow_symlinks=False)
            if not stat.S_ISREG(item.st_mode) or item.st_nlink != 1 or item.st_uid != uid:
                raise ValueError('invalid file')
            total += item.st_size
            if total > MAX_INPUT or (entry.name == 'request.json' and item.st_size > 2_000_000):
                raise ValueError('input too large')
            files.append(entry.name)
    if 'request.json' not in files:
        raise ValueError('missing request')
    return files


def write_marker():
    flags = os.O_WRONLY | os.O_CREAT | os.O_NOFOLLOW
    fd = os.open(MARKER, flags, 0o644)
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
            raise ValueError('invalid namespace marker')
        value = os.readlink('/proc/1/ns/net')
        if not re.fullmatch(r'net:\[\d+\]', value):
            raise ValueError('invalid namespace')
        os.ftruncate(fd, 0)
        os.write(fd, (value + '\n').encode())
    finally:
        os.close(fd)


def launch(job_id):
    account = pwd.getpwnam('bastion')
    uid, gid = account.pw_uid, account.pw_gid
    if os.geteuid() != 0 or os.environ.get('SUDO_UID') != str(uid):
        raise PermissionError('invalid caller')
    if not re.fullmatch(r'[0-9a-f]{48}', job_id):
        raise ValueError('invalid identifier')
    for path in (BASE, INCOMING, PRIVATE, BIND_TARGET):
        info = path.lstat()
        if not path.is_absolute() or not stat.S_ISDIR(info.st_mode) or info.st_uid != 0:
            raise ValueError('invalid configured directory')
        if path == INCOMING:
            if info.st_gid != gid or info.st_mode & 0o007 or info.st_mode & 0o030 != 0o030:
                raise ValueError('unsafe incoming directory')
        elif path == PRIVATE:
            if info.st_mode & 0o077:
                raise ValueError('unsafe private directory')
        elif info.st_mode & 0o022:
            raise ValueError('unsafe configured directory')
    staged = INCOMING / job_id
    check_staged(staged, uid)
    space = os.statvfs(PRIVATE)
    if space.f_bavail * space.f_frsize < CAPACITY + 2_000_000_000:
        raise ValueError('insufficient disk space')
    lock = os.open('/run/lock/bastion-tender-worker.lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    try:
        info = os.fstat(lock)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_nlink != 1 or info.st_mode & 0o022:
            raise ValueError('unsafe lock')
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        job = PRIVATE / job_id
        job.mkdir(mode=0o700)
        moved = job / 'input'
        mount = job / 'volume'
        image = job / 'image'
        unit = 'bastion-tender-' + job_id + '.service'
        mounted = False
        try:
            os.rename(staged, moved)
            names = check_staged(moved, uid)
            mount.mkdir(mode=0o700)
            run(['/usr/bin/fallocate', '-l', str(CAPACITY), str(image)], timeout=120)
            run(['/usr/sbin/mkfs.ext4', '-q', '-F', str(image)], timeout=120)
            run(['/usr/bin/mount', '-o', 'loop,nodev,nosuid,noexec', str(image), str(mount)])
            mounted = True
            os.chmod(mount, 0o777)
            for name in names:
                source = moved / name
                target = mount / name
                fd = os.open(source, os.O_RDONLY | os.O_NOFOLLOW)
                with os.fdopen(fd, 'rb') as src, open(target, 'xb') as dst:
                    info = os.fstat(src.fileno())
                    if not stat.S_ISREG(info.st_mode) or info.st_uid != uid or info.st_nlink != 1:
                        raise ValueError('invalid source')
                    shutil.copyfileobj(src, dst, 256 * 1024)
                os.chmod(target, 0o644)
            write_marker()
            props = [
                'DynamicUser=yes', 'PrivateNetwork=yes', 'PrivateTmp=yes',
                'NoNewPrivileges=yes', 'CapabilityBoundingSet=', 'ProtectSystem=strict',
                'ProtectHome=yes', 'ProtectKernelTunables=yes', 'ProtectKernelModules=yes',
                'ProtectControlGroups=yes', 'ProtectProc=invisible', 'ProcSubset=pid',
                'RestrictSUIDSGID=yes', 'LockPersonality=yes', 'MemoryMax=1G',
                'TasksMax=32', 'CPUQuota=150%', 'RuntimeMaxSec=900',
                'LimitFSIZE=1100000000', 'StandardOutput=null', 'StandardError=null',
                f'BindPaths={mount}:{BIND_TARGET}',
                f'Environment=BASTION_WORKER_ROOT={BIND_TARGET}',
                'Environment=BASTION_TYPST_BIN=/usr/local/bin/typst',
                'Environment=BASTION_PDFINFO_BIN=/usr/bin/pdfinfo',
                f'Environment=BASTION_QPDF_BIN={QPDF}',
                f'Environment=BASTION_TYPST_PACKAGE_PATH={PACKAGES}',
                'Environment=BASTION_TYPST_FONT_PATH=/usr/share/fonts',
                'UnsetEnvironment=AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_SESSION_TOKEN AWS_PROFILE AWS_SHARED_CREDENTIALS_FILE AWS_CONFIG_FILE AWS_CONTAINER_CREDENTIALS_FULL_URI AWS_CONTAINER_CREDENTIALS_RELATIVE_URI',
            ]
            args = ['/usr/bin/systemd-run', '--quiet', '--wait', '--collect', '--unit', unit]
            for prop in props:
                args.extend(['--property', prop])
            args.extend(['/bin/sh', '-c', f'exec {WORKER} < {BIND_TARGET}/request.json'])
            try:
                run(args, timeout=960)
            finally:
                subprocess.run(['/usr/bin/systemctl', 'stop', unit], stdin=subprocess.DEVNULL,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30, check=False)
            result = mount / 'result.pdf'
            info = result.lstat()
            if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or not 5 <= info.st_size <= MAX_OUTPUT:
                raise ValueError('invalid output')
            with open(result, 'rb') as f:
                if f.read(5) != b'%PDF-':
                    raise ValueError('invalid output')
            output = INCOMING / job_id
            output.mkdir(mode=0o700)
            try:
                with open(result, 'rb') as src, open(output / 'result.pdf', 'xb') as dst:
                    shutil.copyfileobj(src, dst, 256 * 1024)
                os.chmod(output / 'result.pdf', 0o600)
                os.chown(output / 'result.pdf', uid, -1)
                os.chown(output, uid, -1)
            except BaseException:
                shutil.rmtree(output)
                raise
        finally:
            if mounted:
                run(['/usr/bin/umount', str(mount)])
            if not os.path.ismount(mount):
                shutil.rmtree(job)
    finally:
        os.close(lock)


if __name__ == '__main__':
    try:
        if len(sys.argv) != 2:
            raise ValueError('invalid arguments')
        launch(sys.argv[1])
        print('ok')
    except Exception:
        print('No se pudo generar el PDF.', file=sys.stderr)
        sys.exit(1)
