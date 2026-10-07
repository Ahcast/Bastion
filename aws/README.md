# AWS

## Responsabilidades
- crawler/: extracción de HonduCompras.
- storage/: S3 para lotes originales, resultados y respaldos.
- automation/: planificación, reintentos y coordinación de tareas externas.
- workers/: procesamiento pesado propuesto (IA, OCR y documentos).
- security/: identidad de servicios y permisos.
- deployment/: runtime y despliegue independiente.

La aplicación web, MySQL y correo permanecen en Bluehost. AWS no necesita alojar una copia de la aplicación.
El bucket conocido es castilloit-honducompras-crawler, región us-east-1. Configuración real pendiente de inspección.
El runtime del crawler y workers queda por definir; esta estructura no obliga a contratar EC2, Lightsail ni otro servicio. IA puede usar un proveedor externo mediante un worker.
