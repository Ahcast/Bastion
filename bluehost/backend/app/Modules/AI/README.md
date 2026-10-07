# AI

Fuente: [Documento maestro v0.5](../../../../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 27. ChatGPT en Bastion

ChatGPT se plantea como:

- supervisor;
- analista;
- interfaz gerencial;
- soporte a investigación;
- Work;
- Codex para ingeniería.

## 27.1 Supervisor

Consultas:

- qué procesos entraron;
- cuáles tienen score alto;
- qué RFQ esperan;
- qué proveedores no respondieron;
- qué cotizaciones necesitan acción.

## 27.2 Analista

- clasificación;
- scoring;
- comparaciones;
- análisis de documentos;
- investigación.

## 27.3 Codex

- GitHub;
- desarrollo;
- tests;
- PR;
- refactor;
- migrations;
- infraestructura.

Codex no debe ser procurement agent operativo.

---


# 28. Grok Employee #1 — Autonomous Procurement

Rol:

> resolver items pendientes de precio hasta encontrar un precio confiable o dejar RFQ enviado y trazable.

## 28.1 Flujo

```text
Proceso analizado
    ↓
Items
    ↓
pricing_tasks
    ↓
Grok Procurement
    ↓
fuentes internas
    ↓
web
    ↓
precio?
 ┌──┴──┐
 sí    no
 ↓      ↓
guardar buscar proveedor
        ↓
       RFQ
        ↓
      email
```

## 28.2 Fuentes

Orden conceptual:

1. catálogo local;
2. histórico;
3. Knowledge/RAG;
4. web;
5. Amazon;
6. eBay;
7. distribuidores;
8. fabricantes;
9. proveedores existentes;
10. nuevos proveedores;
11. RFQ.

## 28.3 Estados

```text
NEEDS_PRICE
RESEARCHING
PRICE_FOUND
NO_PRICE
SUPPLIER_SEARCH
RFQ_CREATED
RFQ_SENT
WAITING_REPLY
FOLLOW_UP_DUE
QUOTED
```

## 28.4 Email

No dar una herramienta genérica:

`send_email()`

Preferir:

`send_supplier_rfq()`

para limitar contexto y destino.

La API debe controlar:

- destinatario;
- proveedor;
- template;
- producto;
- límites;
- duplicados;
- cooldown.

---


# 29. Grok Employee #2 — Office Assistant

Rol:

> puente entre empleado y Bastion.

Ejemplos:

- crea factura;
- genera datasheet package;
- busca cotización;
- envía correo;
- crea documento;
- consulta cliente;
- genera reporte.

## 29.1 Principio

El asistente hereda permisos del empleado.

Nunca:

```text
Empleado cualquiera
  ↓
AI
  ↓
credenciales admin
```

Debe existir identidad delegada.

---


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


# 39. Knowledge / RAG Engine

El RAG es importante para AI, pero su diseño funcional **queda pendiente**.

Aclaración importante:

> El RAG no debe asumirse como una búsqueda de items iguales o equivalentes técnicos.

La relación histórica que debe usar para apoyar pricing y decisiones aún debe definirse.

Por ahora la arquitectura reserva:

```text
Knowledge/RAG Service
  │
  ├── S3 documents
  ├── vector/knowledge index
  └── Bastion API/MCP
```

No se fija todavía:

- Qdrant;
- embeddings;
- reglas de similitud;
- infraestructura final;
- lógica de pricing basada en RAG.

El diseño RAG tendrá revisión independiente.

---


# 45. Estados AI

## Procurement

```text
PENDING
RESEARCHING
PRICE_FOUND
RFQ_REQUIRED
RFQ_SENT
WAITING_REPLY
FOLLOW_UP_DUE
QUOTED
FAILED
NEEDS_REVIEW
```

## Analysis

```text
PENDING
RUNNING
COMPLETED
FAILED
STALE
```

---

