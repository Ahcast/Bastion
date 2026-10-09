# Operación y despliegue


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

## 34.3 Schedule de ejemplo, no aprobado

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

## 34.5 Retención de ejemplo, pendiente de aprobación

```text
diarios: 30 días
semanales: 12 semanas
mensuales: 12 meses
```

Después:

- Glacier según política.

---




# Observabilidad


# 36. Observabilidad

Debe existir:

- logs estructurados;
- request_id;
- correlation_id;
- job_id;
- run_id;
- actor;
- timestamps;
- errors;
- latency;
- health checks.

Crawler:

- coverage;
- retry count;
- records;
- failed units;
- delivery status.

Backups:

- timestamp;
- size;
- checksum;
- S3 verification.

AI:

- task;
- actor;
- tools;
- result;
- duration;
- approval;
- error.

---


# 47. Fallos previstos

## HonduCompras lento/inestable

Mitigación:

- retries;
- backoff;
- checkpoint;
- partial success;
- replay;
- schedule 2x/día.

## Bluehost temporalmente caído

Mitigación:

- store S3;
- pending delivery;
- retry delivery.

## AI no disponible

Mitigación:

- core sigue funcionando;
- tasks quedan pendientes;
- retry posterior.

## Email falla

Mitigación:

- estado;
- retry;
- no duplicar;
- audit.

## Backup inválido

Mitigación:

- checksum;
- size validation;
- manifest;
- alerta.

---

## Observabilidad del pipeline IA

Última ejecución Work satisfactoria, pendientes/antigüedad por etapa, reservas vencidas, fallos, procesos relevantes próximos a cierre y RFQ sin respuesta. Métricas comerciales: investigación -> cotización -> oferta presentada, tiempo por oferta y motivos de descarte. Nunca contar tareas completadas como ofertas. Correlación proceso/ítem/tarea/run/RFQ/documento; modelo/tokens/costo desconocidos se mantienen null, no cero.



## Despliegue objetivo

Frontend: compilar Angular en CI/desarrollo y publicar únicamente assets en Bluehost. Backend Go/API/MCP y workers: Linux en Lightsail, servicios separados con systemd, proxy HTTPS, usuario sin privilegios y configuración privada. Crawler Python conserva su entorno y SQLite técnico; no es la base operativa.

MySQL permanece en Bluehost. Validar conexión cifrada y acceso restringido desde IP estática Lightsail, límites de conexiones y compatibilidad Percona 5.7. Si el hosting no admite TLS directo, validar túnel SSH supervisado antes de producción; no habilitar MySQL público irrestricto ni migrarlo sin decisión expresa. Migraciones con backup/restauración probada, bloqueo de ejecutor único y rollback documentado.

S3 privado para documentos y backups, IAM de mínimo privilegio y credenciales fuera de Git. Bucket crawler confirmado: castilloit-honducompras-crawler, us-east-1; buckets de documentos/backups requieren configuración. Enlaces temporales solo tras autorización por entidad. SMTP/IMAP Bluehost; conexiones/envíos reales requieren entorno validado.

La instancia inicial objetivo es 2 GB/2 vCPU/60 GB. Medir consumo conjunto, limitar concurrencia crawler/jobs y conexiones MySQL; no asumir capacidad sin prueba. Sin OCR, modelos locales ni servidor vectorial. Health/readiness y logs permiten distinguir proceso vivo de dependencias disponibles.

Antes de despliegue: build/pruebas, respaldo, migraciones verificadas, secretos, DNS/HTTPS/CORS/CSRF, accesos MySQL/S3/correo y restauración. Registrar versión y smoke tests; no declarar producción por pruebas locales. Desarrollo autorizado no implica desplegar ni enviar correo real.
