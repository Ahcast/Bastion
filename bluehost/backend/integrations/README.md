# Castillo API y MCP

Fuente: [Documento maestro v0.5](../../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

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


# 43. Conexiones entre componentes

| Origen | Destino | Mecanismo |
|---|---|---|
| Angular | Laravel | HTTPS REST JSON |
| Laravel | MySQL | conexión DB interna |
| Laravel | S3 | AWS SDK/HTTPS |
| Lightsail | HonduCompras | HTTP(S) |
| Lightsail | S3 | boto3/AWS CLI HTTPS |
| Lightsail | Bluehost | actual push a documentar; objetivo HTTPS API |
| Lightsail backup worker | Bluehost | SSH/SFTP/rsync |
| GitHub Actions | Lightsail | SSH |
| GitHub Actions | Bluehost | SSH/SFTP/nativo pendiente |
| ChatGPT | Bastion | MCP/API HTTPS |
| Grok | Bastion | MCP/API HTTPS |
| Office Assistant | Bastion | MCP + OAuth delegated identity |
| Bastion | Email | connector/API/SMTP según diseño final |

---

## Integración operativa IA

Ver [arquitectura](../../../docs/architecture/ai-integration.md) y [contrato](../../../docs/contracts/ai-pipeline.md). ChatGPT Work con skills consulta tareas y entrega resultados mediante MCP; no requiere una llamada de inferencia desde Bluehost. Grokbot usa API/MCP autenticado para RFQ; credenciales SMTP permanecen en Core y correo Bluehost. Recepción de respuestas y alojamiento Grokbot pendientes de validar.
