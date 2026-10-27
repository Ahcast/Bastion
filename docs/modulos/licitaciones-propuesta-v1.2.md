# BASTION — Módulo Licitaciones

**Especificación funcional y lineamientos de arquitectura — propuesta v1.2**  
**Estado:** documento de diseño; no autoriza implementación, migración, activación ni despliegue.  
**Repositorio de referencia:** `Ahcast/Bastion`, rama `main`.  
**Fecha:** 27 de octubre de 2026.

> **Convención de certeza:** **[D]** documentado en BASTION; **[U]** decidido expresamente por el usuario; **[P]** propuesta de diseño pendiente de aprobación; **[PC]** pendiente de confirmar. Los nombres nuevos de tablas, rutas, DTO y permisos son propuestas, no elementos presentes del sistema actual.

## 0. Fuentes y precedencia

Revisar antes de implementar, en este orden:

1. `docs/00-vision-y-alcance.md`
2. `docs/01-aplicacion-actual-y-paridad.md`
3. `docs/02-core-y-ajustes.md`
4. `docs/03-arquitectura-rust.md`
5. `docs/04-contrato-de-modulos.md`
6. `docs/modulos/README.md` y fichas aplicables de Colaboración y Facturas
7. `docs/05-api.md`, `06-mcp.md`, `07-datos-y-reglas.md`, `08-seguridad-y-permisos.md`, `09-despliegue.md`
8. `docs/10-criterios-de-aceptacion.md`

Para la integración se deben revisar también los contratos y las implementaciones **vigentes** de Colaboración, Facturas y el servicio de PDF/Typst; no derivar sus interfaces solo de este documento. Ante contradicciones, registrar la diferencia y solicitar resolución, no elegir silenciosamente.

El informe de CastilloIT documenta Cotizaciones, Facturas, Procesos y Colaboración, pero **no documenta un módulo Licitaciones heredado**. Este módulo es funcionalidad nueva [U], no paridad comprobada con una pantalla Legacy. `Procesos` significa importación/anotación de CSV y no debe confundirse con Licitaciones [D]. El catálogo inicial excluía nuevos módulos hasta acordarlos; la solicitud del usuario autoriza diseñar Licitaciones, **no implementar ni activar** [U].

## 1. Propósito, alcance y principio de expediente

**[U]** Cada licitación es un **expediente independiente**. Su oferta contiene exactamente tres bloques funcionales:

1. **Legal:** archivos PDF y formularios que el usuario completa internamente.
2. **Económico:** una hoja de cálculo lógica por lote ofertado, con plantilla base y variaciones según el pliego.
3. **Técnico:** PDF e imágenes asociados a cada ítem ofertado.

**[U]** El entregable es **un único PDF** con portada, índice, sección legal, económica y técnica, en ese orden, y **numeración continua**. Los PDF externos deben preservar su contenido. **[U]** Usar variables centrales por expediente y conversión **número a letras** (`number_to_words`), no la inversa. **[U]** Licitaciones solo se relaciona con los dominios **Colaboración** y **Facturas**, siempre a través de contratos mediados por los mecanismos del Core.

**[P]** Dentro del módulo: alta y consulta de expedientes; lotes e ítems; selección de lotes; copias de documentos; valores de variables; edición económica; anexos; validación; generación, historial y eventual presentación/adjudicación. **[PC]** Flujos exactos de presentación, evaluación, adjudicación y borrado.

Fuera del primer corte salvo decisión adicional: lectura automática de pliegos con IA, captura de portales, envío automático de ofertas, aprobación jurídica automática, migración masiva de Legacy, integración con otros módulos de negocio, MCP operativo y firma digital.

## 2. Límites de arquitectura y dependencias

| Componente | Propiedad y responsabilidad | Restricción |
|---|---|---|
| Core [D] | Identidad, sesión/CSRF, autorización por recurso, registro/activación de módulos, navegación, Ajustes, auditoría, límites, almacenamiento privado, servicio PDF/Typst | No alojar reglas económicas ni documentos de dominio de Licitaciones |
| Licitaciones [U/P] | Expedientes, lotes/ítems, contexto de variables, copias de trabajo, plantillas seleccionadas, valores, composición, versiones, adjudicación vinculada | No consultar/escribir tablas de otros módulos |
| Colaboración [D/U] | Biblioteca de PDF, archivos `.typ`, estilos, recursos y definiciones reutilizables, siempre que los tipos y funciones se aprueben | No resolver por SQL variables privadas del expediente |
| Facturas [D/U] | Validación fiscal/comercial, creación de factura y su estado | Licitaciones no replica sus reglas ni inserta filas en sus tablas |

