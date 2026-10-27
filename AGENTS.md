# Guía para agentes del proyecto

## Objetivo

Construir una nueva aplicación en Rust que conserve los módulos, procesos, campos, botones y resultados importantes de CastilloIT, con arquitectura extensible de core y módulos, controles de seguridad centralizados y API/MCP integrados.

## Orden de referencia

1. `docs/00-vision-y-alcance.md`
2. `docs/01-aplicacion-actual-y-paridad.md`
3. `docs/02-core-y-ajustes.md`
4. `docs/03-arquitectura-rust.md`
5. `docs/04-contrato-de-modulos.md`
6. `docs/modulos/README.md` y la ficha del módulo que se implemente
7. `docs/05-api.md`, `docs/06-mcp.md`, `docs/07-datos-y-reglas.md`, `docs/08-seguridad-y-permisos.md` y `docs/09-despliegue.md`
8. `docs/10-criterios-de-aceptacion.md`

## Reglas de trabajo

- Tratar el informe actual como fuente de paridad funcional. Mantener lo observado y marcar explícitamente dudas, decisiones pendientes e inferencias.
- No inventar campos, reglas, botones, permisos ni settings como si existieran en el sistema actual. Las mejoras nuevas deben identificarse como propuestas y acordarse.
- Los módulos aparecen en la sección central Ajustes mediante el registro definido en `docs/04-contrato-de-modulos.md`.
- El core es dueño de la autenticación, autorización, catálogo de módulos, navegación y servicios compartidos. Los módulos son dueños de sus capacidades y reglas de dominio.
- API y MCP comparten identidad, autorización, auditoría y límites de dominio; MCP no obtiene acceso directo a SQL, archivos o secretos.
- Preservar la separación de despliegue acordada: frontend Bluehost; backend/API/MCP en AWS; MySQL Bluehost, salvo decisión explícita posterior.
- No almacenar secretos en el frontend ni en los documentos. Usar variables/gestor de secretos para valores de entorno.
- Al cambiar una decisión arquitectónica, actualizar los documentos afectados y registrar la decisión en `docs/README.md`.
- Antes de implementar o publicar, sincronizar las referencias de `origin/main` y revisar divergencias y trabajo local. Conservar un respaldo recuperable antes de reconciliar; comparar los cambios con la versión publicada para evitar restaurar copias antiguas de módulos ya corregidos.
- Publicar commits acotados en `main`, según `docs/09-despliegue.md`. Verificar compilación, pruebas y CI; construir artifacts no equivale a desplegar ni activar un módulo.

## Estado

El repositorio implementa el Core con autenticación, sesiones, ajustes, autorización y auditoría, frontend React y módulos de negocio. Las fichas de `docs/modulos/` y `docs/09-despliegue.md` registran el alcance y despliegue de cada módulo; disponibilidad, activación y paridad son estados distintos. MCP sigue pospuesto conforme a `docs/06-mcp.md`. Lo marcado `Pendiente` requiere definición o verificación antes de considerarse requisito cerrado.
