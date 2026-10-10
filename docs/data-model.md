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
