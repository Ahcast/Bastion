"""Fixed-scope backup engine. No executable or host path comes from HTTP."""
import contextlib
import datetime
import fcntl
import hashlib
import hmac
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import sqlite3
import subprocess
import selectors
import time
import tempfile
import zipfile
from zoneinfo import ZoneInfo
from bastion_backup_inventory import BUCKET, inventory
from bastion_backup_validate import validate_archive
from bastion_backup_plan import LOCAL_ROOTS, LOCAL_FILES, restore_plan
from bastion_backup_zip import write_archive

KEY = re.compile(r'backups/bastion-backup-[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{6}\.zip')


class GuardedReader:
    def __init__(self, stream, root): self.stream, self.root = stream, root
    def read(self, count):
        if shutil.disk_usage(self.root).free < 1024**3: raise RuntimeError('Insufficient temporary space')
        return self.stream.read(count)


class Engine:
    def __init__(self, root):
        self.root = root
        self.aws_env = {'PATH': '/usr/local/bin:/usr/bin:/bin', 'AWS_SHARED_CREDENTIALS_FILE': '/etc/bastion/aws-s3-credentials', 'AWS_PROFILE': 'bastion-documents', 'AWS_EC2_METADATA_DISABLED': 'true', 'AWS_PAGER': ''}
        self.cfg = {}
        for line in Path('/etc/bastion/backend.env').read_text().splitlines():
            if '=' in line and not line.lstrip().startswith('#'):
                key, value = line.split('=', 1)
                self.cfg[key] = shlex.split(value)[0] if value else ''
        if not re.fullmatch(r'[A-Za-z0-9_]+',self.cfg.get('MYSQL_DATABASE','')):
            raise ValueError('Unsupported database name')
        self.cache, self.cache_time = [], 0

    def aws(self, args, operation_timeout=7200, **kwargs):
        return subprocess.run(['aws', *args, '--region', 'us-east-1', '--no-cli-pager'], env=self.aws_env, check=True, capture_output=True, timeout=operation_timeout, **kwargs)

    @staticmethod
    def valid_key(key):
        if not isinstance(key, str) or not KEY.fullmatch(key): raise ValueError('Invalid backup key')
        return key

    @staticmethod
    def digest(path):
        with path.open('rb') as f: return hashlib.file_digest(f, 'sha256').hexdigest()

    @staticmethod
    def next_run():
        tz = ZoneInfo('America/Tegucigalpa')
        current = datetime.datetime.now(tz)
        target = current.replace(hour=17, minute=0, second=0, microsecond=0)
        if target <= current: target += datetime.timedelta(days=1)
        return target.isoformat()

    @staticmethod
    def wait_ready():
        import time
        import urllib.request
        for _ in range(20):
            try:
                with urllib.request.urlopen('http://127.0.0.1:8080/health/ready', timeout=3) as response:
                    if response.status == 200: return
            except OSError:
                pass
            time.sleep(3)
        subprocess.run(['systemctl','stop','bastion.service'],check=False,timeout=120)
        raise RuntimeError('Backend did not become ready; inspect maintenance state')

    def history(self):
        import time
        if time.monotonic() - self.cache_time < 60: return self.cache
        values, token = [], None
        deadline = time.monotonic() + 8
        while True:
            if time.monotonic() >= deadline: raise RuntimeError('History request timed out')
            args = ['s3api', 'list-objects-v2', '--bucket', BUCKET, '--prefix', 'backups/bastion-backup-', '--max-keys', '1000']
            if token: args += ['--continuation-token', token]
            page = json.loads(self.aws(args, operation_timeout=max(1,deadline-time.monotonic())).stdout)
            for obj in page.get('Contents', []):
                if KEY.fullmatch(obj['Key']): values.append({'key': obj['Key'], 'date': obj['LastModified'], 'size': obj['Size'], 'status': 'Almacenado'})
            token = page.get('NextContinuationToken')
            if not token: break
        self.cache = sorted(values, key=lambda x: x['date'], reverse=True)[:100]
        self.cache_time = time.monotonic()
        return self.cache

    def download_url(self, key):
        return self.aws(['s3', 'presign', 's3://' + BUCKET + '/' + self.valid_key(key), '--expires-in', '300'], operation_timeout=8).stdout.decode().strip()

    def retrieve(self, key, path):
        key = self.valid_key(key)
        metadata = json.loads(self.aws(['s3api', 'head-object', '--bucket', BUCKET, '--key', key]).stdout)
        size = metadata['ContentLength']
        if size > 40 * 1024**3 or shutil.disk_usage(self.root).free < size * 2 + 1024**3:
            raise ValueError('Insufficient space')
        self.aws(['s3api', 'get-object', '--bucket', BUCKET, '--key', key, '--if-match', metadata['ETag'], str(path)])
        os.chmod(path, 0o600)

    def signing_key(self, manifest=None):
        if manifest is None: return Path('/etc/bastion/data-encryption-key').read_bytes()
        key_id = manifest.get('key_id')
        if not isinstance(key_id, str) or not re.fullmatch('[0-9a-f]{64}', key_id): raise ValueError('Invalid recovery key')
        try: local = Path('/etc/bastion/data-encryption-key').read_bytes()
        except FileNotFoundError: local = b''
        if hashlib.sha256(local).hexdigest() == key_id: return local
        with tempfile.TemporaryDirectory(dir=self.root, prefix='transient-') as tmp:
            path = Path(tmp) / 'key'
            object_key = 'backups/recovery/' + key_id
            head = json.loads(self.aws(['s3api','head-object','--bucket',BUCKET,'--key',object_key]).stdout)
            if not 0 < head['ContentLength'] <= 4096: raise ValueError('Invalid recovery key size')
            self.aws(['s3api', 'get-object', '--bucket', BUCKET, '--key', object_key, '--if-match',head['ETag'], str(path)])
            key = path.read_bytes()
        if hashlib.sha256(key).hexdigest() != key_id: raise ValueError('Recovery key integrity failure')
        return key

    def inspect(self, path, progress, details=False):
        progress('Validando ZIP y checksums')
        manifest = validate_archive(path)
        canonical = dict(manifest)
        signature = canonical.pop('signature', None)
        raw = json.dumps(canonical, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
        key = self.signing_key(manifest)
        derived = hashlib.sha256(b'bastion-backup-manifest-v1\0' + key).digest()
        expected = hmac.new(derived, raw, hashlib.sha256).hexdigest()
        if not isinstance(signature, str) or not hmac.compare_digest(expected, signature): raise ValueError('Invalid backup signature')
        current = Path('/opt/bastion/current/BUILD.txt').read_text().splitlines()[0]
        if manifest.get('commit') != current: raise ValueError('Deploy the backup commit before restoring')
        plan = restore_plan(manifest)
        if shutil.disk_usage(self.root).free < sum(x['size'] for x in manifest['files']) + 1024**3:
            raise ValueError('Insufficient space for restoration')
        summary = f"Se reemplazará la base principal completa: facturas, cotizaciones, órdenes, usuarios, roles, permisos y configuración; {len(plan['local_files'])} archivos locales y {len(plan['s3_objects'])} objetos de S3. Los objetos operativos posteriores al respaldo se retirarán. Se cerrarán todas las sesiones. No se reemplaza código ni backups/."
        return (manifest, plan) if details else summary

    @contextlib.contextmanager
    def mysql_options(self):
        def quote(v): return '"' + v.replace('\\', '\\\\').replace('"', '\\"').replace('\n','\\n').replace('\r','\\r').replace('\x00','\\0') + '"'
        with tempfile.TemporaryDirectory(dir='/dev/shm', prefix='bastion-backup-') as tmp:
            path = Path(tmp) / 'client.cnf'
            path.touch(mode=0o600)
            values = [('host', self.cfg['MYSQL_HOST']), ('port', self.cfg.get('MYSQL_PORT', '3306')), ('user', self.cfg['MYSQL_USER']), ('password', Path('/etc/bastion/mysql-password').read_text().rstrip('\r\n'))]
            path.write_text('[client]\n' + ''.join(k + '=' + quote(v) + '\n' for k,v in values))
            yield ['--defaults-extra-file=' + str(path), '--ssl-mode=VERIFY_IDENTITY', '--ssl-ca=/etc/bastion/mysql-ca.pem']

    @contextlib.contextmanager
    def maintenance(self, restart_on_error=True):
        services = ['bastion-tender-recover.timer', 'bastion-tender-launcher.socket', 'bastion-tender-recover.service', 'bastion-scout-control.service', 'bastion.service']
        active = [s for s in services if subprocess.run(['systemctl', 'is-active', '--quiet', s]).returncode == 0]
        running = subprocess.run(['systemctl', 'list-units', '--state=running', '--no-legend', 'bastion-tender-launcher@*.service'], capture_output=True, check=True).stdout
        if running.strip(): raise RuntimeError('Tender job running; retry after completion')
        success = False
        self.mutating = False
        with Path('/var/lib/bastion-scout/pipeline.lock').open('a+b') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            (self.root / 'maintenance.json').write_text(json.dumps({'services': active, 'restart_on_error': True}))
            try:
                for service in active: subprocess.run(['systemctl', 'stop', service], check=True, timeout=120)
                running = subprocess.run(['systemctl', 'list-units', '--state=running', '--no-legend', 'bastion-tender-launcher@*.service'], capture_output=True, check=True).stdout
                if running.strip(): raise RuntimeError('Tender job running; retry after completion')
                yield
                success = True
            finally:
                if success or restart_on_error or not self.mutating:
                    for service in reversed(active):
                        subprocess.run(['systemctl', 'start', service], check=True, timeout=120)
                        if service == 'bastion.service': self.wait_ready()
                    (self.root / 'maintenance.json').unlink(missing_ok=True)
                    (self.root / 'maintenance-required').unlink(missing_ok=True)
                else:
                    (self.root / 'maintenance-required').write_text('Operation failed. Review job and recover data before restarting services.\n')

    def backup(self, progress):
        marker = self.root / 'maintenance.json'
        if (self.root / 'maintenance-required').exists() or (marker.exists() and not json.loads(marker.read_text()).get('restart_on_error')):
            raise RuntimeError('Resolve interrupted restoration before creating another backup')
        with self.maintenance():
            return self.snapshot(progress)

    def snapshot(self, progress):
        progress('Recopilando inventario')
        objects = list(inventory(self.aws_env))
        required = sum(o['size'] for o in objects) * 2 + 2 * 1024**3
        if shutil.disk_usage(self.root).free < required: raise RuntimeError('Insufficient space')
        with tempfile.TemporaryDirectory(dir=self.root, prefix='snapshot-') as tmp:
            folder = Path(tmp)
            dump = folder / 'main.sql'
            progress('Exportando MySQL')
            with self.mysql_options() as options, dump.open('wb') as output:
                engines = subprocess.run(['mysql', *options, '--batch', '--skip-column-names', self.cfg['MYSQL_DATABASE'], '-e', "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND ENGINE IS NOT NULL AND ENGINE <> 'InnoDB'"], capture_output=True,check=True,timeout=120).stdout.strip()
                if engines != b'0': raise RuntimeError('Nontransactional tables require a different snapshot strategy')
                process = subprocess.Popen(['mysqldump', *options, '--column-statistics=0', '--set-gtid-purged=OFF', '--single-transaction', '--quick', '--skip-lock-tables', '--hex-blob', '--routines', '--triggers', '--events', self.cfg['MYSQL_DATABASE']], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
                try:
                    size = 0
                    with selectors.DefaultSelector() as ready:
                        ready.register(process.stdout, selectors.EVENT_READ)
                        deadline = time.monotonic() + 7200
                        while True:
                            if time.monotonic() > deadline: raise RuntimeError('Database snapshot timeout')
                            if not ready.select(30): continue
                            chunk = process.stdout.read1(1024*1024)
                            if not chunk: break
                            size += len(chunk)
                            if size > 20 * 1024**3 or shutil.disk_usage(self.root).free < 2 * 1024**3: raise RuntimeError('Insufficient space for database snapshot')
                            output.write(chunk)
                    if process.wait(timeout=120): raise RuntimeError('Database snapshot failed')
                finally:
                    process.stdout.close()
                    if process.poll() is None:
                        process.terminate()
                        try: process.wait(timeout=10)
                        except subprocess.TimeoutExpired: process.kill(); process.wait()
            name = 'bastion-backup-' + datetime.datetime.now(ZoneInfo('America/Tegucigalpa')).strftime('%Y-%m-%d-%H%M%S') + '.zip'
            archive = folder / name
            def streams():
                with dump.open('rb') as source: yield 'databases/main.sql', GuardedReader(source,self.root)
                for label, root in LOCAL_ROOTS.items():
                    files = sorted(root.rglob('*')) if label == 'branding' else [root / name for name in sorted(LOCAL_FILES[label])]
                    for path in files:
                        if not path.is_file() or path.is_symlink(): continue
                        if label == 'branding' and path.suffix.lower() not in ('.png','.jpg','.jpeg','.webp','.svg'): continue
                        relative = path.relative_to(root).as_posix()
                        actual = path
                        if path.suffix == '.sqlite3':
                            actual = folder / (label + '.sqlite3')
                            with sqlite3.connect('file:' + str(path) + '?mode=ro', uri=True) as source, sqlite3.connect(actual) as target: source.backup(target)
                        with actual.open('rb') as source: yield 'local/' + label + '/' + relative, GuardedReader(source,self.root)
                for index, obj in enumerate(objects):
                    progress(f"Archivando S3: {index + 1}/{len(objects)}")
                    path = folder / 'object'
                    if shutil.disk_usage(self.root).free < obj['size'] + 1024**3: raise RuntimeError('Insufficient space for S3 object')
                    self.aws(['s3api', 'get-object', '--bucket', BUCKET, '--key', obj['key'], '--if-match', obj['etag'], str(path)])
                    if path.stat().st_size != obj['size']: raise ValueError('S3 object changed')
                    with path.open('rb') as source: yield 's3/' + obj['key'], GuardedReader(source,self.root)
                    path.unlink()
            key = self.signing_key()
            commit = Path('/opt/bastion/current/BUILD.txt').read_text().splitlines()[0]
            write_archive(archive, streams(), commit, key)
            progress('Guardando clave de recuperación independiente')
            key_id = hashlib.sha256(key).hexdigest()
            recovery_key = 'backups/recovery/' + key_id
            try:
                recovery_head = json.loads(self.aws(['s3api','head-object','--bucket',BUCKET,'--key',recovery_key]).stdout)
                if recovery_head['ContentLength'] != len(key) or recovery_head.get('Metadata',{}).get('sha256') != key_id: raise RuntimeError('Recovery key metadata mismatch')
            except subprocess.CalledProcessError as failure:
                if b'404' not in failure.stderr and b'Not Found' not in failure.stderr: raise
                self.aws(['s3api', 'put-object', '--bucket', BUCKET, '--key', recovery_key, '--body', '/etc/bastion/data-encryption-key', '--server-side-encryption', 'AES256', '--metadata', 'sha256=' + key_id])
            progress('Subiendo ZIP verificado')
            checksum = self.digest(archive)
            self.aws(['s3', 'cp', str(archive), 's3://' + BUCKET + '/backups/' + name, '--sse', 'AES256', '--checksum-algorithm', 'SHA256', '--metadata', 'sha256=' + checksum, '--only-show-errors'])
            head = json.loads(self.aws(['s3api','head-object','--bucket',BUCKET,'--key','backups/' + name]).stdout)
            if head['ContentLength'] != archive.stat().st_size or head.get('Metadata',{}).get('sha256') != checksum: raise RuntimeError('Uploaded archive metadata mismatch')
            self.cache_time = 0
            progress('Respaldo integral almacenado')
            return 'backups/' + name

    def restore(self, path, progress, fresh=False):
        manifest, plan = self.inspect(path, progress, details=True)
        with self.maintenance(restart_on_error=False):
            progress('Creando punto de recuperación previo')
            if fresh:
                with self.mysql_options() as options:
                    count = subprocess.run(['mysql', *options, '--batch', '--skip-column-names', self.cfg['MYSQL_DATABASE'], '-e', 'SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=DATABASE()'], capture_output=True,check=True,timeout=120).stdout.strip()
                if count != b'0': raise ValueError('Fresh recovery requires an empty database')
            else:
                self.snapshot(progress)
            progress('Restaurando MySQL')
            self.mutating = True
            state_path = self.root / 'maintenance.json'
            state = json.loads(state_path.read_text()); state['restart_on_error'] = False
            state_path.write_text(json.dumps(state))
            with zipfile.ZipFile(path) as archive, self.mysql_options() as options:
                with archive.open('databases/main.sql') as source:
                    with subprocess.Popen(['mysql', *options, '--binary-mode=1', self.cfg['MYSQL_DATABASE']], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL) as process:
                        import threading
                        watchdog = threading.Timer(7200, process.kill)
                        watchdog.daemon = True; watchdog.start()
                        try:
                            shutil.copyfileobj(source, process.stdin, 1024 * 1024)
                        finally:
                            process.stdin.close()
                            watchdog.cancel()
                        if process.wait(timeout=7200): raise RuntimeError('Database restoration failed')
                subprocess.run(['mysql', *options, '--binary-mode=1', self.cfg['MYSQL_DATABASE'], '-e', 'UPDATE sessions SET revoked_at=UTC_TIMESTAMP(6) WHERE revoked_at IS NULL; UPDATE trusted_devices SET revoked_at=UTC_TIMESTAMP(6) WHERE revoked_at IS NULL;'], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=120)
                for item in plan['local_files']:
                    progress('Restaurando archivos locales')
                    root = LOCAL_ROOTS[item['namespace']]
                    if not root.exists():
                        import grp
                        import pwd
                        root.mkdir(mode=0o750,parents=True)
                        owner = pwd.getpwnam('ubuntu').pw_uid if item['namespace'].startswith('scout') else 0
                        os.chown(root, owner, grp.getgrnam('bastion').gr_gid)
                    target = root / item['relative']
                    target.parent.mkdir(parents=True, exist_ok=True)
                    if not target.resolve().is_relative_to(root.resolve()) or target.is_symlink(): raise ValueError('Unsafe local path')
                    stat = target.stat() if target.exists() else root.stat()
                    with tempfile.NamedTemporaryFile(dir=target.parent, delete=False) as output:
                        temporary = Path(output.name)
                        try:
                            with archive.open('local/' + item['namespace'] + '/' + item['relative']) as source: shutil.copyfileobj(source,output,1024*1024)
                            output.flush(); os.fsync(output.fileno())
                            os.chown(temporary, stat.st_uid, stat.st_gid)
                            os.chmod(temporary, 0o640)
                            os.replace(temporary,target)
                        finally: temporary.unlink(missing_ok=True)
                key = self.signing_key(manifest)
                key_path = Path('/etc/bastion/data-encryption-key')
                with tempfile.NamedTemporaryFile(dir=key_path.parent, delete=False) as output:
                    temporary = Path(output.name)
                    try:
                        output.write(key); output.flush(); os.fsync(output.fileno())
                        os.chmod(temporary, 0o600); os.replace(temporary,key_path)
                    finally: temporary.unlink(missing_ok=True)
                desired = {x['key'] for x in plan['s3_objects']}
                with tempfile.TemporaryDirectory(dir=self.root, prefix='transient-') as tmp:
                    for item in plan['s3_objects']:
                        progress('Restaurando documentos S3')
                        target = Path(tmp) / 'object'
                        with archive.open('s3/' + item['key']) as source, target.open('wb') as output: shutil.copyfileobj(source,output,1024*1024)
                        self.aws(['s3', 'cp', str(target), 's3://' + BUCKET + '/' + item['key'], '--sse', 'AES256', '--only-show-errors'])
                        target.unlink()
                for obj in list(inventory(self.aws_env)):
                    if obj['key'] not in desired:
                        self.aws(['s3api','delete-object','--bucket',BUCKET,'--key',obj['key']])
            progress('Restauración terminada; se requiere iniciar sesión de nuevo')
