# Módulo: Cotizaciones

## Propósito

Preparar cotizaciones, calcular líneas/impuestos/descuentos, generar PDF, adjuntar documentos, enviar o programar correo y convertir a orden o factura.

## Campos

| Campo | Nombre técnico | Función/validación conocida |
|---|---|---|
| Buscar cotización, cliente o ítem | `search` | Filtra listado. |
| Número de cotización / orden | `quote_number`, `order_number` | Número de cotización automático al guardar según informe/UI; referencia de orden. |
| Cliente / manual | `id_client`, `client_name` | Cliente seleccionado o texto manual; confirmar validación exacta. |
| Fecha, vigencia | `quote_date`, `valid_days`, `valid_until` | Vigencia inicial observada de 15 días. |
| Estado | `status` | Borrador predeterminado; estados válidos por cerrar. |
| Descuento | `discount_type`, `discount_value` | Descuento por monto o porcentaje. |
| Contacto/preparador | `contact_email`, `prepared_by_name` | Datos de contacto y responsable. |
| Ítems | `description`, `quantity`, `unit_price`, `discount_type`, `discount_value`, `tax_type` | Líneas, descuento e impuesto exento/ISV 15/18. |
| Notas/términos | `notes`, `terms` | Notas privadas y términos. |
| Adjunto | título y `file` | Documento asociado a cotización. |
| Correo | `email_to`, `email_cc`, `email_bcc`, `email_subject`, `email_message`, `email_scheduled_at` | Destinatarios, asunto, mensaje y fecha programada; vacío envía ahora. |

## Botones y acciones

Listado: Nueva, Editar/Abrir, PDF, Marcar como enviado, Convertir a orden, Convertir a factura, Enviar por correo, Clonar, Eliminar, selección individual/total, acciones masivas y paginación.

Editor: Guardar; Editar/Cancelar; Agregar/Quitar ítem; PDF; cargar, ver, descargar y eliminar adjunto; volver. Vista previa y totales se actualizan durante edición.

Correo: previsualizar PDF/adjuntos; seleccionar documentos; Enviar ahora o Programar envío; cerrar/cancelar. El backend genera PDF y puede procesar correo programado.

El PDF de cotización se genera con Typst mediante el servicio compartido del core. Se conservan la acción y el flujo actual; los datos de cotización y su permiso de generación corresponden a este módulo. La plantilla y su versionado quedan por definir.

## Settings bajo Ajustes

El sistema tiene configuración de empresa/correo en Ajustes globales del Core, no se confirmó un panel de settings propio de cotizaciones. La sección del módulo existirá en Ajustes; opciones específicas (plantillas, defaults, horarios, etc.) son `Pendientes de decisión` y no se infieren como legado.

## API, datos y permisos

API actual: CRUD, estado, clonación, conversión, adjuntos, correo inmediato/programado y cron. Entidades: cotización, ítems, contactos, adjuntos y envíos programados. Aplicar la matriz Core: `ver` para consultar, PDF y descargar; `editar` para crear, modificar, cambiar estado, gestionar adjuntos, enviar/programar correo, convertir y clonar; `eliminar` para borrar. Envíos, conversiones y borrados validan estado, relaciones, confirmación y auditoría según el caso.

## Pendientes

Estados/transiciones, validación fiscal, destinatarios por defecto, horarios y límites de adjuntos requieren confirmación.