Arquitectura acordada [D]: backend **Rust, Tokio, Axum, SQLx**, monolito modular; **MySQL 5.7 en Bluehost**, frontend **React + TypeScript estricto + Vite** en Bluehost; backend/API en AWS Lightsail; objetos PDF y adjuntos privados en AWS. No introducir otro motor SQL, microservicio ni servidor API paralelo. El Core utiliza Typst como servicio común de PDF. No incluir lineamientos de colores, CSS ni sistema visual: **reutilizar directamente el frontend compartido existente** [U].

**Contrato entre módulos:** Licitaciones llama interfaces/casos de uso públicos y autorizados coordinados por el Core, no rutas internas privilegiadas, SQL ajeno, S3 directo ni credenciales de otro módulo. El adaptador HTTP y el futuro MCP llaman los **mismos casos de uso**. Si una acción escribe en varios dominios, acordar un coordinador transaccional interno o un flujo idempotente y compensable; no simular atomicidad entre MySQL y objetos privados.

## 3. Estructura funcional y navegación

**[U]** El patrón de entrada sigue el listado de Cotizaciones existente: lista de expedientes; al abrir uno se entra en su contexto particular. Las pestañas **Variables** y **PDF final** nunca serán módulos globales ni entradas independientes del menú principal.

**Pantallas del expediente [P]:**

1. **Resumen:** identificación, lotes, documentos y estado de validaciones.
2. **Variables:** valores propios del expediente, incluidas variables calculadas de solo lectura.
3. **Documentación legal:** documentos requeridos, selección/copia de plantillas, formularios y PDF.
4. **Oferta económica:** lotes seleccionados, ítems, campos editables y totales calculados.
5. **Especificaciones técnicas:** anexos por lote/ítem y referencias a PDF/imágenes.
6. **PDF final:** orden, validaciones, previsualización, generación e historial de versiones.

**[P]** El listado ofrece búsqueda y paginación, con columnas por confirmar (identificador, entidad, plazo, estado y otros solo si se aprueban). No agregar botones, estados ni campos heredados sin evidencia. Cada pantalla contempla carga, vacío, error, éxito, acceso denegado, módulo deshabilitado y conflicto de edición; protege datos sin guardar. En dispositivos pequeños, las tablas del editor económico se desplazan **dentro de su contenedor**, sin alterar la navegación del Core.

## 4. Biblioteca de Colaboración y copias de trabajo

**[U]** Colaboración es la biblioteca de originales reutilizables: PDF, formularios, plantillas Typst, recursos/estilos Typst y definiciones de variables. Licitaciones **selecciona una versión y obtiene una copia independiente** para un expediente. La plantilla original jamás se altera al editar esa copia.

**Flujo [U/P]:** seleccionar archivo autorizado → resolver versión/hash → crear copia privada vinculada al expediente → editar datos/contenido permitido → guardar nueva revisión de trabajo → generar PDF → congelar referencias en la versión final. La biblioteca puede evolucionar sin modificar copias ni expedientes ya cerrados. **[PC]** Si las copias también se muestran en una carpeta visible de Colaboración o solo en la interfaz del expediente; no asumir visibilidad por defecto.

### Contrato interno mínimo con Colaboración [P]

| Caso de uso | Entrada lógica | Salida | Seguridad |
|---|---|---|---|
| `list_authorized_assets` | Actor, alcance, clase, paginación | Metadatos permitidos | `ver` sobre objetos; límites |
| `get_asset_version` | ID, versión | Metadatos, tipo, hash, referencia autorizada | `ver` objeto exacto |
| `copy_asset_to_expedient` | Versión fuente, expediente, clave idempotente | ID de copia, hash, procedencia | `ver` origen + `editar` expediente + autorización de creación |
| `save_working_revision` | Copia, contenido validado, versión esperada | Nueva revisión y hash | `editar`, `If-Match`, validación de tipo/tamaño |
| `read_authorized_blob` | ID de copia/versión | Flujo privado controlado | `ver`, alcance y auditoría |
| `delete_working_copy` | ID, confirmación | Resultado controlado | `eliminar`, referencias y retención |

