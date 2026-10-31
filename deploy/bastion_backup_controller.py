"""Private persistent controller for data backup and recovery jobs."""
import argparse
import base64
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import socket
import socketserver
import sqlite3
import threading
import uuid

MAX_UPLOAD = 40 * 1024**3
TOKEN = re.compile(r'[a-f0-9]{32}')


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


class Controller:
    def __init__(self, root, engine):
        self.root, self.engine = root, engine
        root.mkdir(mode=0o700, parents=True, exist_ok=True)
        self.lock = threading.RLock()
        self.db = sqlite3.connect(root / 'jobs.sqlite3', check_same_thread=False)
        self.db.execute('CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY, actor INTEGER, action TEXT, status TEXT, progress TEXT, error TEXT, started TEXT, finished TEXT)')
        self.db.execute('CREATE TABLE IF NOT EXISTS uploads(token TEXT PRIMARY KEY, actor INTEGER, size INTEGER, received INTEGER, created TEXT)')
        self.db.execute('CREATE TABLE IF NOT EXISTS prepared(token TEXT PRIMARY KEY, actor INTEGER, path TEXT, digest TEXT, summary TEXT, created TEXT)')
        self.db.execute('CREATE TABLE IF NOT EXISTS audit(id INTEGER PRIMARY KEY, actor INTEGER, action TEXT, created TEXT)')
        self.db.execute("UPDATE jobs SET status='interrupted',error='Worker restarted; inspect maintenance state',finished=? WHERE status='running'", (now(),))
        self.db.commit()
        self.busy = False

    def status(self, actor):
        row = self.db.execute('SELECT id,status,progress,error FROM jobs ORDER BY rowid DESC LIMIT 1').fetchone()
        prepared = self.db.execute('SELECT token,summary,created FROM prepared WHERE actor=? ORDER BY rowid DESC LIMIT 1', (actor,)).fetchone()
        try: history, history_error = self.engine.history(), None
        except Exception: history, history_error = [], 'No se pudo consultar S3. El estado del trabajo sigue disponible.'
        value = {'available': True, 'busy': self.busy, 'job': dict(zip(('id','status','progress','error'), row)) if row else None, 'history': history, 'history_error': history_error, 'next_run': self.engine.next_run()}
        value['jobs'] = [dict(zip(('id','actor','action','status','progress','error','started','finished'), item)) for item in self.db.execute('SELECT id,actor,action,status,progress,error,started,finished FROM jobs ORDER BY rowid DESC LIMIT 20').fetchall()]
        if prepared:
            expires = datetime.datetime.fromisoformat(prepared[2]) + datetime.timedelta(hours=1)
            if expires > datetime.datetime.now(datetime.timezone.utc):
                value['prepared'] = {'token': prepared[0], 'summary': prepared[1], 'expires_at': expires.isoformat()}
        return value

    def launch(self, actor, action, work):
        if self.busy or self.db.execute('SELECT COUNT(*) FROM uploads').fetchone()[0]:
            return {'error': 'busy'}
        self.busy = True
        job = uuid.uuid4().hex
        self.db.execute('INSERT INTO jobs VALUES(?,?,?,?,?,?,?,?)', (job,actor,action,'running','Iniciando',None,now(),None))
        self.db.commit()
        def progress(text):
            with self.lock:
                self.db.execute('UPDATE jobs SET progress=? WHERE id=?', (text,job)); self.db.commit()
        def run():
            status, error = 'failed', 'Interrupted operation'
            try:
                work(progress)
                status, error = 'succeeded', None
            except Exception as failure:
                # Store only a classified failure; never send command output or secrets.
                status, error = 'failed', str(failure) if isinstance(failure, (ValueError, RuntimeError)) else type(failure).__name__
            finally:
                with self.lock:
                    self.db.execute('UPDATE jobs SET status=?,error=?,finished=? WHERE id=?', (status,error,now(),job))
                    self.db.commit(); self.busy = False
        threading.Thread(target=run, daemon=False).start()
        return {'accepted': True, 'job_id': job}

    def prepare(self, actor, path, progress):
        with self.lock:
            for old_path, in self.db.execute('SELECT path FROM prepared WHERE actor=?', (actor,)).fetchall():
                candidate = Path(old_path)
                if candidate != path and candidate.parent == self.root: candidate.unlink(missing_ok=True)
            self.db.execute('DELETE FROM prepared WHERE actor=?', (actor,)); self.db.commit()
        try:
            summary = self.engine.inspect(path, progress)
            digest = self.engine.digest(path)
        except BaseException:
            path.unlink(missing_ok=True)
            raise
        with self.lock:
            self.db.execute('INSERT INTO prepared VALUES(?,?,?,?,?,?)', (uuid.uuid4().hex, actor, str(path), digest, summary, now()))
            self.db.commit()

    def dispatch(self, request):
        if not isinstance(request, dict) or type(request.get('actor')) is not int:
            return {'error': 'invalid'}
        actor, action = request['actor'], request.get('action')
        with self.lock:
            self.cleanup()
            if action not in ('status','job_status'):
                self.db.execute('INSERT INTO audit(actor,action,created) VALUES(?,?,?)', (actor,str(action)[:64],now())); self.db.commit()
            if action == 'status':
                return self.status(actor)
            if action == 'job_status':
                row = self.db.execute('SELECT id,status,progress,error FROM jobs WHERE id=? AND (actor=? OR ?=0)',(request.get('id'),actor,actor)).fetchone()
                if not row: return {'error':'invalid'}
                return {'job':dict(zip(('id','status','progress','error'),row))}
            if action == 'backup':
                return self.launch(actor, action, self.engine.backup)
            if action == 'download':
                return {'url': self.engine.download_url(request.get('key'))}
            if action == 'prepare_s3':
                key = self.engine.valid_key(request.get('key'))
                def prepare(progress):
                    with self.lock:
                        for previous, in self.db.execute('SELECT path FROM prepared WHERE actor=?',(actor,)).fetchall():
                            candidate = Path(previous)
                            if candidate.parent == self.root: candidate.unlink(missing_ok=True)
                        self.db.execute('DELETE FROM prepared WHERE actor=?',(actor,)); self.db.commit()
                    path = self.root / (uuid.uuid4().hex + '.zip')
                    try:
                        self.engine.retrieve(key, path)
                        self.prepare(actor, path, progress)
                    except BaseException:
                        path.unlink(missing_ok=True)
                        raise
                return self.launch(actor, action, prepare)
            if action == 'upload_begin':
                size = request.get('size')
                if self.busy or type(size) is not int or not 0 < size <= MAX_UPLOAD or shutil.disk_usage(self.root).free < size * 2 + 1024**3:
                    return {'error': 'invalid'}
                if self.db.execute('SELECT count(*) FROM uploads').fetchone()[0] >= 1:
                    return {'error': 'busy'}
                token = uuid.uuid4().hex
                with (self.root / (token + '.partial')).open('xb') as f:
                    os.chmod(f.name, 0o600)
                self.db.execute('INSERT INTO uploads VALUES(?,?,?,?,?)', (token,actor,size,0,now())); self.db.commit()
                return {'token': token}
            token = request.get('token', '')
            if not isinstance(token, str) or not TOKEN.fullmatch(token):
                return {'error': 'invalid'}
            if action == 'upload_abort' and not self.busy:
                row = self.db.execute('SELECT token FROM uploads WHERE token=? AND actor=?',(token,actor)).fetchone()
                if not row: return {'error':'invalid'}
                (self.root / (token + '.partial')).unlink(missing_ok=True)
                self.db.execute('DELETE FROM uploads WHERE token=?',(token,)); self.db.commit()
                return {'discarded':True}
            if action == 'discard' and not self.busy:
                row = self.db.execute('SELECT path FROM prepared WHERE token=? AND actor=?', (token,actor)).fetchone()
                if not row: return {'error': 'invalid'}
                candidate = Path(row[0])
                if candidate.parent != self.root: return {'error': 'invalid'}
                candidate.unlink(missing_ok=True)
                self.db.execute('DELETE FROM prepared WHERE token=?', (token,)); self.db.commit()
                return {'discarded': True}
            if action in ('upload_chunk', 'upload_finish'):
                row = self.db.execute('SELECT size,received,created FROM uploads WHERE token=? AND actor=?', (token,actor)).fetchone()
                if not row or datetime.datetime.fromisoformat(row[2]) + datetime.timedelta(hours=4) < datetime.datetime.now(datetime.timezone.utc):
                    return {'error': 'invalid'}
                path = self.root / (token + '.partial')
                if action == 'upload_chunk':
                    if request.get('offset') != row[1] or not isinstance(request.get('data'), str) or len(request['data']) > 524288:
                        return {'error': 'invalid'}
                    chunk = base64.b64decode(request['data'], validate=True)
                    if not chunk or row[1] + len(chunk) > row[0]:
                        return {'error': 'invalid'}
                    with path.open('ab') as f: f.write(chunk)
                    self.db.execute('UPDATE uploads SET received=received+? WHERE token=?', (len(chunk), token)); self.db.commit()
                    return {'received': row[1] + len(chunk)}
                if row[0] != row[1] or self.busy:
                    return {'error': 'invalid'}
                self.db.execute('DELETE FROM uploads WHERE token=?', (token,)); self.db.commit()
                return self.launch(actor, action, lambda progress: self.prepare(actor,path,progress))
            if action == 'restore' and request.get('confirmed') is True:
                row = self.db.execute('SELECT path,digest,created FROM prepared WHERE token=? AND actor=?', (token,actor)).fetchone()
                if not row or datetime.datetime.fromisoformat(row[2]) + datetime.timedelta(hours=1) < datetime.datetime.now(datetime.timezone.utc):
                    return {'error': 'invalid'}
                if self.busy: return {'error': 'busy'}
                path = Path(row[0])
                def restore(progress):
                    try:
                        if self.engine.digest(path) != row[1]: raise ValueError('Prepared archive changed')
                        self.engine.restore(path, progress)
                    finally: path.unlink(missing_ok=True)
                self.db.execute('DELETE FROM prepared WHERE token=?', (token,)); self.db.commit()
                return self.launch(actor, action, restore)
            return {'error': 'invalid'}

    def cleanup(self):
        cutoff = (datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(hours=4)).isoformat()
        for token, in self.db.execute('SELECT token FROM uploads WHERE created < ?', (cutoff,)).fetchall():
            (self.root / (token + '.partial')).unlink(missing_ok=True)
        self.db.execute('DELETE FROM uploads WHERE created < ?', (cutoff,))
        cutoff = (datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(hours=1)).isoformat()
        for path, in self.db.execute('SELECT path FROM prepared WHERE created < ?', (cutoff,)).fetchall():
            candidate = Path(path)
            if candidate.parent == self.root: candidate.unlink(missing_ok=True)
        self.db.execute('DELETE FROM prepared WHERE created < ?', (cutoff,)); self.db.commit()


