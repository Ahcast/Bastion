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
# exposed REST operations. The mature quote aggregate and the isolated FX
# observation foundation have separate machine-readable contracts; their union
# must match the Go registry exactly and a route may not be documented twice.
quotes_contract_names = ('quotes-openapi.json', 'quotes-fx-openapi.json')
quotes_dir = ROOT / 'backend' / 'internal' / 'modules' / 'quotes'
if all(name in openapi_docs for name in quotes_contract_names) and quotes_dir.is_dir():
    route_pattern = re.compile(
        r'register\("(GET|POST|PATCH|PUT|DELETE) (/api/v1/[^"\\]+)"'
    )
    registered: set[tuple[str, str]] = set()
    for source in quotes_dir.glob('*.go'):
        text = source.read_text()
        registered.update((method.lower(), path) for method, path in route_pattern.findall(text))
    registered.update({
        ('post', '/api/v1/quotes'),
        ('patch', '/api/v1/quotes/{id}'),
        ('post', '/api/v1/quotes/{id}/clone'),
        ('delete', '/api/v1/quotes/{id}'),
        ('post', '/api/v1/quotes/{id}/restore'),
    })

    documented_by: dict[tuple[str, str], str] = {}
    for contract_name in quotes_contract_names:
        contract = openapi_docs[contract_name]
        for path, operations in contract['paths'].items():
            if not isinstance(operations, dict):
                errors.append(f'docs/contracts/{contract_name}: invalid operations for {path}')
                continue
            for method in operations:
                operation = (method.lower(), path)
                if operation[0] not in {'get', 'post', 'patch', 'put', 'delete'}:
                    continue
                previous = documented_by.get(operation)
                if previous is not None:
                    errors.append(
                        f'docs/contracts/{contract_name}: duplicates {method.upper()} {path} '
                        f'already documented by {previous}'
                    )
                else:
                    documented_by[operation] = contract_name

    documented = set(documented_by)
    for method, path in sorted(registered - documented):
        errors.append(
            'Quotes OpenAPI contracts: '
            f'missing registered route {method.upper()} {path}'
        )
    for method, path in sorted(documented - registered):
        errors.append(
            f'docs/contracts/{documented_by[(method, path)]}: '
            f'documents nonexistent Quotes route {method.upper()} {path}'
        )

# PurchaseOrders Unit 1 has one machine-readable REST contract. Its registered
# routes and documented operations must remain bidirectionally identical so a
# future handler cannot silently escape the reviewed commercial contract.
purchase_orders_contract_name = 'purchase-orders-openapi.json'
purchase_orders_dir = ROOT / 'backend' / 'internal' / 'modules' / 'purchaseorders'
if purchase_orders_contract_name in openapi_docs and purchase_orders_dir.is_dir():
    route_pattern = re.compile(
        r'register\("(GET|POST|PATCH|PUT|DELETE) (/api/v1/[^"\\]+)"'
    )
    registered: set[tuple[str, str]] = set()
    for source in purchase_orders_dir.glob('*.go'):
        text = source.read_text()
        registered.update((method.lower(), path) for method, path in route_pattern.findall(text))
    # Save routes are constructed by the POST/PATCH loop in module.go.
    registered.update({
        ('post', '/api/v1/purchase-orders'),
        ('patch', '/api/v1/purchase-orders/{id}'),
    })

    documented: set[tuple[str, str]] = set()
    contract = openapi_docs[purchase_orders_contract_name]
    for path, operations in contract['paths'].items():
        if not isinstance(operations, dict):
            errors.append(
                f'docs/contracts/{purchase_orders_contract_name}: invalid operations for {path}'
            )
            continue
        for method in operations:
            operation = (method.lower(), path)
            if operation[0] in {'get', 'post', 'patch', 'put', 'delete'}:
                documented.add(operation)

    for method, path in sorted(registered - documented):
        errors.append(
            'PurchaseOrders OpenAPI contract: '
            f'missing registered route {method.upper()} {path}'
        )
    for method, path in sorted(documented - registered):
        errors.append(
            f'docs/contracts/{purchase_orders_contract_name}: '
            f'documents nonexistent PurchaseOrders route {method.upper()} {path}'
        )

if errors:
    print('\n'.join(errors))
    sys.exit(1)
print('Repository documentation checks passed')
