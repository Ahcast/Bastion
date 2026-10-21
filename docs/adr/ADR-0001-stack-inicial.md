# ADR-0001: Stack inicial del Core

- Estado: aceptada para el primer corte
- Fecha: 2026-10-20

## Contexto

BastionV1 debe comenzar como backend Rust modular, desplegable primero sin módulos de negocio. El frontend se sirve en Bluehost, el backend/API/MCP en AWS y MySQL 5.7 permanece en Bluehost. La primera entrega debe poder probarse en servidor sin abrir operaciones de negocio no autenticadas.

## Decisión

- Organizar el workspace en `crates/core`, `crates/api`, `crates/mcp` y `apps/server`.
- Usar Tokio como runtime asíncrono, Axum para HTTP, Tower HTTP para middleware y SQLx para MySQL.
- Exigir TLS con verificación de CA e identidad del host (`VerifyIdentity`) en AWS→MySQL. La CA se inyecta como archivo protegido del ambiente; credenciales por variables/gestor de secretos. No se acepta fallback a conexión sin cifrar.
- Limitar el pool a 10 conexiones por instancia y solicitudes generales a 1 MiB. Los límites de rutas de login, operaciones, archivos y concurrencia se añadirán por caso de uso.
- Aceptar un solo origen CORS HTTPS configurado por ambiente; para desarrollo local se permite HTTP solo en `localhost`, `127.0.0.1` o `::1`.
- En la primera entrega, exponer únicamente `GET /health/live` y `GET /health/ready`. Readiness verifica `SELECT 1` y no devuelve detalles de conexión. No se montan rutas de usuario, sesiones, login, MCP ni módulos de negocio.
- Mantener el listener en `127.0.0.1:8080` por defecto para facilitar un proxy/túnel local. Cualquier override de ambiente debe quedar en red privada.

## Consecuencias

- El servidor puede iniciar aunque MySQL esté temporalmente inaccesible; readiness responde no disponible y el pool intenta abrir conexiones al atender la comprobación.
- No se puede declarar el Core listo para uso real hasta completar autenticación TOTP, sesiones/CSRF, permisos, auditoría y administración de ajustes.
- MySQL 5.7 remoto y el certificado de Bluehost deben validarse en staging. Si Bluehost no admite TLS verificable, la app debe fallar cerrada y habrá que resolver la conectividad antes de desplegar.
- Las peticiones de negocio futuras deben aplicar autorización por operación/objeto, límites por usuario/cuenta y errores RFC 9457; MCP debe reutilizar los mismos casos de uso y controles.

## Fuera de esta decisión

Selección del frontend, mecanismo de sesión, proveedor y diseño de OAuth/MCP, almacenamiento compartido para cuotas, pipeline de despliegue, Typst y almacenamiento de objetos. Siguen pendientes conforme a `03-arquitectura-rust.md` y `09-despliegue.md`.
