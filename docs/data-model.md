# MySQL y propiedad de datos


# 41. Modelo conceptual de datos

Entidades principales:

```text
users
roles
permissions

customers
customer_contacts

suppliers
supplier_contacts
supplier_products

processes
process_versions
process_changes
process_items

documents
document_versions

quotes
quote_items
quote_versions

customer_purchase_orders
customer_purchase_order_items

invoices
invoice_items
payments

expenses

products
price_history

rfqs
rfq_items
rfq_messages
rfq_responses

workflow_definitions
workflow_instances
workflow_events

approvals

tasks

ai_tasks
ai_runs
agent_actions

audit_events

crawler_runs
crawler_run_units
crawler_deliveries
```

---


# 42. Data ownership

| Tipo de dato | Fuente de verdad |
|---|---|
| usuarios | MySQL |
| roles/permisos | MySQL |
| clientes | MySQL |
| proveedores | MySQL |
| procesos actuales | MySQL |
| versiones de procesos | MySQL |
| raw crawler | S3 |
| crawler technical state | SQLite Lightsail |
| cotizaciones | MySQL |
| OC | MySQL |
| facturas | MySQL |
| gastos | MySQL |
| documentos físicos | S3 |
| metadata documentos | MySQL |
| código | GitHub |
| backups | S3 |
| prompts/config AI | GitHub/MySQL según uso |
| Recuperación RAG | MySQL para índices/metadatos y S3 para fuentes/texto; vector search opcional |

---

## Estado durable de IA

Tareas, ejecuciones, reservas con vencimiento, puntuaciones, ofertas encontradas, comparación por requisito, RFQ, recomendaciones y feedback según [contrato IA](contracts/ai-pipeline.md). Estados separados del estado comercial. Restricciones únicas e idempotencia contra duplicados; transacciones/actualizaciones condicionales compatibles con MySQL 5.7, sin asumir SKIP LOCKED. AI escribe sus resultados y usa servicios de aplicación del propietario para efectos comerciales.



## Nomenclatura única y persistencia

Una empresa operadora; InnoDB/utf8mb4 y compatibilidad MySQL 5.7. Tablas canónicas: customers/customer_contacts, suppliers, processes/process_items/process_versions/process_changes, quotes/quote_items/quote_versions, customer_purchase_orders/customer_purchase_order_items, invoices/invoice_items/payments, expenses, documents/document_versions/document_links, rfqs, ai_tasks/ai_runs/agent_actions, audit_events, jobs/outbox_events/idempotency_records, document_counters. No crear tablas clients, orders, order_items o invoice_payments como alternativas de estas entidades.

IDs de negocio BIGINT UNSIGNED; exponer IDs como cadenas JSON para evitar pérdida de precisión en JavaScript. IDs técnicos crawler/eventos mantienen identificadores de su contrato. UTC para timestamps; fechas de negocio DATE. Campo version para concurrencia optimista. HTTP 409 ante versión obsoleta.

Cada cotización es un agregado Quote + QuoteItem, no un PDF ni un JSON como fuente maestra. Cabecera, ítems, snapshots, cálculo, versión y auditoría se guardan transaccionalmente; PDFs/adjuntos derivados en S3. Un DTO JSON transporta datos, no reemplaza tablas. snapshots congelan el contexto del cliente/proceso al emitir; originales vinculados por FK nullable.

Importes y cantidades DECIMAL; DTO JSON con cadenas decimales. Precisiones iniciales: cantidades DECIMAL(18,4), precios/costos DECIMAL(18,6), totales DECIMAL(18,2); redondeo monetario half-up a 2 decimales. Descuento absoluto por ítem es por unidad; descuento general se distribuye proporcionalmente y se asigna residuo determinísticamente para conservar totales. Persistir desglose de bases, impuestos y descuento. Tipo de cambio DECIMAL(18,8) con fecha/fuente. Nunca float64 en cálculo financiero.

Consecutivos mediante document_counters y bloqueo transaccional, nunca MAX+1. Restricciones únicas y claves de idempotencia. Migraciones SQL numeradas globalmente en backend/database/migrations; registrar versión aplicada; no generar migraciones duplicadas por módulo. Probar en MySQL 5.7, sin CTE/window functions/SKIP LOCKED ni asumir índices JSON de MySQL 8.

El agente debe definir columnas/índices/DTO exactos de cada módulo antes de implementarlo, respetar este vocabulario y actualizar contrato y migraciones juntos. Las tablas de fases futuras no se crean hasta su fase. No migrar datos legacy con estas sustituciones nominales sin mapa y prueba de migración.

Clientes incorpora customer_addresses y customer_notes además de customers/customer_contacts. Core conserva hechos comerciales mínimos en domain_events (ID, topic, entidad/versión, actor y correlación); la intención de trabajo ejecutable sigue en outbox_events/jobs. Ningún hecho comercial crea un job/correo/AI por defecto. Ver [contrato de Clientes](contracts/customers.md).

