#!/usr/bin/env python3
"""Test-only S3 transport. No network, AWS accounts or credentials are used."""
from pathlib import Path
import sys,tempfile,shutil,json,os
args=sys.argv[1:]
def option(name): return args[args.index(name)+1]
assert args[0]=='s3api'
assert option('--bucket')=='bastion-synthetic-attachment-test'
key=option('--key')
assert key.startswith('cotizaciones/') and all(part not in ('','..','.') for part in key.split('/'))
base=Path(tempfile.gettempdir())/'bastion-synthetic-attachment-test'
target=base/key
operation=args[1]
if operation=='put-object':
 assert option('--server-side-encryption')=='AES256'
 target.parent.mkdir(parents=True,exist_ok=True,mode=0o700)
 shutil.copyfile(option('--body'),target);target.chmod(0o600)
elif operation=='get-object':
 shutil.copyfile(target,args[-1])
elif operation=='delete-object':
 target.unlink(missing_ok=True)
else:
 raise AssertionError('Unexpected test operation')
print(json.dumps({}))
