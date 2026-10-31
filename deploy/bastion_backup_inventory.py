"""Enumerate only the operational prefixes admitted by Bastion's S3 contract."""
import json
import subprocess

PREFIXES = ('cotizaciones/', 'colaboracion/', 'ordenes_compra/', 'facturas/', 'licitaciones/')
BUCKET = 'bastion-prod-assets'


def inventory(aws_env):
    """Yield current object metadata; never enumerate the backups prefix."""
    for prefix in PREFIXES:
        token = None
        while True:
            args = ['aws', 's3api', 'list-objects-v2', '--bucket', BUCKET,
                    '--prefix', prefix, '--max-keys', '1000', '--region', 'us-east-1',
                    '--no-cli-pager', '--output', 'json']
            if token:
                args += ['--continuation-token', token]
            result = subprocess.run(args, env=aws_env, capture_output=True, check=True)
            page = json.loads(result.stdout)
            for obj in page.get('Contents', []):
                key = obj['Key']
                if not key.startswith(prefix) or key.startswith('backups/'):
                    raise ValueError('Unexpected object in inventory')
                yield {'key': key, 'size': obj['Size'], 'etag': obj['ETag']}
            if not page.get('IsTruncated'):
                break
            next_token = page.get('NextContinuationToken')
            if not next_token or next_token == token:
                raise ValueError('Invalid S3 pagination')
            token = next_token
