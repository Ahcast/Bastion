# Arquitectura propuesta

## Enfoque
Monolito modular Laravel con MySQL y frontend Angular. Angular y API se sirven bajo un mismo origen HTTPS. Cada módulo posee sus reglas y operaciones; las integraciones usan servicios definidos, sin acceso directo a tablas ajenas.

## Componentes
| Componente | Responsabilidad |
| --- | --- |
| Angular | Interfaz y navegación; no decide permisos |
| Laravel | Autenticación, autorización, validación y operaciones |
| MySQL | Estado de negocio, historial, jobs y auditoría |
| Integraciones | Crawler, proveedores de IA, correo y almacenamiento |
| Cron y colas | Ejecutar trabajos limitados y recuperables |

## Flujo principal
Crawler → lote de importación → validación → procesos e historial → clasificación asistida por IA → revisión humana → cotización y expediente → aprobación.

## Reglas
- La API aplica autorización por recurso y operación.
- Ningún endpoint público ejecuta comandos arbitrarios.
- Archivos privados fuera del directorio público; descargas autorizadas.
- Los eventos de negocio y jobs deben persistirse de forma consistente, usando un patrón outbox cuando corresponda.
- Correo en Bluehost. No se presupone migración.
- No se requieren EC2 ni Lightsail para alojar la aplicación.
- Ubicación y runtime del crawler por decidir; no asumir que Bluehost admite sus dependencias.
- Este documento propone la nueva base; no representa un inventario verificado del sistema legacy.
