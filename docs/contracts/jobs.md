# Contrato propuesto de trabajos externos

Solicitud: schema_version, job_id, type, input_version, input_object_refs, input_hash, requested_at y correlation_id.
Resultado: job_id, input_version, status, output_object_refs, output_hash, started_at, finished_at y error resumido.
Laravel genera la solicitud después de autorizarla. El worker solo procesa objetos permitidos. Transporte autenticado por definir.
Deduplicar por job_id y versión. Rechazar resultados de otra versión o hash inválido. La recepción no aprueba ni envía comunicaciones.
