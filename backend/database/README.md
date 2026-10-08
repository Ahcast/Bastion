# MySQL y propiedad de datos

Fuente: [Documento maestro vigente](../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

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

Tareas, ejecuciones, reservas con vencimiento, puntuaciones, ofertas encontradas, comparación por requisito, RFQ, recomendaciones y feedback según [contrato IA](../../docs/contracts/ai-pipeline.md). Estados separados del estado comercial. Restricciones únicas e idempotencia contra duplicados; transacciones/actualizaciones condicionales compatibles con MySQL 5.7, sin asumir SKIP LOCKED. AI escribe sus resultados y usa servicios de aplicación del propietario para efectos comerciales.

