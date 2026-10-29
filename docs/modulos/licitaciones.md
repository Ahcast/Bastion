# Módulo: Licitaciones

## Estado, fuente y alcance de la incorporación

Incorporación documental solicitada el 2026-10-27, después de contrastar la propuesta con `origin/main` en `17b95a00c7308b8d8bf6147fe3556a69a15bdad5`. Es un módulo nuevo de la arquitectura objetivo; no procede del informe Legacy ni del módulo Procesos. Posteriormente el usuario autoriza comenzar su desarrollo y precisa que desea infraestructura sin campos ni formularios precargados. El primer corte local incorpora crate, registro, migraciones, API y pantallas para expedientes y esquemas de captura versionados. Alcance exacto y pendientes en [infraestructura inicial](licitaciones-infraestructura.md) y [OpenAPI](licitaciones.openapi.yaml). No acredita despliegue ni módulo completo.

Fuente: [propuesta v1.2 aportada](licitaciones-propuesta-v1.2.md), conservada sin cambios. Esta ficha registra su revisión de compatibilidad y las condiciones de integración; las decisiones posteriores confirmadas aquí prevalecen sobre los puntos pendientes o propuestas de la fuente. Las etiquetas `[U]` de la fuente son atribuciones del documento aportado; no convierten sus tablas, interfaces ni decisiones pendientes en contratos existentes o aprobaciones de implementación. En esta ficha, **verificado** describe código/documentos vigentes; **requisito de la propuesta**, **recomendación** y **pendiente** mantienen significados distintos.

La incorporación conserva Rust/AWS, React/Bluehost, MySQL/Bluehost y el monolito modular. Colaboración aporta biblioteca de esquemas y archivos autorizados. Se publicaron nombre editable y copias privadas de PDF/imágenes desde PC o Colaboración en los bloques legal y técnico; ver [evidencia de AWS y Bluehost](licitaciones-archivos-despliegue.md). Paquetes de plantillas, PDF final y adjudicación/Facturas siguen pendientes. Disponibilidad, activación y paridad son estados distintos.

## Propósito y límites funcionales

Cada licitación se diseña como expediente independiente con tres bloques: documentación legal, oferta económica por lote y documentación técnica por ítem. Las variables tienen definiciones reutilizables y valores aislados por expediente. El resultado previsto es un único PDF en orden portada, índice, legal, económica y técnica, con numeración continua e historial de versiones.

Las únicas relaciones de negocio propuestas son con Colaboración y Facturas. Procesos continúa recibiendo CSV de Scout; no se añade conversión automática desde Procesos, Cotizaciones, Clientes, Proveedores u Órdenes. El uso de infraestructura común y de `document-flows` no crea una dependencia de negocio adicional.

Quedan fuera del primer corte: captura de portales, lectura de pliegos mediante IA, presentación automática, validación jurídica automática, migración Legacy, firma digital y MCP operativo. No se inventan formularios, estados, columnas económicas ni reglas tributarias.

## Compatibilidad verificada y ampliaciones necesarias

Esta tabla conserva la revisión de partida anterior a implementar. El estado actual del primer corte prevalece en [infraestructura inicial](licitaciones-infraestructura.md): ya existen esquemas publicados/copias de esquemas; las copias binarias y PDF no están implementados.

