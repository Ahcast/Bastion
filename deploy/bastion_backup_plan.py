"""Closed restore namespace. Archive metadata never selects host paths."""
from pathlib import Path
from bastion_backup_inventory import PREFIXES
from bastion_backup_validate import _path

LOCAL_ROOTS = {
    'branding': Path('/usr/local/share/bastion/branding'),
    'scout': Path('/var/lib/bastion-scout'),
    'scout-control': Path('/var/lib/bastion-scout-control'),
}
SCOUT_FILES = frozenset(('maestro.csv', 'raw.csv', 'control.sqlite3', 'combiner.json',
                         'main.json', 'scout.json', 'clasificador.json'))
LOCAL_FILES = {'scout': SCOUT_FILES, 'scout-control': frozenset(('runs.sqlite3',))}


def restore_plan(manifest):
    """Describe validated destinations without touching production.

    Must only be called after full ZIP validation. This module intentionally has
    no extraction or SQL execution operation.
    """
    plan = {'databases': [], 'local_files': [], 's3_objects': []}
    for item in manifest['files']:
        name = _path(item['path'])
        if name == 'databases/main.sql':
            plan['databases'].append({'name': 'main', 'bytes': item['size']})
        elif name.startswith('s3/'):
            key = name[3:]
            if not key.startswith(PREFIXES) or key.startswith('backups/'):
                raise ValueError('S3 destination outside operational namespaces')
            plan['s3_objects'].append({'key': key, 'bytes': item['size']})
        elif name.startswith('local/'):
            parts = name.split('/', 2)
            if len(parts) != 3 or parts[1] not in LOCAL_ROOTS:
                raise ValueError('Unrecognized local namespace')
            label, relative = parts[1:]
            if label in LOCAL_FILES and relative not in LOCAL_FILES[label]:
                raise ValueError('Unrecognized Scout data file')
            if label == 'branding' and Path(relative).suffix.lower() not in ('.png', '.jpg', '.jpeg', '.webp', '.svg'):
                raise ValueError('Unrecognized branding data file')
            configured = LOCAL_ROOTS[label]
            root = configured.resolve(strict=False)
            if configured.is_symlink() or root != configured.absolute():
                raise ValueError('Local data root redirects outside its configured location')
            target = root / relative
            # Resolve existing parents, so a host symlink cannot redirect writes.
            resolved = target.resolve(strict=False)
            if not resolved.is_relative_to(root) or target.is_symlink():
                raise ValueError('Local destination escapes its root')
            plan['local_files'].append({'namespace': label, 'relative': relative, 'bytes': item['size']})
        else:
            raise ValueError('Unrecognized archive entry')
    if len(plan['databases']) != 1:
        raise ValueError('Archive must contain exactly one primary database')
    return plan
