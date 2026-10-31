"""Streaming validation for Bastion backup archives; never extracts files."""
import hashlib
import json
import re
import stat
import struct
import zipfile
from pathlib import PurePosixPath

MAX_MANIFEST = 4 * 1024 * 1024
MAX_FILES = 100_000
MAX_FILE = 20 * 1024**3
MAX_TOTAL = 100 * 1024**3
MAX_EXPANSION_RATIO = 10_000
CHUNK = 1024 * 1024


def preflight(stream):
    """Bound directory allocation before ZipFile constructs its entry objects."""
    stream.seek(0, 2); length = stream.tell()
    stream.seek(max(0, length - 65557)); tail = stream.read(65557)
    offset = tail.rfind(b'PK\x05\x06')
    if offset < 0 or len(tail) - offset < 22: raise ValueError('Missing ZIP directory')
    end = struct.unpack('<4s4H2LH', tail[offset:offset+22])
    if end[1] or end[2] or offset + 22 + end[7] != len(tail): raise ValueError('Invalid or split ZIP')
    count, size, start = end[4], end[5], end[6]
    if count == 65535 or size == 0xffffffff or start == 0xffffffff:
        position = length - len(tail) + offset
        if position < 20: raise ValueError('Invalid ZIP64 locator')
        stream.seek(position - 20); locator = struct.unpack('<4sLQL', stream.read(20))
        if locator[0] != b'PK\x06\x07' or locator[1] != 0 or locator[3] != 1: raise ValueError('Invalid ZIP64 locator')
        if locator[2] + 56 > position - 20: raise ValueError('Invalid ZIP64 offset')
        stream.seek(locator[2]); record = struct.unpack('<4sQ2H2L4Q', stream.read(56))
        if record[0] != b'PK\x06\x06' or record[1] < 44 or record[4] or record[5]: raise ValueError('Invalid ZIP64 directory')
        count, size, start = record[7], record[8], record[9]
    if count > MAX_FILES + 1 or size > 128 * 1024**2 or start + size > length: raise ValueError('ZIP directory exceeds limits')
    stream.seek(start); scanned = 0; finish = start + size
    while stream.tell() < finish:
        header = stream.read(46)
        if len(header) != 46 or header[:4] != b'PK\x01\x02': raise ValueError('Invalid ZIP directory entry')
        lengths = struct.unpack('<3H', header[28:34])
        scanned += 1
        if scanned > MAX_FILES + 1: raise ValueError('Too many ZIP entries')
        stream.seek(sum(lengths), 1)
        if stream.tell() > finish: raise ValueError('Invalid ZIP directory lengths')
    if scanned != count: raise ValueError('ZIP directory count mismatch')
    stream.seek(0)


def _path(name):
    if not isinstance(name, str) or not name or len(name) > 1024 or '\\' in name or '\x00' in name:
        raise ValueError('Invalid archive path')
    parts = name.split('/')
    if any(p in ('', '.', '..') for p in parts) or name.startswith('/') or ':' in parts[0]:
        raise ValueError('Unsafe archive path')
    if PurePosixPath(name).as_posix() != name:
        raise ValueError('Noncanonical archive path')
    if any(p.lower() == 'backups' for p in parts):
        raise ValueError('Nested backups are prohibited')
    if parts[-1].lower().startswith('bastion-backup-') and parts[-1].lower().endswith('.zip'):
        raise ValueError('Nested backup archive is prohibited')
    return name


def validate_archive(path):
    """Return validated manifest. Hashes detect corruption, not forgery."""
    with open(path, 'rb') as source: preflight(source)
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        if len(entries) > MAX_FILES + 1:
            raise ValueError('Too many entries')
        by_name = {}
        for entry in entries:
            name = _path(entry.filename)
            if entry.orig_filename != entry.filename or entry.compress_type not in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
                raise ValueError('Unsupported ZIP encoding or compression')
            if name in by_name or entry.is_dir() or entry.flag_bits & 1:
                raise ValueError('Duplicate, directory or encrypted entry')
            mode = entry.external_attr >> 16
            if stat.S_IFMT(mode) not in (0, stat.S_IFREG):
                raise ValueError('Special file not permitted')
            if entry.file_size > MAX_FILE:
                raise ValueError('Entry too large')
            if entry.file_size > 1024**2 and entry.file_size > max(entry.compress_size, 1) * MAX_EXPANSION_RATIO:
                raise ValueError('Excessive compression ratio')
            by_name[name] = entry
        manifest_info = by_name.pop('manifest.json', None)
        if not manifest_info or manifest_info.file_size > MAX_MANIFEST:
            raise ValueError('Invalid manifest')
        def unique_keys(pairs):
            result = {}
            for key, value in pairs:
                if key in result:
                    raise ValueError('Duplicate JSON key')
                result[key] = value
            return result
        with archive.open(manifest_info) as f:
            raw = f.read(MAX_MANIFEST + 1)
        if len(raw) > MAX_MANIFEST:
            raise ValueError('Manifest too large')
        manifest = json.loads(raw, object_pairs_hook=unique_keys)
        if not isinstance(manifest, dict) or manifest.get('format') != 'bastion-backup-v1':
            raise ValueError('Unsupported backup format')
        files = manifest.get('files')
        if not isinstance(files, list) or len(files) != len(by_name):
            raise ValueError('Manifest inventory mismatch')
        seen = set()
        total = 0
        for item in files:
            if not isinstance(item, dict):
                raise ValueError('Invalid manifest entry')
            name = _path(item.get('path'))
            expected = item.get('sha256')
            size = item.get('size')
            entry = by_name.get(name)
            if name in seen or entry is None or type(size) is not int or size < 0 or size != entry.file_size or not isinstance(expected, str) or not re.fullmatch(r'[0-9a-f]{64}', expected):
                raise ValueError('Invalid manifest inventory')
            seen.add(name)
            total += size
            if total > MAX_TOTAL:
                raise ValueError('Archive exceeds size limit')
            digest = hashlib.sha256()
            read = 0
            with archive.open(entry) as f:
                while chunk := f.read(CHUNK):
                    read += len(chunk)
                    if read > size:
                        raise ValueError('Entry exceeds declared size')
                    digest.update(chunk)
            if read != size or digest.hexdigest() != expected:
                raise ValueError('Entry integrity failure')
            with archive.open(entry) as source:
                magic = source.read(4)
                if magic == b'PK\x03\x04':
                    source.seek(0); preflight(source)
                    with zipfile.ZipFile(source) as nested:
                        if 'manifest.json' in nested.namelist():
                            info = nested.getinfo('manifest.json')
                            if info.file_size <= MAX_MANIFEST:
                                with nested.open(info) as value:
                                    raw = value.read(MAX_MANIFEST + 1)
                                try: nested_manifest = json.loads(raw)
                                except (ValueError, UnicodeError): nested_manifest = None
                                if isinstance(nested_manifest, dict) and nested_manifest.get('format') == 'bastion-backup-v1':
                                    raise ValueError('Nested Bastion backup is prohibited')
        return manifest