| Área | Estado actual verificado | Integración de Licitaciones |
|---|---|---|
| Core y frontend | Registro central, allowlist y activación; sesión/CSRF, permisos, auditoría, settings y React/TypeScript/Vite compartidos. | Reutilizar el contrato común, sin identidad, menú de ajustes ni API paralelos. |
| Alcance de datos | Colaboración documenta una empresa por base; `empresa` y `persona:{id}` son ámbitos de autorización. Facturas no presupone multitenencia. | Aplicar autorización por expediente y objeto dentro de ese modelo. Las referencias a múltiples tenants en la fuente son futuras, no un requisito implementado ni autorización para añadir `tenant_id`. |
| Colaboración | Listado, subida, lectura, papelera/restauración/purga; no hay catálogo de variables, aprobación de plantillas, copias de expediente ni revisiones de trabajo. | Diseñar puertos internos autorizados y migraciones propias de Colaboración para las capacidades nuevas; no escribir sus tablas desde Licitaciones. |
| Tipos de archivo | `FilePolicy` acepta una lista de formatos reconocidos; `.typ` no está admitido por el validador actual. | No basta con añadir una extensión en Ajustes. Diseñar el tratamiento de plantillas como contenido restringido y su aprobación antes de ampliar la política. |
| Versiones | Colaboración comprueba hash y usa S3 privado; las versiones físicas de S3 no son un catálogo de revisiones funcionales. | Definir IDs de versión, procedencia y hashes estables mediante contrato, sin exponer claves S3 al cliente. |
| Typst | `DocumentService::render` recibe plantilla estática y JSON; límite actual de 20 segundos, dos trabajos simultáneos y lectura de PDF hasta 8 MiB. | Un paquete editable/versionado de plantillas requiere extensión segura. No asumir que los límites actuales admiten un expediente completo. |
| PDF final | El Core genera PDF y almacena objetos; no tiene ensamblador de anexos, índice conjunto ni foliado de PDF externos. | Diseñar esa capacidad compartida, sus límites y pruebas. Licitaciones conserva orden, contenido y reglas del expediente. |
| Facturas | Borradores y creación desde Cotizaciones/Órdenes; no hay emisión fiscal, pagos ni creación desde adjudicación. | Crear un contrato específico de origen adjudicación y un borrador vinculado, cuando se apruebe. No reutilizar un DTO de Cotizaciones como si fuera una licitación. |
| Coordinación | `crates/document-flows` ya coordina operaciones entre propietarios dentro de una transacción MySQL, con autorización y recibos. | Recomendación: ampliarlo para adjudicación → borrador, mediante un puerto propio de Facturas. No añadir un segundo coordinador ni trasladar reglas fiscales al Core. |
| MCP | Pospuesto según `../06-mcp.md`. | Mantener casos de uso reutilizables; no registrar herramientas ni depender de OAuth/MCP para el primer corte. |

## Decisiones confirmadas el 2026-10-27

- Posteriormente al primer despliegue, el usuario autoriza un nombre editable para cada expediente. Es independiente del ID y no incorpora los demás campos propuestos inicialmente. Ver [nombre editable](licitaciones-nombre.md).
- El usuario autoriza seleccionar PDF/imágenes de Colaboración o subirlos desde su PC en Documentación legal y Especificaciones técnicas para que formen parte del expediente. Son copias privadas independientes; ver [contrato de archivos](licitaciones-archivos.md).

- El usuario solicita infraestructura sin campos de expediente/lotes/ítems prefijados ni formularios/archivos iniciales. Los agregará según se necesiten. El primer corte usa sólo ID y metadatos técnicos de expediente; los datos de negocio se capturan desde los esquemas publicados individualmente en biblioteca.

- Las copias de trabajo aparecen solo dentro del expediente; no son archivos visibles de Colaboración.
- No se usan firmas digitales en el expediente ni se exige preservarlas en el PDF ensamblado. No se incorpora validación/firma digital como requisito del módulo.
- Cada adjudicación puede generar un único borrador de factura. La relación adjudicación/borrador requiere unicidad persistente, además de la clave idempotente de la petición.
- Los formularios y formatos económicos se incorporarán uno por uno a la biblioteca. Su contenido específico se define al incorporar cada recurso, sin exigir un catálogo completo por adelantado.
- El Administrador del sistema aprueba y publica las plantillas. El usuario solo modifica los valores de las variables admitidas; no cambia la plantilla, su estructura, columnas ni fórmulas.
- Solo una adjudicación registrada habilita la creación del borrador de factura vinculado; no se crea desde una oferta aún no adjudicada.
- Todo el PDF final se folia, incluidas portada e índice, y el índice se genera automáticamente con las páginas reales.
- Una licitación puede superar las 100 páginas. El diseño y las pruebas deben admitir ese volumen; no hay un máximo de 100 páginas acordado ni un máximo definitivo aprobado.
- Los archivos incorporados al expediente tienen un máximo de 20 MB cada uno. Se acuerda 500 MB como tope técnico para el expediente completo y, por separado, para su PDF final si se requiere un límite; no reducir contenido para ajustarlo a un límite menor. El PDF final tiene su propio presupuesto y no queda sujeto al máximo de 20 MB de los archivos de entrada.
- Solo se permite eliminar el expediente/oferta en estado Borrador; una oferta Enviada no se elimina. Esta regla pertenece a Licitaciones y no modifica la política global de Facturas.

