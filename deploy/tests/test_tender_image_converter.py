import importlib.util
from pathlib import Path
import tempfile
import unittest
from PIL import Image

spec = importlib.util.spec_from_file_location('converter', Path(__file__).parents[1] / 'bastion-tender-image-converter.py')
converter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(converter)


class TenderImageConversion(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)

    def tearDown(self):
        self.temp.cleanup()

    def test_bmp_and_multipage_tiff_keep_all_pages(self):
        image = Image.new('RGB', (3, 2), 'red')
        image.save(self.root / 'attachment-000003.bin', format='BMP')
        converter.convert(self.root, 3)
        with Image.open(self.root / 'image-000003-0001.png') as result:
            self.assertEqual(result.size, (3, 2))
        self.assertEqual((self.root / 'source-000003.typ').read_text().count('#page['), 1)
        second = Image.new('RGB', (2, 4), 'blue')
        image.save(self.root / 'attachment-000004.bin', format='TIFF', save_all=True, append_images=[second])
        converter.convert(self.root, 4)
        with Image.open(self.root / 'image-000004-0002.png') as result:
            self.assertEqual(result.size, (2, 4))
        self.assertEqual((self.root / 'source-000004.typ').read_text().count('#page['), 2)
        image.close()
        second.close()

    def test_rejects_unexpected_formats_and_duplicate_outputs(self):
        with Image.new('RGB', (1, 1), 'white') as image:
            image.save(self.root / 'attachment-000003.bin', format='PNG')
        with self.assertRaises(ValueError):
            converter.convert(self.root, 3)
        with self.assertRaises(ValueError):
            converter.convert(self.root, 2)


if __name__ == '__main__':
    unittest.main()