Nombres y firmas **no aprobados**; primero comprobar los puertos ya expuestos por Colaboración. No aceptar rutas S3 arbitrarias ni URLs suministradas por el usuario como acceso a objetos. La creación de copias y la lectura deben respetar las políticas de ambos dominios y del Core. Versiones/fuentes se preservan para reproducibilidad.

### Plantillas Typst [P]

Una plantilla reutilizable requiere metadatos de versión, contenido aprobado, esquema de variables, dependencias permitidas, hash y estado de aprobación. Una copia editable **no equivale** a código ejecutable confiable. El usuario puede cambiar solo contenido y estructuras expresamente permitidas; compilación en ambiente restringido, con raíz temporal privada, recursos limitados, sin red, secretos, comandos ni importaciones/rutas arbitrarias. Compilar un paquete de plantilla/estilos **fijado por hash**; nunca permitir ejecutar directamente un `.typ` subido sin validación. Política de quién aprueba/publica plantillas: **[PC]**.

## 5. Biblioteca de variables y contexto por expediente

**[U]** La biblioteca conserva **definiciones reutilizables**, no los valores compartidos entre licitaciones. Cada expediente tiene un **contexto aislado** de valores. Ninguna actualización de `LIC-A` afecta `LIC-B`.

| Capa | Propietario | Datos |
|---|---|---|
| Definición reutilizable [U/P] | Colaboración o contrato de catálogo mediado por Core | Identificador estable, tipo, descripción, validación y versión |
| Declaración requerida por plantilla [P] | Licitaciones | Referencia a definiciones y obligatoriedad para el formulario |
| Valor del expediente [U] | Licitaciones | Valor tipado, origen, versión, usuario y momento de cambio |
| Cálculo [P] | Caso de uso de Licitaciones/Rust | Dependencias, operación permitida y resultado decimal |
| Instantánea final [P] | Licitaciones | Valores resueltos, hashes y revisiones congeladas |

Tipos candidatos: texto, entero, decimal, fecha, booleano y selección; moneda requiere código ISO y decimal exacto. **[PC]** Tipos definitivos, nombres, obligatoriedad, fuentes y reglas por plantilla. Ejemplos **no normativos**: `licitacion.numero`, `oferente.nombre`, `representante.nombre`, `oferta.total`, `oferta.total_en_letras`.

**Resolución [P]:** validar esquema → cargar valores de la licitación → evaluar dependencias en orden acíclico → rechazar faltantes, tipos incorrectos o ciclos → congelar snapshot → entregar JSON tipado a Typst. Las variables calculadas no se editan a mano desde el formulario. Los importes se calculan en Rust sin `f32`/`f64` como fuente de verdad. `number_to_words` convierte **número a letras** [U]; variantes de español, moneda, centavos, negativos, redondeo, magnitud máxima y capitalización **[PC]**. No implementar conversión letras→número.

Cambiar una definición de biblioteca genera versión nueva y **no cambia retroactivamente** valores o documentos ya generados.

## 6. Documentación legal, económica y técnica

### Legal [U/P]

Instancias de formularios a partir de copias de plantillas y referencias a PDF originales, agrupadas por expediente. Registrar la fuente, versión, datos capturados y validación. **[PC]** Lista de formularios y campos, obligatoriedad, subsanabilidad, firmas y reglas de cumplimiento: dependen del pliego, no se inventan. Completar formularios dentro de BASTION [U]; rellenar AcroForms existentes es **[PC]**.

### Económica [U/P]

Un cuadro por **lote ofertado**; formato base y variaciones por pliego. Separar **modelo de valores** de **plantilla oficial de presentación**. Cada columna declara identificador/tipo/orden/editabilidad/fórmula; Rust verifica cantidades, precios, importes y totales. Totales de solo lectura en UI y calculados de nuevo en backend. Utilizar `DECIMAL`; precisión, moneda, impuestos, descuentos, conversiones, redondeos y posibilidad de cantidades parciales: **[PC]**. No filtrar costos o márgenes internos a la salida oficial; incluirlos solo si se aprueba expresamente su captura interna. No establecer integración con Cotizaciones.

