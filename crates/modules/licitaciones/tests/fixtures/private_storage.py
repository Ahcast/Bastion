#!/usr/bin/env python3
"""Isolated S3 transport for tender files. No network or AWS credentials."""
from pathlib import Path
import json,shutil,sys,tempfile
args=sys.argv[1:]
if args[:2]==['s3','cp']:
 assert len(args)>=4 and args[2].startswith('s3://bastion-synthetic-tender-files-test/licitaciones/') and args[3]=='-'
 key=args[2].split('bastion-synthetic-tender-files-test/',1)[1]
 assert all(p not in ('','.','..') for p in key.split('/'))
 target=Path(tempfile.gettempdir())/'bastion-synthetic-tender-files-test'/key
 with target.open('rb') as source:shutil.copyfileobj(source,sys.stdout.buffer,256*1024)
 raise SystemExit(0)
def option(name):return args[args.index(name)+1]
assert args[0]=='s3api' and option('--bucket')=='bastion-synthetic-tender-files-test'
operation=args[1]
key=option('--prefix') if operation=='list-object-versions' else option('--key')
assert key.startswith(('licitaciones/','colaboracion/')) and all(p not in ('','.','..') for p in key.split('/'))
base=Path(tempfile.gettempdir())/'bastion-synthetic-tender-files-test'
target=base/key
if operation=='put-object':
 assert option('--server-side-encryption')=='AES256'
 target.parent.mkdir(parents=True,exist_ok=True,mode=0o700)
 shutil.copyfile(option('--body'),target);target.chmod(0o600)
elif operation=='get-object':shutil.copyfile(target,args[-1])
elif operation=='delete-object':
 assert option('--version-id')=='synthetic-version' if '--version-id' in args else any(a=='--version-id=synthetic-version' for a in args)
 target.unlink(missing_ok=True)
elif operation=='list-object-versions':
 paths=[p for p in target.parent.glob(target.name+'*') if p.is_file()] if target.parent.exists() else []
 print(json.dumps({'IsTruncated':False,'Versions':[{'Key':p.relative_to(base).as_posix(),'VersionId':'synthetic-version'} for p in paths]}));raise SystemExit(0)
else:raise AssertionError('Unexpected S3 test operation')
print(json.dumps({}))
