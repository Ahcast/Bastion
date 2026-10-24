"""Private Unix-socket controller for one fixed Scout pipeline, without a shell."""
import argparse
import datetime
import fcntl
import json
import os
from pathlib import Path
import re
import socketserver
import sqlite3
import subprocess
import threading


def stamp():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


class Controller:
    def __init__(self, python, entry, config, data, state):
        self.command = [str(python), str(entry), '--config', str(config), '--manual']
        self.cwd, self.data, self.state = entry.parent, data, state
        state.mkdir(mode=0o700, parents=True, exist_ok=True)
        self.db = sqlite3.connect(str(state / 'runs.sqlite3'), check_same_thread=False)
        self.db.execute('CREATE TABLE IF NOT EXISTS runs (id TEXT PRIMARY KEY, status TEXT NOT NULL, started_at TEXT NOT NULL, finished_at TEXT)')
        self.db.execute("UPDATE runs SET status='interrupted',finished_at=? WHERE status='running'", (stamp(),))
        self.db.commit()
        self.lock, self.process = threading.RLock(), None

    def external_busy(self):
        with (self.data / 'pipeline.lock').open('a+b') as handle:
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                return True
            fcntl.flock(handle, fcntl.LOCK_UN)
        return False

    def snapshot(self, request_id=None):
        with self.lock:
            row = (self.db.execute('SELECT id,status,started_at,finished_at FROM runs WHERE id=?', (request_id,)).fetchone()
                   if request_id else self.db.execute('SELECT id,status,started_at,finished_at FROM runs ORDER BY rowid DESC LIMIT 1').fetchone())
            job = dict(zip(('id', 'status', 'started_at', 'finished_at'), row)) if row else None
            return {'available': True, 'busy': self.process is not None or self.external_busy(), 'job': job}

    def dispatch(self, request):
        if not isinstance(request, dict):
            return {'error': 'invalid'}
        if request == {'action': 'status'}:
            return self.snapshot()
        if set(request) != {'action', 'id'} or request.get('action') != 'start':
            return {'error': 'invalid'}
        key = request.get('id')
        if not isinstance(key, str) or not re.fullmatch(r'[A-Za-z0-9_-]{16,180}', key):
            return {'error': 'invalid'}
        with self.lock:
            if self.db.execute('SELECT 1 FROM runs WHERE id=?', (key,)).fetchone():
                return self.snapshot(key)
            if self.process is not None or self.external_busy():
                return {'error': 'busy'}
            self.db.execute("INSERT INTO runs VALUES (?,'running',?,NULL)", (key, stamp()))
            self.db.commit()
            try:
                # Output stays private. No request value reaches executable arguments or paths.
                with (self.state / 'pipeline.log').open('ab') as log:
                    self.process = subprocess.Popen(self.command, cwd=self.cwd, stdin=subprocess.DEVNULL,
                                                    stdout=log, stderr=log, close_fds=True)
                threading.Thread(target=self.finish, args=(key, self.process), daemon=True).start()
            except OSError:
                self.db.execute("UPDATE runs SET status='failed',finished_at=? WHERE id=?", (stamp(), key))
                self.db.commit()
                return {'error': 'unavailable'}
            return self.snapshot(key)

    def finish(self, key, process):
        code = process.wait()
        status = 'succeeded' if code == 0 else 'busy' if code == 75 else 'failed'
        # A partial publication is useful but must never be reported as a full success.
        if code == 1:
            try:
                report = json.loads((self.data / 'main.json').read_text())
                started = self.db.execute('SELECT started_at FROM runs WHERE id=?', (key,)).fetchone()[0]
                if (report.get('trigger') == 'manual' and report.get('status') == 'parcial'
                        and datetime.datetime.fromisoformat(report.get('started_at', '')) >= datetime.datetime.fromisoformat(started)):
                    status = 'partial'
            except (OSError, ValueError, TypeError):
                pass
        with self.lock:
            self.db.execute('UPDATE runs SET status=?,finished_at=? WHERE id=?', (status, stamp(), key))
            self.db.commit()
            self.process = None


class Handler(socketserver.StreamRequestHandler):
    def handle(self):
        self.request.settimeout(3)
        try:
            data = self.rfile.readline(1025)
            if len(data) > 1024 or not data.endswith(b'\n'):
                result = {'error': 'invalid'}
            else:
                result = self.server.controller.dispatch(json.loads(data))
        except (OSError, ValueError, TypeError, sqlite3.Error):
            result = {'error': 'unavailable'}
        self.wfile.write(json.dumps(result).encode() + b'\n')


class Server(socketserver.ThreadingUnixStreamServer):
    daemon_threads = True
    request_queue_size = 8

    def __init__(self, *args):
        self.slots = threading.BoundedSemaphore(8)
        super().__init__(*args)

    def process_request(self, request, client_address):
        if not self.slots.acquire(blocking=False):
            request.close()
            return
        try:
            super().process_request(request, client_address)
        except Exception:
            self.slots.release()
            raise

    def process_request_thread(self, request, client_address):
        try:
            super().process_request_thread(request, client_address)
        finally:
            self.slots.release()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('socket', 'python', 'entry', 'config', 'data', 'state'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    assert all(value.is_absolute() for value in vars(args).values())
    os.umask(0o077)
    check = subprocess.run([str(args.python), str(args.entry), '--help'], capture_output=True, timeout=15)
    if check.returncode or b'--manual' not in check.stdout:
        raise RuntimeError('Install the Scout version with --manual before enabling this controller')
    controller = Controller(args.python, args.entry, args.config, args.data, args.state)
    # RuntimeDirectory is private and owned by this service; restart kills its whole cgroup.
    if args.socket.exists():
        assert args.socket.is_socket()
        args.socket.unlink()
    with Server(str(args.socket), Handler) as server:
        server.controller = controller
        args.socket.chmod(0o660)
        server.serve_forever()


if __name__ == '__main__':
    main()