### Técnica [U/P]

PDF/imágenes por ítem del lote ofertado; cada asociación tiene ID de archivo/versión, ítem, orden y metadatos mínimos. Validar pertenencia al expediente, tipo y existencia del archivo. **[PC]** Cantidad mínima por ítem, fichas/certificados exigidos y evaluación técnica sustantiva. Verificar existencia/estructura no equivale a certificar cumplimiento técnico.

## 7. Motor documental, Typst y PDF único

**[U]** Orden de composición: **portada → índice → legal → económica → técnica**. Un solo PDF final, paginación continua, archivos externos preservados. El servicio Typst existente del Core genera portada, formularios y cuadros. Un **ensamblador PDF** controlado integra los PDF externos sin reconstruir su contenido.

### Pipeline propuesto [P]

1. Autorizar expediente, plantillas, anexos y operación.
2. Validar datos y tipos; resolver variables; congelar snapshot de oferta y archivos.
3. Fijar versiones y hashes de plantillas, estilos, fuentes y anexos.
4. Compilar documentos generativos mediante servicio Typst del Core, con datos JSON sin evaluación de código recibido.
5. Normalizar/inspeccionar páginas y combinar PDF preexistentes **sin alterar contenido**.
6. Determinar paginación final, rearmar índice y aplicar numeración continua cuando sea seguro.
7. Validar que referencias del índice correspondan a las páginas reales y que el PDF resultante sea legible.
8. Guardar objeto privado, hash, metadatos de composición y versión; auditar resultado.

**[PC]** Si portada/índice cuentan y muestran folio, posición del folio, páginas apaisadas, tamaños mixtos, PDF protegidos y tratamiento de PDF con firmas digitales. Modificar PDF firmado puede invalidar su firma; no alterar sin política explícita. Si una operación no puede preservar propiedades exigidas, reportar incompatibilidad y detener la generación, no degradar silenciosamente.

La instantánea fija garantiza que ediciones concurrentes no mezclen valores/documentos. La misma entrada y versiones deben reproducir el mismo contenido lógico; no afirmar igualdad binaria si existen metadatos variables del generador. PDF originales y versiones finales permanecen en objetos privados AWS, sin caducidad automática mientras exista el registro relacionado [D].

## 8. Contrato con Facturas a través del Core

**[U]** Se permitirá **crear facturas vinculadas a licitaciones adjudicadas**. **No** se crea factura automáticamente por el simple registro de una adjudicación. El momento comercial/fiscal que autoriza la creación, importes, ítems, cliente, impuestos y relación con lotes ganados son **[PC]**.

**[P]** Caso de uso orquestado `create_invoice_from_award`:

1. Core autentica actor y autoriza **editar** adjudicación/licitación y la operación de creación de **Facturas** sobre su ámbito.
2. Licitaciones comprueba existencia, versión y estado autorizado de adjudicación; resuelve lotes/ítems adjudicados en transacción/instantánea consistente.
3. Contrato tipado interno consulta la política de Facturas para validar campos requeridos, destinatario y reglas fiscales/comerciales. No insertar SQL de Facturas desde Licitaciones.
4. Facturas crea el documento mediante su **caso de uso propio**; se utiliza clave idempotente de la operación vinculada a la adjudicación.
5. Se registra la relación (`licitacion_id`, `adjudicacion_id`, `factura_id`, operación), resultado y auditoría. Un fallo parcial se recupera consultando por la misma clave antes de reintentar; no crear una segunda factura.
6. La edición, emisión, pago, saldo y numeración fiscal continúan siendo responsabilidades de Facturas.

**Contrato candidato [P]:** entrada con referencia opaca de adjudicación, identidad/ámbito, líneas y montos **solo después de aprobar el mapeo** y clave idempotente; salida ID/estado/versionado de factura y problemas de validación. La integración debe comprobar módulo Facturas incluido, disponible, habilitado y migrado. Si no está habilitado, Licitaciones continúa funcionando sin crear facturas y la acción se deshabilita/rechaza claramente. **[PC]** Si se necesita una transacción MySQL coordinada tipo `document-flows` o consistencia mediante ledger/idempotencia con compensación, según los contratos internos existentes de Facturas.

