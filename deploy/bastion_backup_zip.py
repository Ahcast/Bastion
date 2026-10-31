"""Core archive writer; caller supplies verified streams and controls storage."""
import datetime
import hashlib
import json
import hmac
import zipfile
from bastion_backup_validate import _path, MAX_FILE, MAX_FILES, MAX_TOTAL, CHUNK, validate_archive


def write_archive(destination, streams, commit, signing_key):
    """Write one ZIP from (name, binary stream) pairs and verify it before return.

    The caller is responsible for providing a consistent database snapshot and
    stable object versions, locking, free-space checks, and cleanup on failure.
    """
    entries = []
    seen = set()
    total = 0
    try:
        with zipfile.ZipFile(destination, 'x', compression=zipfile.ZIP_DEFLATED, allowZip64=True) as out:
            for name, stream in streams:
                name = _path(name)
                if name == 'manifest.json' or name in seen or len(seen) >= MAX_FILES:
                    raise ValueError('Duplicate or invalid archive entry')
                seen.add(name)
                digest = hashlib.sha256()
                size = 0
                with out.open(name, 'w', force_zip64=True) as target:
                    while chunk := stream.read(CHUNK):
                        size += len(chunk)
                        total += len(chunk)
                        if size > MAX_FILE or total > MAX_TOTAL:
                            raise ValueError('Backup exceeds archive limits')
                        digest.update(chunk)
                        target.write(chunk)
                entries.append({'path': name, 'size': size, 'sha256': digest.hexdigest()})
            if not entries:
                raise ValueError('Empty backup')
            manifest = {'format': 'bastion-backup-v1', 'created_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'commit': commit, 'files': entries}
            manifest['key_id'] = hashlib.sha256(signing_key).hexdigest()
            canonical = json.dumps(manifest, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode('utf-8')
            derived = hashlib.sha256(b'bastion-backup-manifest-v1\0' + signing_key).digest()
            manifest['signature'] = hmac.new(derived, canonical, hashlib.sha256).hexdigest()
            encoded = json.dumps(manifest, separators=(',', ':'), ensure_ascii=False).encode('utf-8')
            from bastion_backup_validate import MAX_MANIFEST
            if len(encoded) > MAX_MANIFEST:
                raise ValueError('Manifest too large')
            out.writestr('manifest.json', encoded)
        return validate_archive(destination)
    except BaseException:
        # Only remove the exact newly created archive, never an existing file.
        if 'out' in locals():
            destination.unlink(missing_ok=True)
        raise
