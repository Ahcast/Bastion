import argparse
import json
import os
import tempfile
from pathlib import Path
from .core import Config, Engine, lock, dumps, validate
from .delivery import deliver


def main():
    p=argparse.ArgumentParser(description='Crawler Bastion; mensajes y operación en español')
    p.add_argument('--env',default='.env')
    sub=p.add_subparsers(dest='command',required=True)
    r=sub.add_parser('run'); r.add_argument('--dry-run',action='store_true')
    r=sub.add_parser('retry'); r.add_argument('run_id',nargs='?')
    r=sub.add_parser('upload'); r.add_argument('run_id',nargs='?')
    sub.add_parser('status')
    r=sub.add_parser('validate'); r.add_argument('batch_path')
    r=sub.add_parser('replay'); r.add_argument('batch_path')
    a=p.parse_args()
    from dotenv import load_dotenv
    load_dotenv(a.env,override=False)
    c=Config()
    exit_code=0
    if a.command=='validate': print(dumps(validate(a.batch_path))); return
    temp=None
    if getattr(a,'dry_run',False):
        temp=tempfile.TemporaryDirectory(prefix='bastion-dry-'); c.root=Path(temp.name)
    try:
        with lock(c.root):
            engine=Engine(c)
            if a.command=='run':
                run=engine.run()
                if not getattr(a,'dry_run',False): deliver(engine,run)
                state=engine.s.db.execute('SELECT status FROM runs WHERE id=?',(run,)).fetchone()[0]
                exit_code={'SUCCESS':0,'PARTIAL_SUCCESS':2,'FAILED':1}[state]
            elif a.command=='retry':
                ids=[a.run_id] if a.run_id else [x[0] for x in engine.s.db.execute("SELECT r.id FROM runs r WHERE (r.status='RUNNING' OR EXISTS (SELECT 1 FROM tasks t WHERE t.run=r.id AND t.status!='SUCCEEDED' AND t.next_at<=?)) ORDER BY r.started LIMIT 1",(__import__('time').time(),))]
                for run in ids: engine.retry(run)
                deliver(engine)
            elif a.command=='upload': deliver(engine,a.run_id)
            elif a.command=='replay':
                path=Path(a.batch_path).resolve(); m=validate(path)
                # Re-deliver exact immutable batch, preserving all idempotency keys.
                with engine.s.db:
                    for target in ('s3','bluehost'):
                        engine.s.db.execute("INSERT INTO deliveries(batch,run,target,path) VALUES(?,?,?,?) ON CONFLICT(batch,target) DO UPDATE SET status='PENDING',next_at=0",
                           (m['batch_id'],m['run_id'],target,str(path)))
                deliver(engine,m['run_id'])
            else:
                print(dumps({'runs':[dict(x) for x in engine.s.db.execute('SELECT * FROM runs ORDER BY started DESC LIMIT 10')],
                   'pending_tasks':engine.s.db.execute("SELECT count(*) FROM tasks WHERE status!='SUCCEEDED'").fetchone()[0],
                   'pending_deliveries':[dict(x) for x in engine.s.db.execute("SELECT batch,target,status,attempts FROM deliveries WHERE status!='DONE'")]}))
            engine.s.db.close()
    finally:
        if temp: temp.cleanup()
    if exit_code: raise SystemExit(exit_code)

if __name__=='__main__': main()