No existe contrato de negocio con Clientes, Proveedores, Cotizaciones u Órdenes. Los datos fiscales que Facturas requiera no autorizan a Licitaciones a escribir esos módulos directamente.

## 9. Permisos y alcance

Core define `ver`, `editar` (crear/actualizar/transiciones no destructivas) y `eliminar` [D]. No crear roles ni asignar permisos automáticos. Administrador del sistema conserva facultades fijadas por Core; roles personalizados empiezan sin permisos [D].

| Recurso de Licitaciones [P] | `ver` | `editar` | `eliminar` |
|---|---|---|---|
| Expediente | Listar/abrir | Crear/cambiar campos admitidos | Borrado solo si se aprueba |
| Lotes e ítems | Consultar | Seleccionar/editar datos admitidos | Retirar si se aprueba |
| Variables | Consultar valores | Modificar valores editables | Retirar valor si se aprueba |
| Documentos legales y anexos | Ver/descargar | Copiar, cargar y versionar | Eliminar con confirmación |
| Oferta económica | Consultar | Guardar precios permitidos | Según política pendiente |
| PDF final | Ver/descargar | Generar nueva versión | Eliminar versión si se aprueba |
| Ajustes propios | Consultar | Modificar configuración aprobada | No definido |

Cada operación debe verificar **permiso + empresa/tenant + objeto específico + campo autorizado + estado/versionado** en Rust, aunque la UI no exponga la acción. Las operaciones de Colaboración requieren permisos propios sobre el archivo fuente/destino. Facturación requiere permisos de ambos dominios. Denegar por defecto y evitar enumeración de IDs privados (`403`/`404` según política). Confirmación explícita adicional para borrado y otras mutaciones sensibles; no sustituye autorización.

## 10. Datos, tablas e índices propuestos

**[P]** Entidades propias; ningún nombre representa paridad Legacy:

| Tabla candidata | Finalidad | Restricciones/índices candidatos |
|---|---|---|
| `licitaciones` | Cabecera/ámbito del proceso | PK, índice por ámbito y código/fecha |
| `licitacion_lotes` | Lotes | FK proceso, código único por proceso |
| `licitacion_items` | Ítems exigidos | FK lote, código único por lote |
| `licitacion_ofertas` | Oferta y estado/versionado | FK expediente; unicidad según regla a aprobar |
| `licitacion_oferta_lotes` | Selección de lotes | FK oferta/lote, par único |
| `licitacion_oferta_items` | Valores por ítem | FK oferta-lote/ítem, par único |
| `licitacion_variable_values` | Valores propios del expediente | FK expediente, variable/versión |
| `licitacion_template_copies` | Copias y procedencia | FK expediente, referencia a archivo/version/hash |
| `licitacion_legal_forms` | Instancias/respuestas legales | FK expediente, plantilla y revisión |
| `licitacion_technical_attachments` | Anexos por ítem | FK ítem ofertado, referencia a objeto/versión |
| `licitacion_pdf_versions` | Snapshot y PDF final | FK expediente, versión/hash únicos |
| `licitacion_awards` | Adjudicaciones, si se aprueba | FK expediente/lote/ítem según alcance |
| `licitacion_invoice_links` | Relación con Facturas | FK adjudicación; ID factura externo lógico; idempotencia |
| `licitacion_operation_ledger` | Idempotencia/recuperación | Operación, actor/ámbito, clave y hash de solicitud |

El catálogo de definiciones de variables y los binarios son de Colaboración/Core según contratos concretos; no duplicar su fuente de verdad en Licitaciones. No crear FK SQL directa hacia tablas privadas de otro módulo sin contrato/decisión arquitectónica aprobada. Campos exactos, IDs, longitudes, FKs y precisión decimal: **[PC]**. Persistir fechas UTC, visualizar `America/Tegucigalpa` [D]; MySQL 5.7 con TLS verificado [D].

