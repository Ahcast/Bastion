# Observabilidad

Fuente: [Documento maestro vigente](../Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

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

