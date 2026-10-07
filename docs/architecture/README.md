# Arquitectura por entorno

| Entorno | Responsabilidad | Datos |
| --- | --- | --- |
| Bluehost | Angular, API Laravel, autorización, negocio, importación y correo | MySQL como fuente de verdad de negocio |
| AWS | Crawler, lotes S3 y workers externos propuestos | Archivos originales, artefactos y resultados |
| Compartido | Contratos, seguridad y decisiones | Documentación versionada |

## Flujo de procesos
Crawler AWS → lote CSV y manifiesto en S3 → importador Laravel en Bluehost → validación, idempotencia e historial MySQL → revisión humana.

## Flujo de trabajos pesados propuesto
Laravel crea job autorizado → worker AWS procesa entrada privada → resultado privado → Laravel valida y registra resultado → aprobación humana cuando corresponda.
Transporte y runtime por elegir. Ningún worker obtiene acceso directo a MySQL ni permisos de aprobación.

## Límites
Angular y API comparten origen HTTPS. Laravel aplica políticas y mantiene sesiones/MFA. El correo sigue en Bluehost.
AWS conserva originales y ejecuta extracción; Bluehost interpreta los datos y decide los cambios de negocio.
Cron y jobs cortos en Bluehost sujetos a sus límites. OCR, IA y generación pesada se reservan a workers externos propuestos.
No hay servicios implementados ni recursos provisionados.