**Migraciones:** versionadas dentro del módulo, ejecutadas con `migrate` y credencial separada, nunca DDL en `serve`. Probar sobre MySQL 5.7 aislado, con backup/restore, doble ejecución, constraints, concurrencia y migraciones previas intactas. El rollback de release **no borra automáticamente datos** ni deshace migraciones potencialmente destructivas. Conservar registros al deshabilitar módulo [D].

## 11. HTTP/API y DTO

Usar el adaptador común `/api/v1`, `application/problem+json`, fechas ISO 8601 con zona, importes decimales exactos, sesiones cookie y CSRF/Origin para mutaciones [D]. Rutas **candidatas [P]**, por especificar en OpenAPI antes de programar:

| Método | Recurso | Caso de uso | Permiso |
|---|---|---|---|
| GET/POST | `/api/v1/licitaciones` | Listar/crear | ver/editar |
| GET/PATCH | `/api/v1/licitaciones/{id}` | Consultar/editar expediente | ver/editar |
| GET/PUT | `/api/v1/licitaciones/{id}/oferta/lotes` | Consultar/seleccionar lotes | ver/editar |
| GET/PATCH | `/api/v1/licitaciones/{id}/variables` | Consultar/editar valores | ver/editar |
| GET/POST | `/api/v1/licitaciones/{id}/plantillas/copias` | Listar/copiar desde Colaboración | ver/editar + permiso archivo |
| GET/PATCH | `/api/v1/licitaciones/{id}/legal/{formId}` | Formulario | ver/editar |
| GET/PATCH | `/api/v1/licitaciones/{id}/economica` | Tabla económica | ver/editar |
| GET/POST | `/api/v1/licitaciones/{id}/items/{itemId}/anexos` | Anexos | ver/editar |
| POST | `/api/v1/licitaciones/{id}/validaciones` | Validación formal | editar propuesto |
| GET/POST | `/api/v1/licitaciones/{id}/pdf-versiones` | Listar/generar PDF final | ver/editar |
| POST | `/api/v1/licitaciones/{id}/adjudicaciones/{awardId}/factura` | Crear factura autorizada | editar en ambos módulos |

DTO versionados candidatos: `LicitacionCreateV1`, `LicitacionPatchV1`, `VariableValuesV1`, `TemplateCopyV1`, `EconomicOfferV1`, `AttachmentV1`, `PdfBuildRequestV1`, `PdfVersionV1`, `AwardInvoiceRequestV1`. Cada uno con allowlist de campos y validaciones; no serializar directamente tablas. Cargas de archivos mediante contrato seguro existente del Core/Colaboración, no multipart sin límites definidos.

**Por operación:** fijar máximos de JSON/archivos, filas, lotes, ítems, páginas, concurrencia, timeout, paginación, rango de fecha, cuotas por actor y política de reintento. Todos los valores concretos son **[PC]**, no inventarlos. `GET` no cambia estado; POST de copias/PDF/factura utiliza `Idempotency-Key`; edición usa `ETag`/`If-Match`, `412` al editar una versión obsoleta; clave repetida con contenido distinto produce conflicto. Documentar `401`, `403`/`404`, `409`, `412`, `413`, `415`, `422`, `429` y fallos controlados que apliquen, con `request_id`. El contrato OpenAPI integrado solo anuncia rutas de módulos habilitados.

## 12. Ajustes y registro central

**[D]** Registrar un `module_id` estable (candidato: `licitaciones`), versión, dependencias, migraciones, permisos, rutas, settings y casos de uso ante el Core. El módulo aparece dentro de **Ajustes** mediante el registro central, no por un menú paralelo. Ajustes propios requieren definición y permisos `ver`/`editar` por alcance `global`, `empresa` o `usuario`; **[PC]** campos concretos. Candidatos aún no aprobados: políticas de plantillas, composición, límites específicos y reglas de validación. No duplicar ajustes de almacenamiento/PDF/seguridad del Core.

**[D]** Diferenciar: (a) código **incluido** en release, (b) módulo **disponible** por allowlist del ambiente y migraciones compatibles y (c) módulo **activado** por Administrador en `Ajustes > Core > Módulos`. Un módulo deshabilitado no publica navegación, rutas API ni herramientas MCP, pero mantiene datos. No cargar código arbitrario en tiempo de ejecución.

