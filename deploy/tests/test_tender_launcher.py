import importlib.util
import os
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('launcher', Path(__file__).parents[1] / 'bastion-tender-launcher.py')
launcher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(launcher)


class TenderLauncherValidation(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        os.chmod(self.root, 0o700)
        (self.root / 'request.json').write_text('{}')

    def tearDown(self):
        self.temp.cleanup()

    def test_accepts_only_safe_regular_input_files(self):
        (self.root / 'source-000003.typ').write_text('= Ejemplo')
        self.assertEqual(set(launcher.check_staged(self.root, os.getuid())), {'request.json', 'source-000003.typ'})
        for name in ('../secret.typ', '.hidden.typ', 'other.sh', 'x/file.pdf'):
            self.assertFalse(launcher.validate_name(name))

    def test_rejects_symlinks_hardlinks_and_accessible_directories(self):
        (self.root / 'source.typ').symlink_to(self.root / 'request.json')
        with self.assertRaises(ValueError):
            launcher.check_staged(self.root, os.getuid())
        (self.root / 'source.typ').unlink()
        os.link(self.root / 'request.json', self.root / 'second.json')
        with self.assertRaises(ValueError):
            launcher.check_staged(self.root, os.getuid())
        (self.root / 'second.json').unlink()
        os.chmod(self.root, 0o755)
        with self.assertRaises(ValueError):
            launcher.check_staged(self.root, os.getuid())

    def test_rejects_oversized_worker_request(self):
        with open(self.root / 'request.json', 'wb') as f:
            f.truncate(2_000_001)
        with self.assertRaises(ValueError):
            launcher.check_staged(self.root, os.getuid())


if __name__ == '__main__':
    unittest.main()
