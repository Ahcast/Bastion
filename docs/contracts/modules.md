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



## Composición ejecutable del Core

registry.Module declara versión, dependencias, permisos y callbacks Routes/Tools. Composición cmd/server registra módulos implementados; Validate rechaza dependencias ausentes/deshabilitadas y ciclos. Routes recibe ServeMux; envolver handlers con api.Operation(service, logger, action) para obtener Actor validado y respuestas uniformes. El servicio del módulo vuelve a autorizar permiso/recurso/estado con Core.Authorize y persiste cambios/auditoría/outbox en la misma transacción. No confiar en el contexto de UI como permiso. Registrar permisos nuevos mediante migración global y ampliar el catálogo permitido explícitamente antes de asignarlos. Tools usa SDK oficial y el mismo servicio de aplicación; no HTTP a sí mismo.

internal/composition registra Customers, Suppliers, Processes, Quotes y PurchaseOrders. RegisterPermissions amplía el catálogo técnico permitido sin otorgar permisos a ningún rol/usuario; migración inserta permisos y un administrador asigna roles explícitamente. Perfil SPA incluye enabled_modules para guards/navegación. MCP propaga actor autenticado al contexto de callbacks; ActorFrom funciona tanto en REST como MCP. Cada tool vuelve a autorizar scopes y permisos actuales. Esquemas MCP explícitos mantienen IDs/versiones string; no confiar en la inferencia de tags Go version,string.

DocumentPolicies registra políticas de entidad en Core sin importar módulos. BindDocument escribe el vínculo bajo ambas políticas/versión; DocumentAccess exige permiso técnico y permiso de entidad registrada. Entidades/módulos no registrados fallan cerrados. EmitEvent persiste hechos en domain_events junto a la transacción del propietario; Enqueue/outbox se reserva a handlers implementados, evitando jobs desconocidos. Desactivar módulo conserva sus datos, bloquea rutas/tools y no registra su política de documentos al siguiente arranque.

Processes registra scopes técnicos de ingestión separados de las capacidades MCP de lectura; esta habilitación no permite escrituras de otros módulos.

Quotes recibe interfaces de snapshot transaccional de Clientes/Procesos por composición. No anida transacciones ni modifica sus tablas; registro de conversión retirado al deshabilitar cualquiera de los módulos requeridos.

PurchaseOrders depende sólo de Core en el registry y recibe interfaces transaccionales opcionales de Customers y Suppliers por composición. Con catálogos deshabilitados continúa admitiendo snapshots manuales; si una entrada incluye `customer_id` o `supplier_id` sin el módulo propietario disponible, falla cerrado. `BASTION_PURCHASE_ORDERS_ENABLED` controla su habilitación sin borrar datos ni otorgar permisos implícitos.