## 13. Seguridad, auditoría y conservación

- Cookie de sesión protegida del Core, `Secure`/`HttpOnly`, CORS origen exacto, CSRF y `Origin` en cada mutación web [D]. No secretos en frontend, URL, `.md`, logs o exportaciones.
- Autorización BOLA/IDOR por expediente, lote, ítem, variable, copia, adjunto, factura y versión; denegar accesos entre empresas.
- Archivos privados en AWS; identidad del Core en cada descarga; validación de tipo real, tamaños, cuota, malware cuando aplique y nombres internos seguros [D]. No publicar binarios en Bluehost.
- Typst y ensamblado consumen presupuesto de CPU/memoria/tiempo y límites de concurrencia, con plantilla y dependencias fijadas. No ejecución libre de `.typ` no aprobado.
- Auditar creación/edición de expediente, cambios de variables, selección de lotes, revisiones económicas, copia/sustitución/borrado de anexos, generación, adjudicación, facturación y activación/configuración, con actor, ámbito, recurso, versión, resultado y correlación. Excluir contenido privado y secretos.
- Bitácora del Core de acciones de negocio: retención de **un año** [D]. El vencimiento de auditoría no destruye expedientes. Documentos permanecen asociados hasta eliminación autorizada, confirmada y auditada [D].
- **[PC]** Si se permite eliminación de expediente/versiones y cómo tratar archivos compartidos, expedientes presentados, PDF firmados y referencias de Facturas. Nunca eliminar un archivo fuente de Colaboración por borrar una copia del expediente.

## 14. MCP

**[D]** MCP está pospuesto en el primer corte del Core. No registrar ni publicar herramientas de Licitaciones hasta aprobar transporte, OAuth Authorization Code + PKCE, consentimiento, permisos vigentes, cuotas, confirmación humana y auditoría. El dominio debe exponer casos de uso reutilizables por HTTP/MCP en el futuro; MCP no accede a SQL, S3, secretos, plantillas arbitrarias ni comandos.

Posibles herramientas futuras —**no aprobadas**—: consultar expedientes, consultar variables, validar composición, generar una versión y crear factura tras adjudicación. Cada herramienta necesitará argumentos tipados, actor/tenant, acción `ver`/`editar`, límites y confirmaciones sensibles; no se implementan ahora.

## 15. Aceptación y evidencias

**Pruebas automatizadas obligatorias propuestas [P]:**

1. Acceso sin sesión/permiso y a expediente de otra empresa denegado en Rust.
2. Módulo deshabilitado: ni rutas ni datos visibles, con persistencia intacta.
3. Copiar plantilla no altera el original; modificar una copia no altera otra licitación.
4. Fuente de Colaboración no autorizada o modificada tras fijar versión: acceso denegado/conflicto controlado.
5. Variable global definida una sola vez, valor independiente por expediente; faltantes/tipos/ciclos rechazados.
6. `number_to_words` con casos límite, moneda/centavos y redondeo aprobados.
7. Totales por lote y expediente exactos en Rust; cliente no puede cambiar cálculo protegido.
8. Anexo de ítem ajeno, MIME falso, exceso de tamaño y referencia privada: rechazo.
9. Índice apunta a páginas correctas; numeración continua tras PDF originales de varias longitudes/orientaciones.
10. PDF firmado/protegido sigue política acordada sin invalidación silenciosa.
11. Snapshot generado permanece inmutable aunque cambien plantillas/variables luego.
12. Edición concurrente produce `412`, sin perder cambio vigente; creación repetida con clave idempotente no duplica copia/PDF/factura.
13. Factura solo a través del caso de uso de Facturas y con permisos en ambos módulos; falta de módulo/condición fiscal provoca fallo controlado.
14. Dos fallos parciales (MySQL/objeto) no dejan documentos disponibles huérfanos ni duplican efectos.
15. Migración repetida sobre MySQL 5.7 sin pérdida de tablas previas; release reversible sin borrado.

**Pruebas manuales [P]:** expediente completo de muestra con varios lotes, formulario con variables, PDF técnicos y PDF original de varias páginas; edición y guardado; comparación del PDF final contra índice y numeración; vista móvil y teclado según criterios existentes del Core; confirmaciones y mensajes de error; previsualización y descarga con permisos.

