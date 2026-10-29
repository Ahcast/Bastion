#!/usr/bin/python3
"""Convert one staged BMP/TIFF to PNG pages inside the isolated PDF worker.

All input and output names are derived from an internally assigned number.
No network, credentials, user paths or user text are accepted.
"""
import os
from pathlib import Path
import sys
import warnings
from PIL import Image, ImageOps

MAX_PIXELS = 50_000_000
MAX_FRAMES = 1000


def convert(root, number):
    if not root.is_absolute() or root.is_symlink() or not root.is_dir() or not 3 <= number <= 4000:
        raise ValueError('invalid input')
    source = root / f'attachment-{number:06}.bin'
    if source.is_symlink() or not source.is_file():
        raise ValueError('invalid image')
    Image.MAX_IMAGE_PIXELS = MAX_PIXELS
    warnings.simplefilter('error', Image.DecompressionBombWarning)
    pages = []
    with Image.open(source) as image:
        if image.format not in ('BMP', 'TIFF'):
            raise ValueError('unsupported image')
        frames = getattr(image, 'n_frames', 1)
        if not 1 <= frames <= MAX_FRAMES:
            raise ValueError('too many image pages')
        for frame in range(frames):
            image.seek(frame)
            if image.width < 1 or image.height < 1 or image.width * image.height > MAX_PIXELS:
                raise ValueError('image too large')
            name = f'image-{number:06}-{frame + 1:04}.png'
            with open(root / name, 'xb') as output:
                with ImageOps.exif_transpose(image).convert('RGBA') as page:
                    page.save(output, format='PNG')
            pages.append(f'#page[#align(center + horizon)[#image("{name}", width: 100%, height: 100%, fit: "contain")]]')
    template = '#set page(paper: "us-letter", margin: (x: 1.3cm, y: 1.5cm))\n' + '\n'.join(pages) + '\n'
    with open(root / f'source-{number:06}.typ', 'x', encoding='utf-8') as output:
        output.write(template)


if __name__ == '__main__':
    try:
        if len(sys.argv) != 3 or not sys.argv[2].isascii() or not sys.argv[2].isdigit():
            raise ValueError('invalid request')
        convert(Path(sys.argv[1]), int(sys.argv[2]))
    except Exception:
        print('No se pudo procesar la imagen.', file=sys.stderr)
        sys.exit(1)
