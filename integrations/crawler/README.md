# Crawler

## Contrato propuesto
processes.csv contiene lo extraído, incluso cuando una ejecución falla o es parcial. Su existencia no demuestra una extracción completa.
Cada lote debe acompañarse de un manifiesto: schema_version, run_id, source, started_at, finished_at, status (SUCCESS/PARTIAL/FAILED), expected_count cuando se conozca, extracted_count, checksum y errores.

## Ingestión
Persistir lote y archivo original; validar antes de aplicar cambios. Identidad única source + external_id; si no existe ID confiable, URL canónica o fingerprint documentado.
Calcular hash del contenido de negocio normalizado, excluyendo timestamps de extracción. Resultado NEW, UNCHANGED o UPDATED. Conservar versiones y referencias al lote.
Nunca TRUNCATE + reload. Un lote parcial no elimina procesos ausentes. Una ausencia solo puede evaluarse después de ejecuciones completas y una política definida.

## Recuperación
run_id e identidad única evitan duplicados. Transacciones por unidad de trabajo, checkpoints, reintentos limitados con backoff y jitter; errores permanentes a revisión. Reprocesar un lote debe conservar el mismo resultado.