**Pruebas de despliegue [D/P]:** CI, builds reproducibles, MySQL TLS y backups, migración en ambiente aislado, objetos privados, no regresiones de Core/Colaboración/Facturas, incorporación en release único y estado inicial deshabilitado; habilitar únicamente tras aprobación administrativa. Evidencia: resultados de pruebas, capturas de estados, OpenAPI, documentos anonimizados, hashes de PDF, registros de migración, auditoría y rollback ensayado.

## 16. Orden de entrega y cambios documentales

1. **Cerrar decisiones [PC]** con plantillas reales y ejemplos de lote/formularios/PDF externos.
2. **Validar contratos vigentes** de Core, Colaboración, Facturas y Typst; documentar diferencias con este diseño.
3. **Crear ficha de módulo y OpenAPI** con campos, DTO, permisos, límites y settings aprobados.
4. **Diseñar migraciones**, versionado, archivos y referencias interdominio; pruebas MySQL 5.7.
5. **Implementar casos de uso Rust** y adaptadores de acceso autorizados; pruebas de dominio/HTTP.
6. **Implementar React** con navegación, cliente, CSRF y componentes compartidos del Core, sin nuevos estilos autónomos.
7. **Integrar generación Typst y ensamblado** del PDF único, validación de índice y numeración.
8. **Integrar Facturas** solo después de fijar condición de adjudicación/facturación e idempotencia.
9. **Verificar en ambiente de prueba**, aplicar release, migrar, mantener deshabilitado; habilitar por Ajustes tras aceptación.

Documentación a actualizar **solo al solicitar implementación/documentación en GitHub**: `docs/00-vision-y-alcance.md`, `docs/modulos/README.md`, ficha `docs/modulos/licitaciones.md`, OpenAPI del módulo, referencias pertinentes de `04`, `05`, `07`, `08`, `09` y `10`; ficha de Colaboración si se amplía su contrato de biblioteca/variables y ficha de Facturas si se agrega el contrato de creación desde adjudicación. No modificar ni publicar sin autorización expresa.

## 17. Decisiones pendientes de confirmar

| Pregunta | Impacto |
|---|---|
| Formulario legal real, campos, obligatoriedad, firmas y subsanación | DTO, plantillas y validación |
| Columnas oficiales, fórmulas, descuentos/impuestos, moneda, redondeo y límites económicos | Modelo y cálculo Rust |
| Campos identificadores de lote/ítem y si puede adjudicarse parcialmente | Relaciones, validación y Facturas |
| Quién puede publicar/editar/autorizar plantillas `.typ` y definiciones | Seguridad y permisos |
| Dónde se muestran las copias de trabajo: solo expediente o también carpeta de Colaboración | Navegación y acceso |
| Tipos, nombres y fuentes reales de variables; alcance del catálogo | Datos y contratos |
| Regla exacta de `number_to_words` en español, moneda y centavos | Salida jurídica y pruebas |
| Si portada e índice muestran y cuentan folio; ubicación del número | Compositor e índice |
| Qué hacer con PDF con firma digital, protegidos y tamaños mixtos | Integridad de anexos |
| Estado/evento y autorización para generar factura; mapeo de partidas e impuestos | Contrato con Facturas |
| Estados de licitación, posibilidad de varias ofertas/revisiones, presentación/adjudicación | Máquina de estados y versiones |
| Borrado/retención de expedientes con facturas, versiones y archivos compartidos | Seguridad y conservación |
| Límites máximos por archivo, lote, ítems, PDF, concurrencia y tiempo | API y rendimiento |
| Settings propios del módulo y alcance global/empresa/usuario | Registro y Ajustes |

## 18. Regla final para el equipo de desarrollo

No considerar una función terminada porque existe la pestaña o el botón. Cada operación requiere contrato, autorización de servidor por recurso, DTO y campos permitidos, validación Rust, persistencia/transacción o compensación, control de concurrencia, límites, auditoría, errores documentados y evidencia de pruebas. **Colaboración proporciona recursos/versiones; Licitaciones produce el expediente; Facturas emite sus facturas; el Core gobierna seguridad e infraestructura común.**
