import csv
import fcntl
import hashlib
import json
import os
import random
import re
import sqlite3
import shutil
import time
import uuid
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import urljoin, urlsplit, urlunsplit, parse_qsl, urlencode

import requests
from bs4 import BeautifulSoup
from .legacy import extraer_formulario, texto_limpio, convertir_fecha

SOURCE = 'honducompras'
TABLE = 'ctl00_cphCuerpo_gvResultados'
FIELDS = ['Entidad', 'Expediente', 'Fecha Recepción Ofertas', 'Especificaciones']

def now():
    return datetime.now(timezone.utc).isoformat()

def dumps(obj):
    return json.dumps(obj, ensure_ascii=False, sort_keys=True, separators=(',', ':'))

def digest(data):
    return hashlib.sha256(data).hexdigest()

def normalize(text):
    return re.sub(r'\s+', ' ', text or '').strip()

def canonical(url):
    p = urlsplit(url)
    return urlunsplit((p.scheme.lower(), p.netloc.lower(), p.path,
                      urlencode(sorted(parse_qsl(p.query, keep_blank_values=True))), ''))

def atomic(path, data):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(path.suffix + '.tmp')
    with temp.open('wb') as f:
        f.write(data)
        f.flush()
        os.fsync(f.fileno())
    temp.replace(path)

class Config:
    def __init__(self):
        self.root = Path(os.getenv('CRAWLER_DATA', './data')).resolve()
        self.url = os.getenv('SOURCE_URL', 'http://sicc.honducompras.gob.hn/HC/Procesos/BusquedaHistorico.aspx')
        self.delay = max(2, float(os.getenv('REQUEST_DELAY', '3')))
        self.connect = float(os.getenv('CONNECT_TIMEOUT', '10'))
        self.read = float(os.getenv('READ_TIMEOUT', '90'))
        self.attempts = max(1, int(os.getenv('HTTP_ATTEMPTS', '3')))
        self.max_pages = int(os.getenv('MAX_PAGES', '500'))
        self.max_seconds = int(os.getenv('MAX_RUN_SECONDS', '14400'))
        self.max_bytes = int(os.getenv('MAX_RESPONSE_BYTES', '10485760'))
        self.bucket = os.getenv('S3_BUCKET', 'castilloit-honducompras-crawler')

class Store:
    def __init__(self, root):
        self.root = Path(root)
        self.root.mkdir(parents=True, exist_ok=True)
        self.db = sqlite3.connect(self.root / 'state.sqlite3')
        self.db.row_factory = sqlite3.Row
        self.db.executescript('''
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=FULL;
        CREATE TABLE IF NOT EXISTS runs(id TEXT PRIMARY KEY, started TEXT, finished TEXT,
          status TEXT, listing_complete INTEGER DEFAULT 0);
        CREATE TABLE IF NOT EXISTS tasks(run TEXT, kind TEXT, key TEXT, status TEXT DEFAULT 'PENDING',
          attempts INTEGER DEFAULT 0, next_at REAL DEFAULT 0, error TEXT,
          PRIMARY KEY(run,kind,key));
        CREATE TABLE IF NOT EXISTS observations(run TEXT, id TEXT, payload TEXT,
          PRIMARY KEY(run,id));
        CREATE TABLE IF NOT EXISTS downloads(run TEXT,url TEXT,sha TEXT,PRIMARY KEY(run,url));
        CREATE TABLE IF NOT EXISTS errors(id INTEGER PRIMARY KEY, run TEXT, at TEXT, kind TEXT, unit TEXT, error TEXT);
        CREATE TABLE IF NOT EXISTS deliveries(batch TEXT, run TEXT, target TEXT, path TEXT,
          status TEXT DEFAULT 'PENDING', attempts INTEGER DEFAULT 0, next_at REAL DEFAULT 0,
          ack TEXT, PRIMARY KEY(batch,target));
        ''')
    def create(self):
        run = 'HC-' + datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ-') + uuid.uuid4().hex[:8]
        with self.db:
            self.db.execute('INSERT INTO runs(id,started,status) VALUES(?,?,?)', (run, now(), 'RUNNING'))
        return run
    def add(self, run, kind, key):
        self.db.execute('INSERT OR IGNORE INTO tasks(run,kind,key) VALUES(?,?,?)', (run,kind,key))
    def error(self, run, kind, key, exc):
        # Never serialize request headers or credentials into logs.
        message = type(exc).__name__ + ': ' + str(exc)[:300]
        with self.db:
            self.add(run,kind,key)
            self.db.execute("UPDATE tasks SET status='RETRY_WAIT',attempts=attempts+1,next_at=?,error=? WHERE run=? AND kind=? AND key=?",
                            (time.time()+900,message,run,kind,key))
            self.db.execute('INSERT INTO errors(run,at,kind,unit,error) VALUES(?,?,?,?,?)',(run,now(),kind,key,message))
        print(dumps(dict(timestamp=now(),run_id=run,severity='ERROR',component=kind,unit=key,error=message)),flush=True)
    def success(self, run, kind, key):
        self.db.execute("UPDATE tasks SET status='SUCCEEDED',error=NULL WHERE run=? AND kind=? AND key=?", (run,kind,key))