Evidencia de código: [Colaboración](../../crates/modules/colaboracion/src/service.rs), [política de formatos](../../crates/modules/colaboracion/src/policy.rs), [migración de archivos](../../crates/modules/colaboracion/migrations/202610230001_colaboracion.sql), [servicio documental](../../crates/core/src/documents.rs), [conversiones de Facturas](../../crates/modules/facturas/src/conversion.rs) y [document-flows](../../crates/document-flows/src/lib.rs). Alcance operativo: [Colaboración](colaboracion-implementacion.md) y [Facturas](facturas-implementacion.md). Esta revisión es estática; no verifica un despliegue nuevo.

## Pantallas y flujos propuestos

Un listado paginado permite abrir cada expediente, siguiendo el patrón existente de Cotizaciones. Dentro del expediente se proponen Resumen, Variables, Documentación legal, Oferta económica, Especificaciones técnicas y PDF final. Variables y PDF final no son módulos globales ni entradas independientes del menú principal.

El flujo previsto es crear expediente → definir lotes/ítems → seleccionar recursos autorizados de Colaboración → completar variables y formularios → preparar los lotes ofertados y sus anexos → validar → generar una versión del PDF. La creación del único borrador vinculado requiere una adjudicación registrada. La acción de envío y las demás transiciones del expediente siguen pendientes de definición; Borrador y Enviada ya tienen una regla de eliminación confirmada.

Campos, columnas del listado, filtros, obligatoriedad y acciones concretas siguen pendientes de formularios reales. Cada pantalla reutiliza componentes, cliente HTTP/CSRF y tokens existentes, con estados de carga, vacío, error, conflicto y acceso denegado; protege cambios sin guardar. Las tablas económicas se desplazan dentro de su contenedor en móvil.

## Propiedad de recursos y copias

Colaboración conserva originales y las futuras definiciones/plantillas reutilizables; Licitaciones conserva valores, respuestas, asociación a lotes/ítems, copias independientes, revisiones y composición del expediente. Core presta almacenamiento privado y generación documental; no resuelve reglas económicas del módulo.

Los nombres `list_authorized_assets`, `get_asset_version` y `copy_asset_to_expedient` de la fuente son contratos candidatos, no funciones actuales. Antes de implementarlos se deben fijar autorización, versión/hash, límites, idempotencia, propiedad de la copia y respuesta ante eliminación o desactivación del origen. El usuario necesita lectura sobre el origen y edición del expediente para copiarlo; después se aplica el permiso del propietario de cada copia.

Las copias son visibles solo en el expediente por confirmación del usuario. Recomendación técnica: conservarlas junto a los PDF finales bajo propiedad de Licitaciones, en un prefijo privado separado y autorizado por el Core, sin someterlas a la papelera automática de los originales de Colaboración. El prefijo exacto e IAM se diseñarán antes del despliegue. El borrado del original no debe destruir una copia independiente ya guardada ni una versión final; la biblioteca conserva su política vigente de papelera.

Pendiente: el alcance de acceso a las copias entre usuarios autorizados y qué ocurre con nuevas copias si Colaboración está deshabilitado. Recomendación: exigir Colaboración activo para consultar/copiar originales y permitir abrir instantáneas propias ya guardadas según permisos de Licitaciones. No habilitar el módulo mediante una dependencia implícita.

## Variables, formularios y cálculo económico

La biblioteca guarda definiciones versionadas; cada expediente guarda sus valores. Cambiar una definición no modifica expedientes o PDF anteriores. El usuario incorporará formularios legales y formatos económicos uno por uno: tipos, nombres, fuentes, valores iniciales, obligatoriedad, columnas, fórmulas, impuestos y redondeos se especificarán al revisar cada recurso real. La carga de un archivo no interpreta ni aprueba automáticamente esas reglas. No se añade un catálogo genérico de negocio al Core.

