import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch,MagicMock
from bastion_crawler.core import Config,Engine,parse_detail,HTTP
from test_core import HTML
import requests

class Recovery(unittest.TestCase):
    def test_page_failure_keeps_observation_then_resume(self):
        with tempfile.TemporaryDirectory() as d:
            c=Config(); c.root=Path(d); e=Engine(c)
            a=MagicMock(); a.http.failures=0
            a.search.return_value='page1'; a.links.return_value=['http://host/?id=1']
            a.detail.return_value=parse_detail(HTML,'http://host/?id=1')
            a.next_page.side_effect=TimeoutError('site unavailable')
            with patch('bastion_crawler.core.HonduCompras',return_value=a): run=e.run()
            self.assertEqual(e.s.db.execute('select count(*) from observations').fetchone()[0],1)
            self.assertEqual(e.s.db.execute('select status from runs').fetchone()[0],'PARTIAL_SUCCESS')
            a.next_page.side_effect=None; a.next_page.return_value=None
            with patch('bastion_crawler.core.HonduCompras',return_value=a): e.retry(run)
            self.assertEqual(a.detail.call_count,1)
            self.assertEqual(e.s.db.execute('select status from runs').fetchone()[0],'SUCCESS')
            e.s.db.close()
    def test_http_only_three_attempts(self):
        with tempfile.TemporaryDirectory() as d:
            c=Config(); c.root=Path(d); h=HTTP(c,Path(d))
            h.session=MagicMock(); h.session.request.side_effect=requests.Timeout()
            with patch('bastion_crawler.core.time.sleep'),patch('bastion_crawler.core.shutil.disk_usage',return_value=MagicMock(free=10**12)):
                with self.assertRaises(requests.Timeout): h.get(c.url)
            self.assertEqual(h.session.request.call_count,3)
    def test_http_404_not_retried(self):
        with tempfile.TemporaryDirectory() as d:
            c=Config(); c.root=Path(d); h=HTTP(c,Path(d)); h.session=MagicMock()
            response=requests.Response(); response.status_code=404
            h.session.request.side_effect=requests.HTTPError(response=response)
            with patch('bastion_crawler.core.time.sleep'),patch('bastion_crawler.core.shutil.disk_usage',return_value=MagicMock(free=10**12)):
                with self.assertRaises(requests.HTTPError): h.get(c.url)
            self.assertEqual(h.session.request.call_count,1)
