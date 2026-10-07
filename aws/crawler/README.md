# Crawler en AWS

Extraer HonduCompras y publicar processes.csv junto a un manifiesto versionado en S3 privado.
01:00 y 19:00, America/Tegucigalpa; programación configurable.
Mantener run_id, checkpoints, timeout, reintentos y estado SUCCESS/PARTIAL/FAILED.
Usar prefijos por ejecución y publicar el manifiesto al terminar de subir los objetos. No sobrescribir la historia.
La importación y los cambios de negocio ocurren en Bluehost; el crawler no modifica MySQL.
Ver [contrato completo](../../docs/contracts/crawler-import.md).
Runtime de ejecución por definir.