class Server(socketserver.UnixStreamServer):
    def __init__(self, path, controller):
        self.controller = controller
        super().__init__(path, Handler)

    def service_actions(self):
        import time
        if time.monotonic() - getattr(self, 'last_cleanup', 0) > 600:
            with self.controller.lock: self.controller.cleanup()
            self.last_cleanup = time.monotonic()


class Handler(socketserver.StreamRequestHandler):
    def handle(self):
        self.connection.settimeout(10)
        try:
            line = self.rfile.readline(1024 * 1024)
            if not line.endswith(b'\n'): raise ValueError('Invalid frame')
            result = self.server.controller.dispatch(json.loads(line))
        except ValueError:
            result = {'error': 'invalid'}
        except Exception:
            result = {'error': 'unavailable'}
        self.wfile.write(json.dumps(result).encode() + b'\n')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--request', choices=['backup'])
    parser.add_argument('--recover', type=Path)
    parser.add_argument('--confirm')
    parser.add_argument('--fresh', action='store_true')
    args = parser.parse_args()
    address = '/run/bastion-backup/control.sock'
    if args.request:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
            connection.settimeout(15); connection.connect(address)
            connection.sendall(json.dumps({'action': args.request, 'actor': 0}).encode() + b'\n')
            response = json.loads(connection.makefile('rb').readline(1024*1024))
            if not response.get('accepted'): raise SystemExit('Backup was not accepted')
            job_id = response['job_id']
        import time
        for _ in range(360):
            time.sleep(60)
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
                connection.settimeout(15); connection.connect(address)
                connection.sendall(json.dumps({'action':'job_status','actor':0,'id':job_id}).encode()+b'\n')
                status = json.loads(connection.makefile('rb').readline(1024*1024))
            job = status.get('job') or {}
            if job.get('id') != job_id: raise SystemExit('Job status no longer available')
            if job.get('status') != 'running':
                if job.get('status') != 'succeeded': raise SystemExit('Backup failed; inspect controller status')
                return
        raise SystemExit('Backup timeout; inspect controller status')
    from bastion_backup_engine import Engine
    import grp
    root = Path('/var/lib/bastion-backup')
    root.mkdir(mode=0o700, parents=True, exist_ok=True)
    import fcntl
    process_lock = (root / 'controller.lock').open('a+b')
    fcntl.flock(process_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    for pattern in ('snapshot-*', 'transient-*'):
        for temporary in root.glob(pattern):
            if temporary.parent == root and temporary.is_dir() and not temporary.is_symlink(): shutil.rmtree(temporary)
    engine = Engine(root)
    if args.recover:
        if os.geteuid() != 0 or args.confirm != 'RESTAURAR': raise SystemExit('Root and --confirm RESTAURAR are required')
        engine.restore(args.recover.resolve(strict=True), print, fresh=args.fresh)
        return
    controller = Controller(root, engine)
    previous = root / 'maintenance.json'
    if previous.exists():
        state = json.loads(previous.read_text())
        if state.get('restart_on_error'):
            import subprocess
            for service in reversed(state['services']):
                if service not in ('bastion.service', 'bastion-scout-control.service', 'bastion-tender-launcher.socket', 'bastion-tender-recover.timer', 'bastion-tender-recover.service'): raise ValueError('Invalid service state')
                subprocess.run(['systemctl','start',service], check=True,timeout=120)
            previous.unlink()
    controller.cleanup()
    path = Path(address)
    if path.exists():
        if not path.is_socket(): raise ValueError('Invalid socket path')
        path.unlink()
    with Server(address, controller) as server:
        os.chown(address, 0, grp.getgrnam('bastion').gr_gid)
        os.chmod(address, 0o660)
        server.serve_forever(poll_interval=1)


if __name__ == '__main__':
    main()
