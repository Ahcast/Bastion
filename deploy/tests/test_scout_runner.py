import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest.mock import patch


@unittest.skipUnless(os.name == 'posix', 'The controller uses Linux Unix sockets and flock')
class ControllerTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location('runner', Path(__file__).resolve().parents[1] / 'scout_runner.py')
        self.runner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.runner)
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.script = self.root / 'synthetic.py'
        self.script.write_text('import sys,time\nfrom pathlib import Path\nassert "--manual" in sys.argv\ntime.sleep(0.15)\n')
        self.control = self.runner.Controller(Path(sys.executable), self.script, self.root/'config.json', self.root, self.root/'state')
        self.addCleanup(self.control.db.close)

    def completed(self):
        for _ in range(100):
            if self.control.process is None:
                return self.control.snapshot()
            time.sleep(0.02)
        self.fail('Synthetic executable did not finish')

    def test_status_and_untrusted_requests_never_execute(self):
        with patch.object(self.runner.subprocess, 'Popen') as spawn:
            self.assertFalse(self.control.dispatch({'action':'status'})['busy'])
            for request in [{'action':'start','id':'../../etc/passwd'}, {'action':'start','id':'1234567890abcdef','command':'sh'}, {'action':'status','path':'/etc/passwd'}, [], {'action':'exec'}]:
                self.assertEqual(self.control.dispatch(request), {'error':'invalid'})
            spawn.assert_not_called()

    def test_pipeline_runs_in_background_and_retries_recover_receipt(self):
        request = {'action':'start','id':'7_1234567890abcdef'}
        started = self.control.dispatch(request)
        self.assertTrue(started['busy'])
        self.assertEqual(self.control.dispatch(request)['job']['id'], request['id'])
        self.assertEqual(self.control.dispatch({'action':'start','id':'7_another_request_id'}), {'error':'busy'})
        final = self.completed()
        self.assertEqual(final['job']['status'], 'succeeded')
        with patch.object(self.runner.subprocess,'Popen') as spawn:
            self.assertEqual(self.control.dispatch(request)['job']['status'],'succeeded')
            spawn.assert_not_called()

    def test_scheduled_lock_prevents_manual_start(self):
        import fcntl
        with (self.root/'pipeline.lock').open('a+b') as handle:
            fcntl.flock(handle,fcntl.LOCK_EX|fcntl.LOCK_NB)
            self.assertTrue(self.control.dispatch({'action':'status'})['busy'])
            with patch.object(self.runner.subprocess,'Popen') as spawn:
                self.assertEqual(self.control.dispatch({'action':'start','id':'7_1234567890abcdef'}), {'error':'busy'})
                spawn.assert_not_called()

    def test_spawn_failure_is_not_reported_as_success(self):
        with patch.object(self.runner.subprocess,'Popen',side_effect=OSError('PRIVATE_TOKEN')):
            reply = self.control.dispatch({'action':'start','id':'7_1234567890abcdef'})
        self.assertEqual(reply,{'error':'unavailable'})
        self.assertEqual(self.control.snapshot()['job']['status'],'failed')
        self.assertNotIn('PRIVATE_TOKEN',json.dumps(self.control.snapshot()))

    def test_restart_marks_unfinished_receipt_interrupted(self):
        self.control.db.execute("INSERT INTO runs VALUES ('7_1234567890abcdef','running','2026-10-24T01:00:00Z',NULL)")
        self.control.db.commit()
        resumed = self.runner.Controller(Path(sys.executable), self.script, self.root/'config.json',self.root,self.root/'state')
        self.addCleanup(resumed.db.close)
        self.assertEqual(resumed.snapshot()['job']['status'],'interrupted')
        with patch.object(self.runner.subprocess,'Popen') as spawn:
            self.assertEqual(resumed.dispatch({'action':'start','id':'7_1234567890abcdef'})['job']['status'],'interrupted')
            spawn.assert_not_called()


if __name__ == '__main__':
    unittest.main()
