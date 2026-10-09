# Contrato de incorporación de módulos

1. Declarar identificador, versión y dependencias.
2. Documentar tablas, datos propios y acciones públicas.
3. Registrar permisos y políticas de autorización por acción y registro; no basta ocultar el menú.
4. Aportar rutas REST y herramientas MCP que delegan en los mismos servicios de aplicación.
5. Declarar eventos, jobs, límites, idempotencia y auditoría.
6. Mantener migraciones y configuración propias; no escribir tablas de otros módulos directamente.
7. Aportar rutas frontend y etiquetas en español; consumir Core y shared.
8. Definir habilitación, deshabilitación, recuperación y pruebas de acceso.

El registro será explícito y versionado mediante configuración/registro explícito de módulos Go. No se instala código subido desde la interfaz. No crear dependencias circulares: la composición conecta los módulos y Core consume contratos, no implementaciones de negocio.

## Registro físico al implementar

La composición raíz Go registra explícitamente módulos, rutas REST, herramientas MCP y migraciones. Angular compone rutas desde src/app. No instalar código subido desde la interfaz.

Las migraciones mantendrán un orden global inequívoco. La habilitación no equivale a migrar ni la deshabilitación equivale a borrar.

## Pruebas

Pruebas Go junto a cada paquete separan escenarios comunes y de negocio; integración en backend/tests/integration. Las pruebas de integración cubren permisos, duplicados, auditoría y coherencia REST/MCP. Se usan los contratos del Core sin duplicar seguridad.