@contextmanager
def lock(root):
    Path(root).mkdir(parents=True, exist_ok=True)
    with open(Path(root)/'worker.lock','w') as f:
        fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield

class HTTP:
    def __init__(self, config, run_path):
        self.c = config
        self.path = run_path
        self.session = requests.Session()
        self.session.headers.update({'User-Agent':'Bastion-HonduCompras/1.0', 'Accept-Language':'es-HN,es;q=0.9', 'Connection':'close'})
        self.last = 0
        self.failures = 0
    def get(self, url, data=None):
        if urlsplit(url).hostname != urlsplit(self.c.url).hostname:
            raise ValueError('Source host not allowed')
        if shutil.disk_usage(self.c.root).free < int(os.getenv('MIN_FREE_BYTES','2147483648')):
            raise OSError('Insufficient free disk space; extraction paused')
        for attempt in range(self.c.attempts):
            time.sleep(max(0, self.last+self.c.delay-time.monotonic()))
            try:
                with self.session.request('POST' if data is not None else 'GET', url, data=data,
                        timeout=(self.c.connect,self.c.read), stream=True, allow_redirects=False) as r:
                    if 300 <= r.status_code < 400:
                        raise ValueError('Redirect requires explicit source configuration')
                    r.raise_for_status()
                    chunks = bytearray()
                    for part in r.iter_content(65536):
                        chunks.extend(part)
                        if len(chunks)>self.c.max_bytes:
                            raise ValueError('Response exceeds size limit')
                    body = bytes(chunks)
                    atomic(self.path/'raw'/f'{digest(body)}.bin',body)
                    self.failures=0
                    return body
            except requests.RequestException as exc:
                status = exc.response.status_code if exc.response is not None else None
                if status is not None and status != 429 and status < 500:
                    raise
                if attempt+1 == self.c.attempts:
                    self.failures += 1
                    if self.failures >= 3:
                        time.sleep(60)
                    raise
                wait = 2*(2**attempt) + random.uniform(0,1)
                if exc.response is not None:
                    retry = exc.response.headers.get('Retry-After','')
                    if retry.isdigit():
                        # Do not retry earlier than requested; defer long waits to queue.
                        if int(retry)>300:
                            raise
                        wait=max(wait,int(retry))
                time.sleep(wait)
            finally:
                self.last=time.monotonic()

