# Contrato de incorporación de módulos

1. Declarar identificador, versión y dependencias.
2. Documentar tablas, datos propios y acciones públicas.
3. Registrar permisos y Policies por acción y registro; no basta ocultar el menú.
4. Aportar rutas REST y herramientas MCP que delegan en las mismas Actions.
5. Declarar eventos, jobs, límites, idempotencia y auditoría.
6. Mantener migraciones y configuración propias; no escribir tablas de otros módulos directamente.
7. Aportar rutas frontend y etiquetas en español; consumir Core y shared.
8. Definir habilitación, deshabilitación, recuperación y pruebas de acceso.

El registro será explícito y versionado mediante configuración/Service Providers. No se instala código subido desde la interfaz. No crear dependencias circulares: la composición conecta los módulos y Core consume contratos, no implementaciones de negocio.

## Registro físico al implementar

El backend usará `config/modules.php` y Service Providers de módulo; cada provider registrará sus rutas y migraciones. `routes/ai.php` compondrá el servidor MCP y el registro de herramientas. El frontend compondrá rutas y navegación desde la raíz de `src/app`, sin importar módulos desde servicios de Core. Estos archivos ejecutables aún no se crean en este cambio documental.

Las migraciones mantendrán un orden global inequívoco. La habilitación no equivale a migrar ni la deshabilitación equivale a borrar.

## Pruebas

`bluehost/backend/tests/Core` y `tests/Modules` separan escenarios comunes y de negocio. Las pruebas de integración cubren permisos, duplicados, auditoría y coherencia REST/MCP. Se usan los contratos del Core sin duplicar seguridad.