El Administrador del sistema aprueba y publica cada versión de plantilla y administra su estructura y esquema. El usuario con autorización sobre el expediente solo introduce/cambia los valores de las variables editables declaradas; no modifica código Typst, disposición, columnas, fórmulas ni definiciones de variables. Los valores calculados siguen siendo de solo lectura. Tener `editar` en Colaboración o Licitaciones no concede a un rol personalizado la facultad de aprobar/publicar plantillas. Los formularios de captura y las celdas económicas editables deben corresponder a variables expresamente declaradas en el esquema aprobado.

Rust resuelve tipos y dependencias, rechaza ciclos/faltantes y calcula valores derivados de solo lectura. Las fórmulas de cada plantilla requieren un lenguaje de operaciones permitido; no se ejecuta código aportado ni se evalúan expresiones JavaScript, SQL o Typst para calcular dinero. Se recomienda un registro explícito de fórmulas admitidas para la configuración de plantillas por el Administrador; el usuario final no edita fórmulas.

La oferta económica es una tabla lógica por lote, no un editor Excel ni un importador de `.xlsx` ya comprometido. Columnas, formato oficial, cantidades parciales, impuestos, descuentos, moneda, precisión y redondeos se definirán para cada plantilla que se incorpore. Dinero viaja como cadenas decimales y usa aritmética exacta en Rust; el uso de `DECIMAL` o enteros escalados en persistencia debe fijarse con las precisiones y convenciones existentes. No asumir multimoneda ni exponer costos/márgenes internos en la oferta oficial.

La conversión requerida es número → letras (`number_to_words`). Español, moneda, centavos, negativos, magnitud máxima y redondeo se definirán antes de implementarla y se probarán con ejemplos aceptados. Los formularios legales y anexos técnicos dependen del pliego; comprobar presencia/tipo no certifica cumplimiento legal o técnico.

## PDF único y versiones

La composición propuesta autoriza recursos, congela datos/plantillas/anexos y sus hashes, genera las piezas con Typst y combina PDF externos. Por confirmación del usuario, todas las páginas llevan foliado continuo, incluidas portada e índice; el índice es automático y apunta a las páginas reales de las secciones. Recomendación: calcular el índice con las longitudes reales de las piezas y repetir la composición si cambia su longitud, hasta un límite definido; fallar de forma controlada si no converge. El usuario confirmó que los folios se colocan en el margen existente, incluso si se superponen al contenido, y se comprueban también con páginas externas.

Conservar el original externo byte por byte en su objeto privado. El PDF ensamblado es un derivado: preservar el contenido visible no garantiza conservar bytes, firmas, campos interactivos, enlaces, marcadores o etiquetas de accesibilidad. Acordar qué propiedades deben conservarse y probarlas con documentos representativos; no rasterizar ni aplanar silenciosamente.

