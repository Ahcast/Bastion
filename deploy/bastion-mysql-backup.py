#!/usr/bin/env python3
"""Stream a MySQL backup to S3; upload a manifest only on success."""
import datetime
import hashlib
import json
import os
import re
import subprocess
import tempfile
import uuid

credentials = os.environ['CREDENTIALS_DIRECTORY']
database = os.environ['MYSQL_DATABASE']
if not re.fullmatch(r'[A-Za-z0-9_]+', database):
    raise SystemExit('Invalid database name')
password = open(os.path.join(credentials, 'mysql-password'), encoding='utf-8').read().rstrip('\r\n')
def quote(value):
    return '"' + value.replace('\\', '\\\\').replace('"', '\\"') + '"'
aws_env = {'PATH': '/usr/local/bin:/usr/bin:/bin', 'AWS_SHARED_CREDENTIALS_FILE': os.path.join(credentials, 'aws-s3-credentials'), 'AWS_PROFILE': 'bastion-documents', 'AWS_EC2_METADATA_DISABLED': 'true', 'AWS_PAGER': ''}
run_id = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ') + '-' + str(uuid.uuid4())
key = 'backups/mysql/' + run_id + '/database.sql.gz'
uri = 's3://bastion-prod-assets/' + key
with tempfile.TemporaryDirectory(prefix='bastion-backup-', dir='/dev/shm') as directory:
    os.chmod(directory, 0o700)
    cfg = os.path.join(directory, 'mysql.cnf')
    with open(cfg, 'w', encoding='utf-8') as f:
        os.chmod(cfg, 0o600)
        f.write('[client]\n')
        for k, v in [('host', os.environ['MYSQL_HOST']), ('port', os.environ.get('MYSQL_PORT', '3306')), ('user', os.environ['MYSQL_USER']), ('password', password)]:
            f.write(k + '=' + quote(v) + '\n')
    with tempfile.TemporaryFile(mode='w+') as errors:
        dump = subprocess.Popen(['mysqldump', '--defaults-extra-file=' + cfg, '--ssl-mode=VERIFY_IDENTITY', '--ssl-ca=' + os.path.join(credentials, 'mysql-ca.pem'), '--column-statistics=0', '--set-gtid-purged=OFF', '--single-transaction', '--quick', '--skip-lock-tables', '--hex-blob', '--routines', '--triggers', '--events', database], stdout=subprocess.PIPE, stderr=errors)
        zipper = subprocess.Popen(['gzip', '-c'], stdin=dump.stdout, stdout=subprocess.PIPE, stderr=errors)
        dump.stdout.close()
        upload = subprocess.Popen(['aws', 's3', 'cp', '-', uri, '--sse', 'AES256', '--only-show-errors', '--region', 'us-east-1'], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=errors, env=aws_env)
        digest = hashlib.sha256()
        size = 0
        try:
            while chunk := zipper.stdout.read(1024 * 1024):
                digest.update(chunk)
                size += len(chunk)
                upload.stdin.write(chunk)
        except BrokenPipeError:
            pass
        finally:
            try:
                upload.stdin.close()
            except BrokenPipeError:
                pass
            zipper.stdout.close()
        statuses = [dump.wait(), zipper.wait(), upload.wait()]
        if statuses != [0, 0, 0]:
            errors.seek(0)
            print('Backup failed; no manifest written. Exit codes:', statuses)
            print(errors.read()[-3000:])
            raise SystemExit(1)
    metadata = {'format': 'bastion-mysql-v1', 'created_utc': run_id[:16], 'object': key, 'sha256': digest.hexdigest(), 'bytes': size}
    marker = subprocess.run(['aws', 's3', 'cp', '-', 's3://bastion-prod-assets/backups/mysql/' + run_id + '/manifest.json', '--sse', 'AES256', '--content-type', 'application/json', '--only-show-errors', '--region', 'us-east-1'], input=json.dumps(metadata).encode(), env=aws_env, capture_output=True)
    if marker.returncode:
        raise SystemExit('Dump uploaded but manifest failed; backup is incomplete.')
    print(json.dumps(metadata))

