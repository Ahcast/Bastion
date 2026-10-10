"""Validate project documentation, OpenAPI JSON and REST contract coverage."""
from pathlib import Path
import json
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
errors: list[str] = []

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

# Every checked-in OpenAPI contract must at least be valid JSON with the expected
# top-level objects. This catches truncated/manual edits before they become a
# misleading source of truth.
openapi_docs: dict[str, dict] = {}
for file in sorted((ROOT / 'docs' / 'contracts').glob('*-openapi.json')):
    try:
        document = json.loads(file.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        errors.append(f'{file.relative_to(ROOT)}: invalid JSON: {exc}')
        continue
    if not isinstance(document, dict) or not isinstance(document.get('paths'), dict):
        errors.append(f'{file.relative_to(ROOT)}: missing OpenAPI paths object')
        continue
    if not isinstance(document.get('openapi'), str):
        errors.append(f'{file.relative_to(ROOT)}: missing OpenAPI version')
    openapi_docs[file.name] = document

# Quotes is the active commercial module and its Go registry is authoritative for
# exposed REST operations. Keep the machine-readable contract from lagging behind
# newly registered routes. Static register("METHOD /path", ...) calls are found
# automatically; three legacy routes are assembled by a small loop in module.go
# and are declared explicitly here until that registration is made static.
quotes_contract = openapi_docs.get('quotes-openapi.json')
quotes_dir = ROOT / 'backend' / 'internal' / 'modules' / 'quotes'
if quotes_contract is not None and quotes_dir.is_dir():
    route_pattern = re.compile(
        r'register\("(GET|POST|PATCH|PUT|DELETE) (/api/v1/[^"\\]+)"'
    )
    registered: set[tuple[str, str]] = set()
    for source in quotes_dir.glob('*.go'):
        text = source.read_text()
        registered.update((method.lower(), path) for method, path in route_pattern.findall(text))
    registered.update({
        ('post', '/api/v1/quotes/{id}/clone'),
        ('delete', '/api/v1/quotes/{id}'),
        ('post', '/api/v1/quotes/{id}/restore'),
    })

    documented: set[tuple[str, str]] = set()
    for path, operations in quotes_contract['paths'].items():
        if not isinstance(operations, dict):
            errors.append(f'docs/contracts/quotes-openapi.json: invalid operations for {path}')
            continue
        for method in operations:
            if method.lower() in {'get', 'post', 'patch', 'put', 'delete'}:
                documented.add((method.lower(), path))

    for method, path in sorted(registered - documented):
        errors.append(
            'docs/contracts/quotes-openapi.json: '
            f'missing registered Quotes route {method.upper()} {path}'
        )

if errors:
    print('\n'.join(errors))
    sys.exit(1)
print('Repository documentation checks passed')
