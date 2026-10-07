# Backup worker Bluehost a S3

Fuente: [Documento maestro v0.5](../../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 34. Backups Bluehost -> S3

Arquitectura preferida:

```text
Bluehost
  │
SSH/SFTP
  │
Lightsail Backup Worker
  │
S3
```

Ventaja:

- evitar credenciales AWS completas en hosting compartido;
- centralizar lógica en Lightsail.

## 34.1 MySQL

Flujo:

```text
mysqldump
  ↓
gzip
  ↓
SHA256
  ↓
S3
  ↓
verify
```

## 34.2 Files

- rsync/SFTP;
- compresión;
- incremental cuando sea posible;
- S3 sync.

## 34.3 Schedule sugerido

```text
23:30 MySQL
02:30 archivos
```

## 34.4 Manifest

Cada backup:

```json
{
  "backup_id": "...",
  "source": "bluehost",
  "started_at": "...",
  "finished_at": "...",
  "size_bytes": 0,
  "sha256": "...",
  "s3_key": "...",
  "status": "success"
}
```

## 34.5 Retención

```text
diarios: 30 días
semanales: 12 semanas
mensuales: 12 meses
```

Después:

- Glacier según política.

---

