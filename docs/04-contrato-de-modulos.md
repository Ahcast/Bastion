# Contrato de módulos

## Integración del frontend

El frontend usa React + TypeScript estricto y Vite (ADR-0002). Cada pantalla de módulo reutiliza cliente HTTP/CSRF, componentes, tokens visuales y navegación del Core; su código se carga bajo demanda. Registrar pantallas conforme al catálogo y permisos recibidos del API. Los permisos visibles no autorizan operaciones: Rust vuelve a verificarlos por recurso. Las vistas previas de Inicio no activan módulos ni llaman endpoints futuros.

## Objetivo

Permitir que un módulo se conecte al core y contribuya capacidades sin duplicar identidad, permisos, navegación, API, MCP ni Ajustes.

## Identidad de módulo

Cada módulo declara:

| Propiedad | Requisito |
|---|---|
| `id` | Identificador estable, único y versionable; se define al diseñar cada módulo. |
| nombre/descripcion | Etiqueta visible y propósito. |
| versión | Versión de código y compatibilidad del contrato. |
| estado | Disponible, habilitado o deshabilitado. |
| dependencias | Módulos/servicios requeridos, si los hay. |
| datos y migraciones | Tablas/índices propios y estrategia de actualización. |
| permisos | Una capacidad requerida por operación (`ver`, `editar` o `eliminar`), alcance del recurso/tenant y campos modificables. `editar` autoriza altas y actualizaciones; `eliminar` es independiente. |
| navegación | Entradas/rutas/pantallas aportadas. |
| settings | Grupos, campos, esquema, alcance (`global`, `empresa` o `usuario`) y permisos `ver`/`editar`, administrados bajo Ajustes. |
| API | Operaciones HTTP aportadas, DTO explícitos/versionados, permisos y alcances, límites y soporte de idempotencia/concurrencia cuando aplique. |
| MCP | Herramientas aportadas, esquema de argumentos, alcance, límites por invocación, nivel de riesgo y si requieren confirmación. |
| documentos PDF | Documentos generados por el módulo, datos de entrada, permiso requerido y referencia a su plantilla/versionado Typst. |
| auditoría | Eventos de negocio que el core registra. |

## Registro y presentación

El core construye el catálogo a partir del registro explícito de módulos que estén incluidos en el release. Debe ser válido un registro con solo el core, antes de conectar módulos de negocio. De ahí genera navegación permitida y secciones de Ajustes. El cliente no puede habilitarse a sí mismo ni declarar permisos efectivos.

El registro debe distinguir al menos entre módulo incluido/disponible y módulo habilitado en un ambiente. Un módulo que aún no se haya publicado o que esté deshabilitado no aparece en navegación ni expone rutas API o herramientas MCP. La activación se controla desde el servidor y se audita; no la decide el cliente.

## Settings

- El módulo define schema y validación; el core almacena los settings en una tabla común, indexados por `module_id`, alcance, clave y versión. El JSON se valida en Rust y los secretos se cifran antes de persistirlos.
- Cada setting declara si es global, de empresa o de usuario. Las operaciones de lectura y edición comprueban el permiso correspondiente y el actor solo accede a los datos de su alcance; las credenciales personales no se comparten entre usuarios.
- Los permisos de settings de módulo se asignan como `ver`/`editar` por rol, módulo y alcance. Los roles nuevos empiezan sin acceso; el Administrador del sistema tiene acceso total. El alcance `usuario` limita cada actor a sus propios valores.
- Campos sensibles se etiquetan como secretos, se cifran y tienen lectura redactada. La respuesta indica si hay un secreto guardado, sin devolverlo; una cadena vacía al editar conserva el valor existente cuando el formulario así lo indique.
- El módulo puede definir valores iniciales seguros y una función de validación/migración.
- El core solo presenta settings a actores autorizados y emite auditoría. Todo ajuste perteneciente al Core es exclusivo del Administrador del sistema; los settings propios de usuario y los settings aportados por módulos conservan sus alcances y permisos específicos.
- Todo módulo conectado debe tener su propia sección bajo Ajustes y declarar settings de módulo antes de considerarse configurado para uso. Si aún no se han definido sus campos, se marca pendiente en su ficha y no se inventan valores; el módulo no se da por terminado hasta acordarlos.

## Permisos

Permisos acotados por módulo y acción mediante la matriz común `ver`, `editar` y `eliminar`. `editar` autoriza altas, actualizaciones y comandos de dominio que cambian datos/estado sin eliminar registros. Cada caso de uso/API/MCP declara cuál requiere, su alcance de registros y sus campos editables. Se comprueba el acceso al objeto afectado, no solo el permiso genérico de pantalla. El servidor es autoridad aunque la interfaz o el cliente MCP oculten opciones. Denegar por defecto; las confirmaciones no sustituyen al permiso.

