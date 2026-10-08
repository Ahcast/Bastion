# Castillo API y MCP

Fuente: [Documento maestro vigente](../Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 30. Castillo API

La API es el núcleo de integración.

Endpoints conceptuales:

```text
/processes
/products
/prices
/suppliers
/rfqs
/quotes
/invoices
/expenses
/documents
/tasks
/approvals
/reports
/audit
```

Ejemplos:

```http
GET  /api/v1/processes
GET  /api/v1/processes/{id}
POST /api/v1/processes/{id}/assign

GET  /api/v1/quotes
POST /api/v1/quotes
PATCH /api/v1/quotes/{id}

POST /api/v1/invoices

GET  /api/v1/suppliers

POST /api/v1/rfqs
```

---


# 31. Castillo MCP

Herramientas propuestas:

```text
search_processes()
get_process()
get_process_items()
save_analysis()

search_products()
get_price_history()
save_price()

search_suppliers()
get_supplier()
create_supplier_candidate()

create_rfq()
send_supplier_rfq()
get_pending_rfqs()
record_supplier_quote()

get_quote()
create_quote()
update_quote()
generate_quote_datasheet()

get_invoice()
create_invoice()
generate_invoice_pdf()

find_document()
generate_datasheet_package()

draft_email()
send_customer_email()

get_pending_tasks()
complete_task()
request_approval()
```

---

## Contrato IA vigente

El [contrato del pipeline IA](ai-pipeline.md) define herramientas acotadas de claim/save/complete, investigación, RFQ, antecedentes y artefactos. Actualiza el catálogo histórico anterior; nombres definitivos y schemas se validarán al implementar. MCP en /mcp pertenece al Core y aplica identidad/delegación, scopes, políticas de autorización, idempotencia y auditoría. Toda reserva y resultado queda en MySQL; ningún cliente externo accede directamente a la base.