Proveedores incorpora supplier_contacts, supplier_tags (categorías/marcas), supplier_products, supplier_notes y supplier_price_observations. Evidencia manual append-only, fuente/fechas obligatorias y valores DECIMAL como cadenas; ninguna observación web es cotización oficial. Snapshot y LegacySnapshot habilitan vínculos futuros sin crear maestros por texto. Ver [contrato de Proveedores](contracts/suppliers.md).

Procesos implementa crawler_runs/crawler_deliveries, processes/process_versions/process_changes/process_observations y process_user_marks (marca, nota personal y versión por usuario). Versiones fuente y versión personal son independientes. Documentos observados y clocks conservan snapshots JSON dentro de la versión; no son documentos Core disponibles ni acreditan bytes S3 analizados. Tablas de ítems/estados comerciales/asignaciones adicionales se incorporan cuando su fase propietaria exista. Ver [contrato de Procesos](contracts/processes.md).

Cotizaciones incorpora quotes/quote_items/quote_versions/document_counters en 0006: maestro relacional y versiones inmutables, snapshots, cuatro bases fiscales, costos y margen internos. Resultado calculador versionado, soft-delete y consecutivos con lock; no se crean tablas OC/Factura de fases futuras. Ver [contrato](contracts/quotes.md).

Archivos Quotes incorpora quote_pdf_snapshots en 0007: FK a versión inmutable, hash único por cotización, proyección pública congelada, object key privada, estado pending/available y documento Core. Reserva durable permite recuperar fallos render/S3/registro. documents agrega deleted_at/is_generated: papelera bloquea acceso y PDFs servidor preservan evidencia; documentos subidos conservan cuarentena y análisis. No PDF base64 en BD; únicamente activos gráficos PNG opcionales de la plantilla pública.

Estados preliminares Quotes reutilizan quotes.status/version y quote_versions; 0008 registra permiso de transición sin asignarlo a roles. No se crean tablas de aprobación antes de implementar su propietario Workflow. Ver contrato Quotes para grafo y revisiones.

Aprobaciones humanas en 0009: Workflow posee approvals/approval_events con binding de acción/entidad/versión/hash, ámbito, solicitante/aprobador, vigencia, versión y eventos append-only. Quotes aplica solicitud/decisión/revocación y cambios comerciales en la misma TX. Ningún ledger de aprobación entrega correo; consumo único deberá compartir la TX del efecto comercial. No se crean workflow_definitions/instances configurables antes de su fase.

Emisión Quotes en 0010 amplía quote_pdf_snapshots: kind draft/issued, approval_id único, binding_hash e issued_by/issued_at. La emisión conserva la versión aprobada de Quote y consume Workflow en la misma TX del documento/auditoría/hecho. pending no tiene documento descargable; approved no significa sent.

Programación Core en 0011 agrega outbox_events.available_at nullable UTC con precisión de microsegundos. PublishOutbox conserva la fecha efectiva al crear jobs; eventos anteriores usan created_at. No se crean tablas comerciales de correo ni se convierten hechos de dominio en envíos automáticos. Ver [contrato de transporte](contracts/mail-transport.md).

Conversión Quotes→OC en 0017: customer_purchase_orders.source_quote_id único nullable y source_quote_version FK a quote_versions; quotes.purchase_order_id único nullable/FK al destino. Los propietarios escriben sus agregados dentro de la misma TX; Quote converted y snapshot/versiones conservan destino. No copia documentos ni cambia estados sin efecto.

## Documentos privados OC e identidad de ítems

Contrato propietario vigente: [documentos privados OC](contracts/purchase-order-documents.md). Migración 0018 y vínculos durables a IDs de ítem; edición/reordenamiento preserva identidad, papelera no permite eliminar el ítem vinculado.

## Clonación OC

Contrato vigente: [clonación OC](contracts/purchase-order-cloning.md). Migración 0019 conserva origen/versión, reinicio operacional y relación inmutable sin copiar adjuntos.

## Aceptación integral OC

[Contrato vigente](contracts/purchase-orders-acceptance.md): utilidad neta sin ISV menos inversión total; profit/profit_version persistidos y eliminación reversible con evidencia protegida. Migración 0020 y permisos explícitos. Integraciones posteriores conservan sus propietarios.

Facturas Borradores (0021): invoices e invoice_items propietarios, campos DECIMAL y snapshots de cliente, condiciones contado/crédito, número/SAR NULL, saldo/pagado iniciales e IDs estables de ítems. Permisos facturas.view/create/update sin grants. No payments ni correlativo en esta unidad; contratos en contracts/invoices.md e invoices-openapi.json.

Facturas configuración fiscal (0022): invoice_fiscal_settings singleton, configuración/version y actualización auditada; contador único document_counters.invoice creado junto a primera configuración, sin adelantar/retroceder al editar. No emisión ni payments en este bloque.

Facturas emisión (0023): issued_at/issued_by/due_date y snapshot/versión pública de empresa; número/SAR previamente nullable se congelan al emitir junto a correlativo document_counters.invoice. Fechas de negocio Honduras, timestamps UTC; pago y saldo conservados sin crear payments. Contrato propietario invoices.md.
