"""Validate local Markdown links and mandatory project entry points."""
from pathlib import Path
import re
import sys
ROOT = Path(__file__).resolve().parents[1]
errors = []
for name in ('AGENTS.md', 'README.md', 'docs/development/plan.md', 'docs/development/status.md'):
    if not (ROOT / name).is_file():
        errors.append(f'Missing required file: {name}')
for file in ROOT.rglob('*.md'):
    if any(p in ('.git', 'node_modules', '.venv') for p in file.parts):
        continue
    for target in re.findall(r'!?\[[^\]]*\]\(([^\s)]+)(?:\s+[^)]*)?\)', file.read_text()):
        if target.startswith(('https:', 'http:', 'mailto:', '#')):
            continue
        destination = (file.parent / target.split('#')[0]).resolve()
        if not destination.exists():
            errors.append(f'{file.relative_to(ROOT)}: missing {target}')
if errors:
    print('\n'.join(errors))
    sys.exit(1)
print('Repository documentation checks passed')