class HonduCompras:
    def __init__(self,http,c):
        self.http,self.c=http,c
    def soup(self, body):
        return BeautifulSoup(body,'html.parser')
    def search(self):
        soup=self.soup(self.http.get(self.c.url))
        data=extraer_formulario(soup)
        data.update({'ctl00$cphCuerpo$wpParametros$ddlModalidad':'2',
                     'ctl00$cphCuerpo$wpParametros$ddlTipoAdquisicion':'1',
                     'ctl00$cphCuerpo$wpParametros$btnBuscar':'Buscar'})
        return self.listing(self.http.get(self.c.url,data))
    def listing(self,body):
        soup=self.soup(body)
        if soup.find(id=TABLE) is None:
            raise ValueError('Expected results table missing; cannot assume empty search')
        return soup
    def links(self,soup):
        return list(dict.fromkeys(canonical(urljoin(self.c.url,a['href'])) for a in soup.select('a[id*="gvResultados"][id$="hlGoTo"][href]')))
    def next_page(self,soup,page):
        # Only finish when the pager explicitly has no forward numeric/Next/Last link.
        args=re.findall(r'Page\$([0-9]+|Next|Last)',str(soup.find(id=TABLE)), re.I)
        numeric=[int(x) for x in args if x.isdigit()]
        if not any(x>page for x in numeric) and not any(x.lower() in ('next','last') for x in args):
            return None
        data=extraer_formulario(soup)
        data.update({'__EVENTTARGET':'ctl00$cphCuerpo$gvResultados','__EVENTARGUMENT':f'Page${page+1}'})
        return self.listing(self.http.get(self.c.url,data))
    def detail(self,url):
        body=self.http.get(url)
        return parse_detail(body,url)

def parse_detail(body,url):
    soup=BeautifulSoup(body,'html.parser')
    table=soup.find(id='ctl00_cphCuerpo_dvProceso')
    if table is None:
        raise ValueError('Expected process detail missing')
    data={}
    labels={'Entidad':'entity','Expediente':'reference','Fecha Recepcion Ofertas':'deadline','Fecha Recepción Ofertas':'deadline'}
    for row in table.find_all('tr'):
        cells=row.find_all('td',recursive=False)
        if len(cells)!=2: continue
        label=texto_limpio(cells[0])
        for text,field in labels.items():
            if text in label: data[field]=normalize(texto_limpio(cells[1]))
    products=soup.find(id='ctl00_cphCuerpo_WebTab1_tmpl0_wdgProductos')
    if products is not None:
        parts=[]
        for row in products.find_all('tr'):
            cells=row.find_all('td',recursive=False)
            if len(cells)>=3:
                text=normalize(texto_limpio(cells[2]))
                if text and text.lower()!='especificaciones': parts.append(text)
        data['description']=' | '.join(parts)
    if not data.get('reference') and not data.get('entity'):
        raise ValueError('Detail contains no identifying metadata')
    canonical_url=canonical(url)
    # Preserve query identifiers verbatim; URL fallback is stable, not a content hash.
    ids=[(k,v) for k,v in parse_qsl(urlsplit(url).query) if k.lower() in ('id','idproceso','id_proceso')]
    external_id=('official:'+dumps(sorted(ids))) if ids else 'url:'+digest(canonical_url.encode())
    complete=all(data.get(k) for k in ('entity','reference','deadline','description'))
    docs=[]
    for a in soup.select('a[href]'):
        doc=urljoin(url,a['href'])
        if urlsplit(doc).path.lower().endswith(('.pdf','.doc','.docx','.xls','.xlsx','.zip')):
            docs.append({'url':canonical(doc),'name':normalize(a.get_text()) or Path(urlsplit(doc).path).name})
    if 'deadline' in data:
        parsed=convertir_fecha(data['deadline'])
        if parsed is not None:
            from zoneinfo import ZoneInfo
            data['deadline']=parsed.replace(tzinfo=ZoneInfo('America/Tegucigalpa')).isoformat()
    data['documents']=sorted({x['url']:x for x in docs}.values(),key=lambda x:x['url'])
    return {'source':SOURCE,'external_id':external_id,'source_url':canonical_url,'observed_at':now(),
            'fields_present':[k for k in data if k!='documents'], 'data':data,
            'documents_complete':False, # Direct links cannot prove completeness of ASP.NET tabs.
            'completeness':'COMPLETE' if complete else ('PARTIAL' if data.get('description') else 'EMPTY'),
            'content_hash':digest(dumps(data).encode()),'raw_sha256':digest(body)}

