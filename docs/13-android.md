# Android nativo de Bastion

## Decisión del propietario — 2026-11-17

Marco solicita una aplicación nativa conectada a la API, con todos los módulos. Esta decisión sustituye TWA/Custom Tabs de 0.1.0/0.1.1. Android 0.2.0 usa Java 17 y Android Views: Activity, listas, formularios, selectores, fechas y visor PdfRenderer nativos. No incluye WebView, Browser Helper, React, HTML ni JavaScript de ejecución. Los metadatos JSON incluidos en el APK describen campos y rutas de contratos revisados; no descargan ni ejecutan interfaz remota.

La aplicación web sigue publicada en Bluehost. Android accede a `https://api.bastion.castilloit.net`; AWS conserva backend/API/MCP y MySQL permanece en Bluehost. No se duplica el motor de cálculo ni se despliegan cambios del servidor para este cliente.

## Áreas

| Área | Pantallas y operaciones nativas |
|---|---|
| Acceso | Contraseña, TOTP, recuperación, sesión, reautenticación y cierre |
| Clientes / Proveedores | Búsqueda, páginas, alta, detalle, edición, archivo y selección para documentos |
| Cotizaciones | Editor con partidas, cálculo servidor, estados, duplicación, conversiones, PDF, adjuntos y correo |
| Órdenes de compra | Productos, proveedores, costos/venta, gastos, destinos, archivos y conversión |
| Facturas | Borradores, edición, emisión, pagos, duplicación, factura/nota de entrega/recibo |
| Gastos | Registro, fechas, proveedor, bases, cálculo ISV y edición |
| Procesos | Consultas, filtros, fichas, marcas/notas, Scout, importación y exportación |
| Licitaciones | Expedientes, variables, formularios, lotes, archivos, orden/generación PDF y adjudicaciones |
| Colaboración | Carpetas, búsqueda, carga, descarga, títulos, papelera, restauración y política |
| Contabilidad | Cuentas, períodos, asientos, acciones, propuestas de origen y balance |
| Reportes | Órdenes, facturas y gastos con filtros y exportación |
| Ajustes | Perfil, sesiones, dispositivos, seguridad, SMTP, personas, usuarios, roles, Core, SAR, módulos y auditoría |

La disponibilidad de módulos y operaciones depende del catálogo y los permisos del usuario. El servidor es la autoridad final de estados, límites y autorizaciones. Los formularios se derivan de OpenAPI con complementos cotejados contra los DTO Rust. `android/qa/generate-contract.cjs` genera el recurso estático `contracts.json`; regenerar requiere `yaml` 2.9.1. Importes decimales se mantienen como texto y se calculan en el servidor. Las claves estables de líneas se conservan al editar.

## Seguridad y comportamiento

- Transporte HTTPS fijo, sin redirecciones ni caché; no se acepta URL de servidor desde Intents.
- Cookies privadas cifradas AES-GCM con Android Keystore, backup deshabilitado, contraseñas/TOTP solo en memoria durante su operación.
- Se conservan CSRF ligado a sesión, TOTP, límite de sesiones, auditoría y permisos Core. Un 428 conserva el formulario; un 412 no sobrescribe otra versión.
- La misma carga conserva Idempotency-Key ante un fallo de red; no se reintentan automáticamente mutaciones ambiguas.
- Los formularios permanecen en memoria mientras vive la Activity, incluida rotación. No hay edición offline; al terminar el proceso Android se pierden cambios sin guardar.
- Selectores del sistema para archivos, sin permiso general de almacenamiento. La carga móvil se limita inicialmente a 20 MiB por archivo; los límites menores del servidor siguen aplicando.
- PDF se muestra con PdfRenderer y se exporta por elección del usuario. Otros formatos se guardan con el selector Android. No se ejecuta HTML de archivos.

## Validación y distribución

CI construye APK Preview firmado, release sin firmar y APK de pruebas. Verifica lint, firma, metadatos y hashes. El ensayo instala el mismo Preview entregable en Android 7/API 24, Android 15/API 35 y Android 16/API 36; deshabilita Chrome y exige controles `android.widget.EditText`, Activity propia y ausencia de WebView. Repite arranque a 320 dp y fuente 130 %. La instrumentación ejercita acceso/TOTP sintéticos, navegación de todos los módulos, escritura de cliente, serialización de documentos/contabilidad, permisos, CSRF, idempotencia y cifrado de sesión. No usa cuentas reales ni modifica producción.

Compilar o definir estas pruebas no acredita su aprobación. Los resultados concretos, commit y hashes de entrega se registran en `09-despliegue.md` después de observar CI. No se ha probado el teléfono físico de Marco. La Preview utiliza la firma de desarrollo del entorno de compilación; para reemplazar otra Preview con distinta firma hay que desinstalarla. La distribución estable requiere la clave duradera del propietario; no se publica esa clave ni se entrega el release sin firmar como instalable.

El historial del fallo y reparación del contenedor 0.1.x se conserva en `09-despliegue.md`. Digital Asset Links ya no es un requisito de la aplicación nativa.