Pendientes: biblioteca/ejecutable de ensamblado, posición y formato del número sin cubrir contenido, orientaciones/tamaños mixtos y archivos protegidos. El usuario confirma que no se usan firmas digitales ni se requiere conservarlas: no se bloquea la composición por exigir preservación criptográfica ni se incorpora un servicio de firma/validación. Si se aporta un archivo firmado, el original sigue intacto y no se promete validez de su firma en el PDF derivado. La diferencia entre composición y preservación de firmas está descrita en la [documentación de Adobe](https://helpx.adobe.com/sign/web/config/digital-signatures/digital_signatures_preservation.html).

Las plantillas nuevas requieren aprobación/publicación del Administrador del sistema, recursos fijados por hash, raíz privada, acceso mínimo a archivos, bloqueo de red/importaciones no autorizadas y presupuestos de CPU/memoria/tiempo. Los usuarios aportan únicamente valores al esquema aprobado; una variable de texto no se interpreta como código Typst. La raíz de Typst delimita lectura de archivos, pero no acredita por sí sola todo ese aislamiento; véase [rutas y raíz en Typst](https://typst.app/docs/reference/foundations/path/). No ejecutar directamente `.typ` subidos ni ampliar permisos del compilador existente por inferencia.

Límites de peso confirmados: **20 MB por archivo de entrada** (carga, anexo o copia incorporada desde la biblioteca), **500 MB como tope técnico del expediente completo** y **500 MB como tope técnico independiente del PDF final** cuando sea necesario acotarlos. Convención del contrato nuevo: MB decimales, 20 000 000 y 500 000 000 bytes respectivamente, sin confundirlos con los MiB de otros contratos existentes. El tamaño se valida sobre los bytes reales, no sobre la longitud base64 ni el tamaño declarado por el navegador. Definir en el contrato la contabilización de revisiones/instantáneas; no eliminar historial ni originales para liberar espacio por inferencia.

Estos límites pertenecen a Licitaciones y no cambian la política global vigente de Colaboración. Un original de biblioteca permitido allí pero mayor de 20 MB no se incorpora a Licitaciones sin resolver explícitamente ese exceso; no se comprime ni recorta silenciosamente. La generación debe producir el contenido requerido y comprobar el resultado contra su presupuesto propio de hasta 500 MB, sin aplicar al PDF final el máximo individual de entrada.

Siguen pendientes resolución/píxeles de imágenes, tiempo/memoria de generación y cantidad de trabajos simultáneos, a dimensionar mediante mediciones. Una licitación puede superar las 100 páginas: no imponer ese número como tope. La cantidad de páginas por sí sola no determina el peso ni el costo de procesamiento; verificar expedientes representativos de más de 100 páginas, con texto y anexos escaneados. No se ha aprobado un máximo numérico de páginas.

Los límites de Colaboración, almacenamiento y PDF no son intercambiables. El almacenamiento común actual tiene un techo de 100 MiB por operación y el renderer lee hasta 8 MiB: los 500 MB aprobados requieren adaptar y verificar el procesamiento/almacenamiento para Licitaciones, sin elevar globalmente los límites de los demás módulos. Recomendar procesamiento acotado por flujo o archivos temporales en lugar de acumular el expediente/base64 completo en memoria. Si el trabajo no cabe en el presupuesto HTTP existente, diseñar estado, consulta, reintentos y descarga autorizada de un trabajo persistente antes de aprobar una ejecución en segundo plano. La incorporación documental no acredita capacidad operativa de 500 MB.

Una versión final congela datos, fuentes, revisiones y resultado; cualquier cambio produce otra versión. Reproducibilidad significa mismo contenido lógico con las mismas entradas/versiones, sin promesa de igualdad binaria. Una caída entre MySQL y S3 se recupera mediante recibo/estados y limpieza controlada; no existe transacción atómica entre ambos sistemas.

## Integración futura con Facturas

Recomendación: crear explícitamente un **borrador** desde una adjudicación aprobada mediante `document-flows`, usando un puerto de Facturas con datos tipados y origen de Licitaciones. El coordinador comprueba acceso a ambos módulos, estado, versión, activación y migraciones; Facturas valida y calcula su documento. Licitaciones no inserta en tablas de Facturas ni asigna números fiscales.

El contrato actual de conversiones exige `ver` y `editar` en ambos dominios. La integración debe conservar esas comprobaciones y la autorización por objeto; no reducirla al `editar` genérico de la propuesta. La adjudicación no genera automáticamente una factura, no registra pago ni significa emisión fiscal.

Confirmado: un único borrador por adjudicación. El coordinador debe respaldar esa unicidad en MySQL y resolver intentos simultáneos o reintentos con claves distintas sin crear un segundo borrador; la lectura del resultado conserva autorización sobre ambos objetos. La clave idempotente sigue ligada al actor/contenido y rechaza su reutilización con otra solicitud.

Confirmado: debe existir una adjudicación registrada para habilitar la acción de crear el borrador; no basta presentar/enviar la oferta. Esta condición no supone creación automática ni emisión fiscal. Siguen pendientes cliente manual/instantánea, líneas/cantidades/impuestos, moneda y relación con lotes ganados; no se diseña facturación parcial de varios documentos por adjudicación. También debe acordarse el tratamiento de la relación si se permite borrar/corregir el borrador de Facturas. La regla de eliminación de ofertas de Licitaciones no decide ese caso. Emisión/pagos dependen de que Facturas implemente y valide esas capacidades. Facturas deshabilitado bloquea esa acción, sin bloquear la consulta y preparación del expediente.

## Estado y eliminación del expediente/oferta

El usuario confirma que la eliminación solo se permite en Borrador. Una oferta Enviada no se elimina, tampoco mediante una solicitud directa a la API con permiso `eliminar`. El servidor comprueba permiso y estado en la misma operación transaccional, coordinada con la transición de envío para impedir una carrera entre enviar y borrar. La interfaz muestra el motivo del bloqueo; ocultar el botón no sustituye la regla del servidor.

La eliminación autorizada de un borrador requiere confirmación y auditoría y afecta solo sus copias/archivos propios, sin borrar originales de Colaboración. La versión presentada/enviada y sus archivos no se eliminan como atajo para borrar la oferta. Las reglas de sustitución/corrección después de Enviada, el resto de transiciones y el tratamiento de referencias a Facturas siguen pendientes; no se introduce un estado nuevo ni una acción de anulación por inferencia.

## Registro, permisos, Ajustes, API y datos

`licitaciones` es el ID candidato. Versión de contrato, compatibilidad, dependencias y alcances de permiso se fijarán antes de crear el registro ejecutable. La incorporación documental no agrega el módulo a allowlists, navegación, permisos efectivos ni Ajustes en runtime.

Aplicar `ver` para consultar/descargar, `editar` para crear/actualizar valores admitidos y generar una versión persistente, y `eliminar` para borrar solo en Borrador, con confirmación/auditoría y comprobación de estado en servidor. Publicar/aprobar plantillas queda reservado al Administrador del sistema. Revisar los alcances por recurso antes de asignarlos; no crear roles ni conceder permisos automáticamente. Las confirmaciones no sustituyen autorización. Reutilizar sesión, CSRF, origen exacto, validación de campos y auditoría del Core.

La futura sección Ajustes > Licitaciones debe declarar campos, alcance y permisos mediante el almacén común. Sus campos siguen pendientes; no duplicar configuración S3, SMTP, Empresa/SAR o seguridad del Core.

Las tablas, DTO y rutas de los apartados 10 y 11 de la propuesta son candidatos. Deben reflejar las decisiones de esta revisión; no son un esquema aprobado. No se crea OpenAPI con campos inventados: completar operaciones, permisos, DTO, límites y errores antes de implementar, usando `/api/v1`, `application/problem+json`, claves idempotentes y `ETag`/`If-Match` cuando aplique.

Licitaciones conserva sus tablas y migraciones/ledger propios. Los contratos fijan qué relaciones son suyas y cuáles son del coordinador; no duplicar relación e idempotencia en dos propietarios. Las relaciones con otros módulos usan referencias/puertos explícitos. MySQL 5.7, migraciones controladas con `migrate`, sin DDL en `serve`; fechas UTC y visualización `America/Tegucigalpa`.

## Aceptación y entrega

El catálogo de formularios/formatos se completará de forma incremental, uno por uno en la biblioteca. Cada plantilla debe tener esquema, reglas, pruebas y versión aprobados por el Administrador antes de usarse; no es necesario disponer de todos los formularios para diseñar la infraestructura reutilizable. Antes de implementar cada operación deben cerrarse sus campos, permisos, límites, settings y contrato OpenAPI. La revisión de arquitectura no cierra esos detalles.

Los escenarios verificables incluyen:

1. Sesión, permiso, objeto y módulo: denegación sin acceso, sin enumerar recursos; deshabilitar conserva los datos.
2. Copias y variables: editar un expediente o cambiar/eliminar el original no altera copias independientes o PDF ya guardados de otro expediente.
3. Tipos y cálculos: errores/faltantes/ciclos rechazados; totales y número a letras coinciden con los ejemplos aprobados.
4. Archivos/plantillas: MIME falso, rutas/importaciones indebidas, exceso de recursos y anexos de ítems ajenos rechazados. Solo el Administrador aprueba/publica versiones; un usuario no cambia plantilla, columnas, fórmulas ni variables calculadas aunque envíe esos campos directamente al API. Sus valores de texto no se ejecutan como código.
5. Documento: todas las páginas se folian, incluidas portada/índice; el índice automático apunta a páginas reales, también en expedientes de más de 100 páginas con anexos representativos. Contenido externo y propiedades acordadas se conservan; archivos firmados/protegidos siguen política explícita. La capacidad y los máximos técnicos se verifican con mediciones antes de declararse operativos.
6. Versiones: edición obsoleta devuelve `412`; reintentar copia/generación/facturación no duplica efectos; una edición concurrente no mezcla el snapshot.
7. Fallos MySQL/S3: operación recuperable, sin objetos disponibles huérfanos ni borrados de fuentes/expedientes vecinos.
8. Facturas: sin adjudicación registrada la acción se rechaza; con ella solo crea un borrador por adjudicación, incluso con peticiones concurrentes y claves distintas. Conserva autorización de ambos dominios, origen e idempotencia. Facturas deshabilitado rechaza la acción.
9. Migración/release: MySQL 5.7 aislado, tablas previas intactas, restauración y deshabilitación sin pérdida; no regresión de Core/Colaboración/Facturas.
10. UI: recorrido completo con expediente sintético, móvil, teclado, conflictos y protección de cambios; descarga privada y errores de dependencias visibles.
11. Eliminación: un borrador con permiso, confirmación y estado vigente puede eliminarse; una oferta Enviada se rechaza en API y no pierde su versión ni archivos. Enviar y borrar concurrentemente nunca permiten eliminar una oferta ya enviada.
12. Biblioteca incremental: incorporar una plantilla aprobada no altera versiones anteriores ni expedientes existentes; el formulario de captura admite solo sus variables editables.
13. Peso: aceptar un archivo de entrada de hasta 20 000 000 bytes y rechazar el exceso también al copiar desde biblioteca; no aplicar ese límite al PDF final. Verificar los presupuestos independientes de 500 000 000 bytes para expediente y PDF, con error controlado y sin truncar contenido, degradar calidad o publicar un resultado parcial. Probar esos volúmenes y su concurrencia con almacenamiento privado y descarga autorizada antes de declarar soporte operativo.

Recomendación de etapas: (1) contratos e infraestructura de biblioteca con incorporación individual de recursos; (2) expediente, lotes/ítems y copias; (3) variables, formularios y tabla económica según las plantillas disponibles; (4) PDF único y versiones; (5) adjudicación y borrador vinculado. Las etapas son planificación y no autorizan implementación. Cada entrega futura requiere checks/CI, migración controlada, respaldo, artifacts del commit y verificación. Inicialmente deshabilitado; activación administrativa independiente. MCP permanece pospuesto.

## Decisiones pendientes

| Decisión | Efecto |
|---|---|
| Campos de expediente/lotes/ítems; una o varias ofertas/revisiones y demás transiciones | Datos y pantallas; eliminación solo Borrador, bloqueo de Enviada y requisito de adjudicación ya confirmados. |
| Esquema de cada formulario legal, obligatoriedad y AcroForms, al incorporarlo individualmente | Plantillas y validación; no se requiere catálogo completo por adelantado. |
| Columnas/fórmulas oficiales, moneda, impuestos, precisión y redondeo, por formato incorporado | Cálculo y correspondencia con Facturas. |
| Tipos/nombres de variables y reglas de número a letras | Catálogo versionado y resultados de documentos. |
| Detalles técnicos del ciclo de versiones/aprobación y restricciones de las variables de cada plantilla | Administrador aprueba/publica y el usuario solo edita valores de variables, ya confirmado. |
| Acceso compartido a las copias del expediente y nuevas copias con Colaboración deshabilitado | Permisos y dependencia; visibilidad solo en expediente ya confirmada. |
| Posición/formato del folio, tamaños/orientaciones y PDF protegidos | Todas las páginas foliadas e índice automático ya confirmados; firmas digitales no requeridas. |
| Mapeo a Facturas y tratamiento de corrección/borrado de su borrador | Requiere adjudicación registrada y único borrador por adjudicación, ya confirmado. |
| Retención y referencias a Facturas; correcciones/sustituciones posteriores al envío | Eliminación solo en Borrador y prohibición en Enviada confirmadas; proteger documentos enviados y originales. |
| Contabilización de revisiones/instantáneas, píxeles, ítems, tiempo/memoria, concurrencia y posible trabajo persistente | Confirmados 20 MB por archivo de entrada, tope técnico de 500 MB para expediente y de 500 MB para PDF final; medir y adaptar el servicio actual para soportarlos. Más de 100 páginas requerido, sin máximo de páginas aprobado. |
| Settings concretos y alcance; versión/compatibilidad del módulo | Registro y criterio de configuración completa. |

Se conservan en la fuente los detalles candidatos de diseño. Cualquier respuesta posterior se registra aquí y en los contratos afectados antes de implementar.
