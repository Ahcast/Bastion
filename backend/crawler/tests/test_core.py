import json
import sqlite3
import tempfile
import unittest
from datetime import date
from pathlib import Path
from unittest.mock import patch, MagicMock
from bastion_crawler.core import *
from bastion_crawler.delivery import deliver, api_send

HTML=b'''<table id="ctl00_cphCuerpo_dvProceso"><tr><td>Entidad</td><td> ENEE </td></tr><tr><td>Expediente</td><td>X-1</td></tr></table>'''
class Tests(unittest.TestCase):
    def test_deadline_yesterday_uses_honduras_calendar(self):
        today=date(2025,1,2)
        self.assertTrue(deadline_is_yesterday('2025-01-02T01:00:00+00:00',today))
        self.assertFalse(deadline_is_yesterday('2025-01-02T12:00:00-06:00',today))
        self.assertFalse(deadline_is_yesterday('not a date',today))
        self.assertFalse(deadline_is_yesterday(None,today))

    def test_cutoff_after_three_consecutive_yesterday_deadlines_across_pages(self):
        with tempfile.TemporaryDirectory() as d:
            c=Config(); c.root=Path(d); e=Engine(c)
            adapter=MagicMock(); adapter.http.failures=0
            adapter.search.return_value='page1'
            adapter.links.side_effect=[['u1','u2'],['u3','u4']]
            adapter.next_page.return_value='page2'
            adapter.detail.side_effect=lambda url: {
                'external_id':url,'source_url':url,'data':{'deadline':'2025-01-01T12:00:00-06:00','documents':[]}}
            today=date(2025,1,2)
            with patch('bastion_crawler.core.HonduCompras',return_value=adapter), \
                 patch('bastion_crawler.core.deadline_is_yesterday',side_effect=lambda value: deadline_is_yesterday(value,today)):
                run=e.run()
            self.assertEqual([call.args[0] for call in adapter.detail.call_args_list],['u1','u2','u3'])
            adapter.next_page.assert_called_once_with('page1',1)
            row=e.s.db.execute('SELECT listing_complete,stop_reason,status FROM runs WHERE id=?',(run,)).fetchone()
            self.assertEqual(tuple(row),(1,'three_consecutive_offer_deadlines_yesterday','SUCCESS'))
            batch=next((Path(d)/'batches').iterdir())
            info=json.loads((batch/'run.json').read_text())
            self.assertIsNone(info['coverage'])
            self.assertIn('three consecutive',info['coverage_note'])
            self.assertEqual(info['records'],3)
            e.s.db.close()

    def test_non_yesterday_deadline_resets_consecutive_count(self):
        with tempfile.TemporaryDirectory() as d:
            c=Config(); c.root=Path(d); e=Engine(c)
            adapter=MagicMock(); adapter.http.failures=0
            adapter.search.return_value='page1'; adapter.links.return_value=['u1','u2','u3','u4','u5']
            adapter.next_page.return_value=None
            deadlines=['2025-01-01T12:00:00-06:00','2025-01-01T12:00:00-06:00',
                       '2025-01-02T12:00:00-06:00','2025-01-01T12:00:00-06:00',
                       '2025-01-01T12:00:00-06:00']
            adapter.detail.side_effect=lambda url: {
                'external_id':url,'source_url':url,'data':{'deadline':deadlines[int(url[1:])-1],'documents':[]}}
            today=date(2025,1,2)
            with patch('bastion_crawler.core.HonduCompras',return_value=adapter), \
                 patch('bastion_crawler.core.deadline_is_yesterday',side_effect=lambda value: deadline_is_yesterday(value,today)):
                run=e.run()
            self.assertEqual(adapter.detail.call_count,5)
            self.assertEqual(e.s.db.execute('SELECT stop_reason FROM runs WHERE id=?',(run,)).fetchone()[0],None)
            self.assertEqual(e.s.db.execute('SELECT status FROM runs WHERE id=?',(run,)).fetchone()[0],'SUCCESS')
            e.s.db.close()

    def test_existing_state_adds_stop_reason_without_losing_runs(self):
        with tempfile.TemporaryDirectory() as d:
            db=sqlite3.connect(Path(d)/'state.sqlite3')
            db.execute('CREATE TABLE runs(id TEXT PRIMARY KEY,started TEXT,finished TEXT,status TEXT,listing_complete INTEGER DEFAULT 0)')
            db.execute("INSERT INTO runs(id,started,status) VALUES('existing','2025-01-01','SUCCESS')")
            db.commit(); db.close()
            store=Store(d)
            row=store.db.execute('SELECT id,started,status,stop_reason FROM runs').fetchone()
            self.assertEqual(tuple(row),('existing','2025-01-01','SUCCESS',None))
            store.db.close()

    def test_empty_retained(self):
        x=parse_detail(HTML,'http://host/p?id=1')
        self.assertEqual(x['completeness'],'EMPTY')
        self.assertNotIn('description',x['fields_present'])
    def test_identity_stable(self):
        a=parse_detail(HTML,'http://host/p?id=1&x=2')
        b=parse_detail(HTML.replace(b'ENEE',b'SAR'),'http://host/p?x=2&id=1')
        self.assertEqual(a['external_id'],b['external_id'])
        self.assertNotEqual(a['content_hash'],b['content_hash'])
    def test_normalized_hash(self):
        a=parse_detail(HTML,'http://host/p?id=1')
        b=parse_detail(HTML.replace(b' ENEE ',b'  ENEE\n '),'http://host/p?id=1')
        self.assertEqual(a['content_hash'],b['content_hash'])
    def test_unexpected_html(self):
        with self.assertRaises(ValueError): parse_detail(b'<html>error</html>','http://host')
    def test_absent_vs_empty(self):
        x=parse_detail(HTML+b'<table id="ctl00_cphCuerpo_WebTab1_tmpl0_wdgProductos"></table>','http://host')
        self.assertIn('description',x['fields_present'])
        self.assertEqual(x['data']['description'],'')
    def test_checkpoint_survives_restart(self):
        with tempfile.TemporaryDirectory() as d:
            s=Store(d); run=s.create()
            with s.db: s.add(run,'detail','u'); s.success(run,'detail','u')
            s.db.close(); s=Store(d)
            self.assertEqual(s.db.execute('select status from tasks').fetchone()[0],'SUCCEEDED')
            s.db.close()
    def test_partial_export_and_immutable_batches(self):
        with tempfile.TemporaryDirectory() as d:
            c=Config(); c.root=Path(d); e=Engine(c); run=e.s.create()
            obs=parse_detail(HTML,'http://host?id=1')
            with e.s.db:
                e.s.db.execute('insert into observations values(?,?,?)',(run,obs['external_id'],dumps(obs)))
                e.s.add(run,'detail','failed')
            e.finish(run)
            path=next((Path(d)/'batches').iterdir()); old=(path/'manifest.json').read_bytes()
            self.assertEqual(json.loads((path/'run.json').read_text())['status'],'PARTIAL_SUCCESS')
            validate(path); e.finish(run)
            self.assertEqual(old,(path/'manifest.json').read_bytes())
            self.assertEqual(len(list((Path(d)/'batches').iterdir())),2)
            e.s.db.close()
    def test_corruption_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d); atomic(p/'manifest.json',dumps({'files':{'processes.csv':'bad'}}).encode()); atomic(p/'processes.csv',b'abc')
            with self.assertRaises(ValueError): validate(p)
    def test_retry_persistent(self):
        with tempfile.TemporaryDirectory() as d:
            s=Store(d); run=s.create(); s.error(run,'detail','u',TimeoutError('timeout'))
            row=s.db.execute('select * from tasks').fetchone()
            self.assertEqual(row['status'],'RETRY_WAIT'); self.assertGreater(row['next_at'],time.time()); s.db.close()
    def test_duplicate_task(self):
        with tempfile.TemporaryDirectory() as d:
            s=Store(d); run=s.create()
            with s.db: s.add(run,'detail','u'); s.add(run,'detail','u')
            self.assertEqual(s.db.execute('select count(*) from tasks').fetchone()[0],1); s.db.close()
    def test_no_false_document_removal(self):
        self.assertFalse(parse_detail(HTML,'http://host')['documents_complete'])
    def test_pager(self):
        c=Config(); h=MagicMock(); a=HonduCompras(h,c)
        soup=BeautifulSoup('<table id="ctl00_cphCuerpo_gvResultados"></table>','html.parser')
        self.assertIsNone(a.next_page(soup,1)); h.get.assert_not_called()
    def test_delivery_failure_does_not_erase_data(self):
        with tempfile.TemporaryDirectory() as d:
            c=Config(); c.root=Path(d); e=Engine(c); run=e.s.create(); e.finish(run)
            with patch.dict(os.environ,{'S3_ENABLED':'true','BLUEHOST_MODE':'disabled'}),patch('bastion_crawler.delivery.s3_send',side_effect=TimeoutError()): deliver(e)
            row=e.s.db.execute("select status,attempts from deliveries where target='s3'").fetchone()
            self.assertEqual(tuple(row),('RETRY_WAIT',1)); self.assertTrue(list((Path(d)/'batches').iterdir())); e.s.db.close()
    def test_api_uncommitted_ack_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d); atomic(p/'processes.json',dumps({'processes':[{}]}).encode()); atomic(p/'run.json',dumps({'started':'x'}).encode())
            response=MagicMock(status_code=200); response.json.return_value={'status':'RECEIVED'}
            session=MagicMock(); session.__enter__.return_value=session; session.post.return_value=response
            with patch.dict(os.environ,{'BASTION_API_URL':'https://host/api','BASTION_TOKEN':'test'}),patch('bastion_crawler.delivery.requests.Session',return_value=session):
                with self.assertRaises(ValueError): api_send(p,{'run_id':'r','batch_id':'b'})

if __name__=='__main__': unittest.main()