## Límites de operación

Cada endpoint y herramienta declara la categoría de costo y los límites aplicables: intentos, tamaño de petición/archivo, cantidad de filas, rango de fechas, tamaño de lote, timeout y concurrencia. El módulo valida sus reglas de dominio; el core aplica los topes transversales y límites por actor antes de consultar o modificar grandes volúmenes. Los límites por IP que configure Cloudflare Free no reemplazan los límites de aplicación.

Las operaciones de reporte/exportación declaran límites de tiempo y concurrencia y el resultado controlado al alcanzarlos. Si una ficha propone ejecución en segundo plano, también define estado, consulta/descarga autorizada y el flujo visible para el usuario. Las cuotas y valores concretos se documentan en cada ficha y no alteran flujos normales de forma implícita.

## API y MCP

Un módulo registra handlers en el API común y herramientas en el MCP común. Ambos adaptadores llaman los casos de uso del módulo y reutilizan autorización, validación, límites, auditoría, CSRF cuando aplique e idempotencia. Las operaciones HTTP se documentan en OpenAPI antes de implementarse. No se duplican reglas de negocio por protocolo ni se exponen permisos adicionales a un agente de IA.

## Documentos PDF

Los PDFs se generan con Typst a través del servicio compartido del core y se guardan, como los adjuntos privados, en almacenamiento privado de objetos en AWS. El módulo es dueño de los datos y del contenido de dominio del documento; declara qué PDFs ofrece, el permiso para generarlos y las entradas necesarias. La ubicación y versionado de las plantillas y la forma de integrar Typst se definen antes de implementar el primer documento. Los objetos no son públicos y toda lectura o descarga comprueba identidad, permisos sobre el objeto y límites de recursos. Los archivos no caducan automáticamente mientras exista el registro asociado; eliminar el registro requiere el permiso de eliminación y confirmación, y el borrado de sus archivos queda auditado.

## Datos e interoperabilidad

Los módulos son dueños de sus tablas. Las dependencias entre dominios se expresan mediante casos de uso/eventos internos o contratos explícitos. Las operaciones entre módulos usan transacciones o patrones de consistencia definidos cuando afectan varios dominios.

## Versionado y compatibilidad

El core valida versión del contrato, identificador único, dependencias y colisiones de rutas/permisos/settings. Un módulo no se habilita si no satisface compatibilidad o migración requerida. Cada módulo debe tener versión de release y migraciones propias identificables para poder incorporarlo y validarlo en una etapa independiente.

## Release por módulo

- Desplegar primero una versión core-only en el ambiente de prueba y verificar el core sin módulos de negocio.
- Incorporar un módulo por release; revisar su compatibilidad con el core ya desplegado y ejecutar sus migraciones antes de habilitar su uso.
- La habilitación debe poder controlarse por ambiente para probar un módulo antes de habilitarlo en producción.
- Deshabilitar o revertir un módulo no elimina sus datos ni deshace automáticamente migraciones con pérdida potencial de datos.
- En el monolito modular inicial, un release de módulo actualiza el artefacto común del backend. La inclusión del módulo en el release y su habilitación por ambiente son controladas por el servidor.
- La configuración de despliegue permite o excluye módulos por ambiente; dentro de ese conjunto, el Administrador del sistema puede activar/desactivar cada módulo desde `Ajustes > Core > Módulos`. La interfaz no puede habilitar módulos ausentes, excluidos por ambiente o con migraciones incompatibles/no aplicadas.
- El mismo artefacto puede ejecutarse con Core solo o con los módulos habilitados para ese ambiente. Los módulos deshabilitados no registran navegación, rutas API, permisos efectivos ni herramientas MCP.
- Cada cambio de estado se autoriza en el servidor y queda auditado con actor, módulo, ambiente, estado anterior/nuevo y fecha. Desactivar bloquea nuevas operaciones y oculta el módulo sin eliminar sus datos.
- Se incorpora y valida un módulo por release. Artefactos cargables en runtime y procesos desplegables independientes quedan fuera del primer release.

## Pendiente de decisión

Forma técnica de representar la lista permitida por ambiente y el estado activo elegido en Ajustes, recuperación ante cambios incompatibles, compatibilidad semántica, eventos internos y política de datos al retirar un módulo. El control combinado de despliegue y Ajustes, la autorización exclusiva del Administrador y la auditoría están confirmados; artefactos cargables en runtime no forman parte del primer release.
