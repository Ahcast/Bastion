# Automatización en AWS

Programar crawler y workers externos. Servicio de planificación y transporte por definir según duración, dependencias y costo.
Reintentos transitorios limitados con backoff y jitter; checkpoints y cuarentena de fallos permanentes.
La extracción tiene identidad run_id. Un job tiene job_id y versión de entrada. No repetir efectos de negocio ni enviar correo directamente.
Ver [contrato de jobs](../../docs/contracts/jobs.md).
