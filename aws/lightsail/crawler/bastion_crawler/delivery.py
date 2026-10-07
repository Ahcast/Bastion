import json
import os
import ssl
import time
from ftplib import FTP_TLS
from pathlib import Path
from urllib.parse import urlsplit

import requests
from .core import validate, digest, dumps, now


def api_send(path, manifest):
    base=os.environ['BASTION_API_URL'].rstrip('/')
    if urlsplit(base).scheme!='https': raise ValueError('API must use HTTPS')
    token=os.environ['BASTION_TOKEN']
    payload=json.loads((path/'processes.json').read_text())
    info=json.loads((path/'run.json').read_text())
    run=manifest['run_id']; batch=manifest['batch_id']
    with requests.Session() as s:
        s.headers.update({'Authorization':'Bearer '+token,'Accept':'application/json'})
        def post(endpoint,body,key):
            r=s.post(base+endpoint,json=body,headers={'Idempotency-Key':key},timeout=(10,90),allow_redirects=False)
            if r.status_code not in (200,201):
                # No response body or headers in logs: may contain sensitive data.
                raise RuntimeError('API did not confirm import: HTTP '+str(r.status_code))
            return r.json()
        post('/runs',{'run_id':run,'started_at':info['started'],'source':'honducompras'},run)
        total={'received':0,'new':0,'updated':0,'unchanged':0,'errors':0}
        for i in range(0,len(payload['processes']),100):
            chunk=payload['processes'][i:i+100]
            key=batch+'-'+str(i//100)
            ack=post('/runs/'+run+'/processes',{'batch_id':key,'schema_version':'1.0','processes':chunk},key)
            if ack.get('batch_id')!=key or ack.get('status')!='IMPORTED':
                raise ValueError('ACK must confirm committed import for exact batch')
            for k in total:
                if type(ack.get(k)) is not int or ack[k]<0: raise ValueError('Invalid ACK counters')
            if ack['received']!=len(chunk) or ack['errors'] or sum(ack[k] for k in ('new','updated','unchanged'))!=len(chunk):
                raise ValueError('Incomplete import ACK')
            for k in total: total[k]+=ack[k]
        result=post('/runs/'+run+'/complete',{'batch_id':batch,'extraction':info,'import':total},batch+'-complete')
        if result.get('batch_id')!=batch or result.get('status')!='COMPLETED':
            raise ValueError('Run completion not acknowledged')
        return total


def ftps_send(path,manifest):
    # Versioned inbox, NOT the legacy production filename. No .htaccess mutation.
    host=os.environ['BLUEHOST_FTP_HOST']
    remote=os.environ.get('BLUEHOST_FTP_REMOTE_DIR','/private_csv')
    name=manifest['batch_id']+'.csv'
    with FTP_TLS(context=ssl.create_default_context(),timeout=60) as ftp:
        ftp.connect(host,int(os.environ.get('BLUEHOST_FTP_PORT','21')))
        ftp.login(os.environ['BLUEHOST_FTP_USER'],os.environ['BLUEHOST_FTP_PASSWORD'])
        ftp.prot_p(); ftp.set_pasv(True); ftp.cwd(remote)
        for local,final in ((path/'processes.csv',name),(path/'processes.json',manifest['batch_id']+'.json')):
            with local.open('rb') as f: ftp.storbinary('STOR '+final+'.tmp',f)
            h=__import__('hashlib').sha256()
            ftp.retrbinary('RETR '+final+'.tmp',h.update)
            if h.hexdigest()!=digest(local.read_bytes()): raise ValueError('FTPS checksum mismatch')
            ftp.rename(final+'.tmp',final)
        # A transfer is not an import. Read an ACK created by the Bluehost importer.
        from io import BytesIO
        out=BytesIO()
        try: ftp.retrbinary('RETR '+manifest['batch_id']+'.ack.json',out.write)
        except Exception: return {'status':'WAITING_ACK','batch_id':manifest['batch_id']}
        ack=json.loads(out.getvalue())
        records=len(json.loads((path/'processes.json').read_text())['processes'])
        if ack.get('batch_id')!=manifest['batch_id'] or ack.get('status')!='IMPORTED' or ack.get('errors')!=0 or ack.get('received')!=records:
            raise ValueError('Invalid FTPS import ACK')
        return ack


def s3_send(path,manifest,config):
    import boto3
    from boto3.s3.transfer import TransferConfig
    from botocore.config import Config
    client=boto3.client('s3',region_name=os.getenv('AWS_DEFAULT_REGION','us-east-1'),config=Config(retries={'max_attempts':3,'mode':'standard'},connect_timeout=10,read_timeout=90))
    prefix='raw/'+manifest['run_id']+'/'+manifest['batch_id']+'/'
    files=list(path.iterdir())
    raw=config.root/'runs'/manifest['run_id']/'raw'
    for name,sha in manifest.get('raw_files',{}).items():
        if Path(name).name!=name: raise ValueError('Invalid raw path')
        file=raw/name
        if digest(file.read_bytes())!=sha: raise ValueError('Raw evidence missing or corrupt')
        files.append(file)
    for file in sorted(files,key=lambda p:p.name=='manifest.json'):
        if not file.is_file(): continue
        rel=('raw/' if file.parent==raw else '')+file.name
        sha=digest(file.read_bytes())
        key=prefix+rel
        client.upload_file(str(file),config.bucket,key,
            ExtraArgs={'ServerSideEncryption':'AES256','Metadata':{'sha256':sha},'ChecksumAlgorithm':'SHA256'},
            Config=TransferConfig(use_threads=False,multipart_threshold=64*1024*1024))
        head=client.head_object(Bucket=config.bucket,Key=key)
        if head['ContentLength']!=file.stat().st_size or head.get('Metadata',{}).get('sha256')!=sha:
            raise ValueError('S3 verification failed')
    return {'bucket':config.bucket,'prefix':prefix,'status':'STORED'}


def deliver(engine,run=None):
    sql="SELECT * FROM deliveries WHERE status!='DONE' AND next_at<=?"
    args=[time.time()]
    if run: sql+=' AND run=?'; args.append(run)
    for row in engine.s.db.execute(sql,args).fetchall():
        target=row['target']
        if target=='s3' and os.getenv('S3_ENABLED','false').lower()!='true': continue
        mode=os.getenv('BLUEHOST_MODE','disabled')
        if target=='bluehost' and mode=='disabled': continue
        try:
            path=Path(row['path']); manifest=validate(path)
            if target=='s3': ack=s3_send(path,manifest,engine.c)
            elif mode=='api': ack=api_send(path,manifest)
            elif mode=='ftps': ack=ftps_send(path,manifest)
            else: raise ValueError('Unknown BLUEHOST_MODE')
            status='WAITING_ACK' if ack.get('status')=='WAITING_ACK' else 'DONE'
            with engine.s.db:
                engine.s.db.execute('UPDATE deliveries SET status=?,ack=?,next_at=? WHERE batch=? AND target=?',
                    (status,dumps(ack),time.time()+900 if status!='DONE' else 0,row['batch'],target))
        except Exception as exc:
            with engine.s.db:
                engine.s.db.execute("UPDATE deliveries SET status='RETRY_WAIT',attempts=attempts+1,next_at=?,ack=? WHERE batch=? AND target=?",
                    (time.time()+min(21600,900*2**min(row['attempts'],5)),dumps({'error_type':type(exc).__name__}),row['batch'],target))
            print(dumps({'timestamp':now(),'component':'delivery','target':target,'batch_id':row['batch'],'error_type':type(exc).__name__}),flush=True)