class Engine:
    def __init__(self,c):
        self.c=c
        self.s=Store(c.root)
    def run(self,run=None):
        run=run or self.s.create()
        if not self.s.db.execute('SELECT 1 FROM runs WHERE id=?',(run,)).fetchone():
            raise ValueError('Unknown run')
        with self.s.db:
            self.s.db.execute("UPDATE runs SET listing_complete=0,status='RUNNING' WHERE id=?",(run,))
        started=time.monotonic()
        path=self.c.root/'runs'/run
        adapter=HonduCompras(HTTP(self.c,path),self.c)
        try:
            soup=adapter.search()
            page=1
            seen=set()
            while True:
                urls=adapter.links(soup)
                signature=digest(dumps(urls).encode())
                if not urls or signature in seen:
                    raise ValueError('Empty or repeated listing; coverage unconfirmed')
                seen.add(signature)
                with self.s.db:
                    self.s.add(run,'page',str(page))
                    for url in urls: self.s.add(run,'detail',url)
                    self.s.success(run,'page',str(page))
                self.details(run,adapter,started)
                if time.monotonic()-started>self.c.max_seconds or page>=self.c.max_pages:
                    raise TimeoutError('Configured run limit reached; listing remains incomplete')
                next_soup=adapter.next_page(soup,page)
                if next_soup is None:
                    with self.s.db:
                        self.s.db.execute('UPDATE runs SET listing_complete=1 WHERE id=?',(run,))
                    break
                page+=1
                soup=next_soup
        except Exception as exc:
            self.s.error(run,'listing','scan',exc)
        finally:
            try: self.documents(run,adapter,started)
            finally: adapter.http.session.close()
        # A successful traversal resolves earlier listing failures.
        if self.s.db.execute('SELECT listing_complete FROM runs WHERE id=?',(run,)).fetchone()[0]:
            with self.s.db: self.s.success(run,'listing','scan')
        self.finish(run)
        return run
    def details(self,run,adapter,started):
        tasks=self.s.db.execute("SELECT key FROM tasks WHERE run=? AND kind='detail' AND status!='SUCCEEDED' AND next_at<=?",(run,time.time())).fetchall()
        for task in tasks:
            if time.monotonic()-started>self.c.max_seconds: return
            url=task['key']
            try:
                obs=adapter.detail(url)
                with self.s.db:
                    self.s.db.execute('INSERT OR REPLACE INTO observations VALUES(?,?,?)',(run,obs['external_id'],dumps(obs)))
                    self.s.success(run,'detail',url)
                    for doc in obs['data']['documents']:
                        self.s.add(run,'document',doc['url'])
                print(dumps(dict(timestamp=now(),run_id=run,severity='INFO',component='detail',process_id=obs['external_id'])),flush=True)
            except Exception as exc:
                self.s.error(run,'detail',url,exc)
                if adapter.http.failures>=3: break
    def retry(self,run):
        row=self.s.db.execute('SELECT * FROM runs WHERE id=?',(run,)).fetchone()
        if row is None: raise ValueError('Unknown run')
        if not row['listing_complete']: return self.run(run)
        adapter=HonduCompras(HTTP(self.c,self.c.root/'runs'/run),self.c)
        try:
            started=time.monotonic()
            self.details(run,adapter,started)
            self.documents(run,adapter,started)
        finally: adapter.http.session.close()
        self.finish(run)
        return run
    def documents(self,run,adapter,started):
        rows=self.s.db.execute("SELECT key FROM tasks WHERE run=? AND kind='document' AND status!='SUCCEEDED' AND next_at<=?",(run,time.time())).fetchall()
        for row in rows:
            if time.monotonic()-started>self.c.max_seconds: break
            url=row['key']
            try:
                body=adapter.http.get(url)
                # Login/error pages returned with HTTP 200 must not become documents.
                if body.lstrip().lower().startswith((b'<!doctype html',b'<html')):
                    raise ValueError('HTML received instead of document')
                with self.s.db:
                    self.s.db.execute('INSERT OR REPLACE INTO downloads VALUES(?,?,?)',(run,url,digest(body)))
                    self.s.success(run,'document',url)
            except Exception as exc:
                self.s.error(run,'document',url,exc)
                if adapter.http.failures>=3: break

    def finish(self,run):
        rows=self.s.db.execute('SELECT payload FROM observations WHERE run=? ORDER BY id',(run,)).fetchall()
        records=[json.loads(x[0]) for x in rows]
        downloaded={x['url']:x['sha'] for x in self.s.db.execute('SELECT * FROM downloads WHERE run=?',(run,))}
        for obs in records:
            for doc in obs['data']['documents']:
                doc['sha256']=downloaded.get(doc['url'])
                doc['download_status']='DOWNLOADED' if doc['sha256'] else 'PENDING'
            obs['content_hash']=digest(dumps(obs['data']).encode())
        failed=self.s.db.execute("SELECT count(*) FROM tasks WHERE run=? AND status!='SUCCEEDED'",(run,)).fetchone()[0]
        listing=self.s.db.execute('SELECT listing_complete FROM runs WHERE id=?',(run,)).fetchone()[0]
        status=('SUCCESS' if listing and not failed else 'PARTIAL_SUCCESS') if records else 'FAILED'
        with self.s.db:
            self.s.db.execute('UPDATE runs SET status=?,finished=? WHERE id=?',(status,now(),run))
        info=dict(self.s.db.execute('SELECT * FROM runs WHERE id=?',(run,)).fetchone())
        info['run_id']=run
        info['pages_processed']=self.s.db.execute("SELECT count(*) FROM tasks WHERE run=? AND kind='page' AND status='SUCCEEDED'",(run,)).fetchone()[0]
        info['pages_expected']=info['pages_processed'] if listing else None
        info.update(records=len(records),pending_units=failed,coverage=100 if status=='SUCCESS' else None,
                    coverage_note='Unknown total until verified end of pagination',schema_version='1.0')
        # Immutable snapshot per attempt; never alter an already queued batch.
        batch=run+'-'+uuid.uuid4().hex[:12]
        path=self.c.root/'batches'/batch
        atomic(path/'processes.json',dumps({'schema_version':'1.0','run_id':run,'batch_id':batch,'processes':records}).encode())
        atomic(path/'run.json',dumps(info).encode())
        errors=[dict(x) for x in self.s.db.execute('SELECT * FROM errors WHERE run=?',(run,))]
        atomic(path/'errors.json',dumps(errors).encode())
        with (path/'processes.csv').open('w',encoding='utf-8-sig',newline='') as f:
            writer=csv.writer(f); writer.writerow(FIELDS)
            for obs in records:
                d=dict(obs['data'])
                try:
                    dt=datetime.fromisoformat(d['deadline'])
                    d['deadline']=dt.strftime('%d/%m/%Y %I:%M:%S ') + ('a.m.' if dt.hour<12 else 'p.m.')
                except (KeyError,ValueError): pass
                writer.writerow([d.get(k,'') for k in ('entity','reference','deadline','description')])
        files={p.name:digest(p.read_bytes()) for p in path.iterdir()}
        raw_dir=self.c.root/'runs'/run/'raw'
        raw_files={p.name:p.stem for p in raw_dir.glob('*.bin')}
        atomic(path/'manifest.json',dumps({'schema_version':'1.0','run_id':run,'batch_id':batch,'files':files,'raw_files':raw_files}).encode())
        with self.s.db:
            for target in ('s3','bluehost'):
                self.s.db.execute('INSERT INTO deliveries(batch,run,target,path) VALUES(?,?,?,?)',(batch,run,target,str(path)))
        print(dumps(info),flush=True)

def validate(path):
    path=Path(path)
    manifest=json.loads((path/'manifest.json').read_text())
    for name,sha in manifest['files'].items():
        if Path(name).name!=name or digest((path/name).read_bytes())!=sha:
            raise ValueError('Invalid manifest or checksum')
    return manifest
