# Proyecto Bastion
## Documento maestro del proyecto

**Empresa:** Castillo IT S. de R.L.  
**Proyecto:** Bastion  
**Estado:** Arquitectura conceptual aprobada como base, pendiente de validación senior y refinamiento técnico  
**Versión documental:** 0.5 — Documento maestro consolidado  
**Fecha:** 6 de octubre de 2026

---

# 1. Resumen ejecutivo

**Bastion** es el ecosistema central de operación, automatización, colaboración y AI de Castillo IT.

Su propósito es convertir la operación de la oficina en un sistema integrado donde:

- los procesos de contratación pública se descubren automáticamente;
- los procesos se versionan y conservan históricamente;
- la AI clasifica, puntúa y analiza oportunidades;
- un agente autónomo investiga productos, precios y proveedores;
- si no existe un precio público, se generan y envían RFQ a proveedores;
- los empleados pueden operar el sistema mediante un asistente conversacional;
- clientes, proveedores, cotizaciones, órdenes de compra, facturas y gastos quedan conectados;
- todas las acciones relevantes quedan auditadas;
- las acciones de alto impacto requieren autorización;
- GitHub conserva el código;
- MySQL conserva la verdad transaccional;
- Amazon S3 conserva archivos, histórico y backups;
- AWS Lightsail ejecuta el crawler y tareas técnicas;
- Bluehost hospeda el núcleo operativo de Bastion.

La arquitectura debe permanecer **modular, API-first, event-driven y AI-ready**, evitando quedar atada a un único proveedor de AI.

---

# 2. Nombre del proyecto

El nombre oficial del proyecto es:

> **Proyecto Bastion**

El nombre oficial debe usarse en toda la documentación del proyecto.

---

# 3. Objetivos de Bastion

## 3.1 Objetivo principal

Crear una plataforma central para administrar y automatizar la operación comercial y administrativa de Castillo IT.

## 3.2 Objetivos funcionales

Bastion debe cubrir:

- dashboard operativo en tiempo real;
- gestión de clientes;
- gestión de proveedores;
- procesos de contratación;
- análisis y clasificación de procesos;
- cotizaciones;
- órdenes de compra recibidas de clientes;
- facturación;
- gastos;
- reportes;
- colaboración;
- documentos;
- workflows;
- autorizaciones;
- auditoría;
- agentes AI autónomos;
- asistentes AI para empleados;
- investigación de precios;
- gestión de RFQ;
- respuestas de proveedores;
- integración con correo;
- integración con GitHub;
- backups automáticos;
- futura integración con un Knowledge/RAG Engine.

## 3.3 Objetivos no funcionales

El sistema debe ser:

- modular;
- extensible;
- auditable;
- seguro por defecto;
- tolerante a fallos;
- idempotente;
- recuperable;
- desacoplado de proveedores AI;
- mantenible por un equipo pequeño;
- compatible inicialmente con la infraestructura actual;
- preparado para crecer.

---

# 4. Principios de arquitectura

Las reglas base del proyecto son:

1. **MySQL es la fuente de verdad de datos estructurados y transaccionales.**
2. **S3 es almacenamiento histórico, documental, raw y de backups.**
3. **GitHub es la fuente de verdad del código.**
4. **Laravel controla la lógica empresarial.**
5. **Angular es la interfaz principal para usuarios.**
6. **Lightsail ejecuta extracción y tareas técnicas, no la lógica principal de negocio.**
7. **Los agentes AI nunca acceden directamente a MySQL.**
8. **AI utiliza API/MCP con permisos limitados.**
9. **Las acciones críticas requieren autorización humana.**
10. **Todo cambio importante queda auditado.**
11. **El sistema debe operar aunque ChatGPT o Grok no estén disponibles temporalmente.**
12. **El crawler nunca reemplaza el histórico: observa, acumula, compara, versiona y reporta.**
13. **El sistema debe permitir agregar módulos futuros sin rediseñar el núcleo.**
14. **No se implementarán microservicios prematuramente.**
15. **La arquitectura inicial será un modular monolith.**

---

# 5. Infraestructura actual

## 5.1 Bluehost

Castillo IT tiene **Bluehost Choice Plus Hosting**.

Actualmente Bluehost aloja:

- aplicación web;
- Laravel;
- Angular SPA;
- MySQL;
- correo electrónico de Castillo IT;
- archivos asociados a la aplicación.

Bluehost continuará siendo inicialmente el **Bastion Core**.

## 5.2 AWS Lightsail

Instancia actual:

- **1 GB RAM**
- **2 vCPU**
- **40 GB SSD**

Responsabilidades previstas:

- crawler HonduCompras;
- Python;
- cron/scheduler;
- retries;
- checkpoints;
- estado técnico local;
- generación de CSV/JSON;
- subida de raw a S3;
- backup worker;
- sincronizaciones técnicas;
- logs.

El Lightsail actual **no se considera adecuado para un RAG/vector DB completo** por la RAM disponible.

## 5.3 Amazon S3

Bucket existente:

`castilloit-honducompras-crawler`

Región:

`us-east-1` — Norte de Virginia.

Usos:

- raw crawler;
- snapshots;
- CSV;
- JSON;
- manifests;
- logs;
- documentos descargados;
- reconstrucción/replay.

Bucket propuesto adicional:

`castilloit-backups`

Usos:

- backups de MySQL;
- backups de Bluehost;
- uploads;
- documentos;
- aplicación;
- manifests de backups.

En el futuro puede crearse:

`castilloit-documents`

para separar documentación operativa de backups y raw del crawler.

## 5.4 GitHub

GitHub será la fuente oficial del código.

Repositorios propuestos:

```text
CastilloIT/
├── bastion
│   ├── backend
│   └── frontend
├── honducompras-crawler
└── infrastructure
```

Alternativa inicial simplificada:

```text
CastilloIT/
├── bastion
└── honducompras-crawler
```

---

# 6. Arquitectura general

```text
                                   USUARIOS
                                      │
                                      ▼
                                Angular SPA
                                      │
                                    HTTPS
                                      │
                                      ▼
                            ┌──────────────────┐
                            │ Laravel / Bastion│
                            │      Core        │
                            └────────┬─────────┘
                                     │
                  ┌──────────────────┼──────────────────┐
                  │                  │                  │
                  ▼                  ▼                  ▼
                MySQL               S3          Knowledge/RAG
                  │                                     │
                  │                                     │
                  └────────── Castillo API/MCP ─────────┘
                                     │
                     ┌───────────────┼───────────────┐
                     │               │               │
                     ▼               ▼               ▼
                  ChatGPT         Grok Auto      Grok Assistant


HonduCompras
     │
     ▼
AWS Lightsail
     │
     ├── crawler
     ├── retries
     ├── checkpoints
     ├── raw
     └── backup worker
        │
        ├──────────────► Amazon S3
        │
        └──────────────► Bluehost / Bastion


GitHub
  │
  ├── crawler
  ├── Laravel
  ├── Angular
  └── infrastructure
```

---

# 7. Responsabilidad por ubicación

## 7.1 Bluehost — Bastion Core

Ejecuta:

- Angular SPA;
- Laravel;
- REST API;
- autenticación;
- RBAC;
- workflow engine;
- approval engine;
- audit log;
- importador de procesos;
- lógica de negocio;
- integración con MySQL;
- endpoints para agentes;
- endpoints internos;
- generación de documentos cuando sea razonable;
- jobs según capacidad del hosting.

Almacena en MySQL:

- usuarios;
- roles;
- permisos;
- clientes;
- proveedores;
- procesos;
- versiones de procesos;
- items;
- cotizaciones;
- órdenes de compra;
- facturas;
- gastos;
- RFQ;
- respuestas de RFQ;
- precios;
- tareas;
- workflows;
- aprobaciones;
- auditoría;
- estado AI;
- referencias a documentos.

## 7.2 AWS Lightsail

Ejecuta:

- crawler HonduCompras;
- scheduler;
- retries;
- checkpoints;
- estado técnico del crawler;
- generación raw;
- subida a S3;
- entrega de resultados a Bluehost;
- backup worker;
- sincronización de backups a S3.

No debe convertirse en fuente maestra de negocio.

## 7.3 Amazon S3

Almacena:

- raw del crawler;
- CSV;
- JSON;
- run manifests;
- error manifests;
- documentos históricos;
- PDFs;
- datasheets;
- snapshots;
- backups MySQL;
- backups de archivos;
- documentos generados;
- archivos de colaboración.

No reemplaza MySQL.

## 7.4 GitHub

Almacena:

- código;
- migrations;
- tests;
- documentación;
- configuración no secreta;
- scripts;
- workflows CI/CD;
- infraestructura declarativa.

Nunca almacenar:

- `.env`;
- contraseñas;
- API keys;
- AWS keys;
- dumps SQL;
- CSV empresariales;
- documentos sensibles.

---

# 8. Stack tecnológico propuesto

## 8.1 Frontend

Baseline:

- **Angular 22.x**
- **TypeScript 6.x**
- **RxJS 7.x**
- Angular Router
- Angular Signals
- Reactive Forms
- HttpClient
- standalone components
- lazy loading por feature

Build/CI:

- **Node.js 24 LTS**

Estructura:

```text
src/app/
├── core/
│   ├── auth/
│   ├── api/
│   ├── guards/
│   ├── interceptors/
│   └── config/
├── shared/
│   ├── ui/
│   ├── pipes/
│   ├── directives/
│   └── utils/
└── features/
    ├── dashboard/
    ├── customers/
    ├── suppliers/
    ├── processes/
    ├── quotes/
    ├── purchase-orders/
    ├── invoices/
    ├── expenses/
    ├── reports/
    ├── collaboration/
    └── settings/
```

## 8.2 Backend

Baseline:

- **Laravel 13.x**
- **PHP 8.5.x**
- Composer 2.x

Fallback temporal si Bluehost no ofrece PHP 8.5:

- PHP 8.4

Patrón:

> **Modular Monolith**

Estructura propuesta:

```text
app/
└── Modules/
    ├── Core/
    ├── Customers/
    ├── Suppliers/
    ├── Processes/
    ├── Quotes/
    ├── PurchaseOrders/
    ├── Invoices/
    ├── Expenses/
    ├── Reports/
    ├── Collaboration/
    ├── Workflow/
    └── AI/
```

Cada módulo puede contener:

```text
Models/
Services/
Actions/
DTOs/
Policies/
Events/
Listeners/
Jobs/
Controllers/
Requests/
Resources/
Routes/
```

## 8.3 Base de datos

Propuesta:

- **MySQL 8.x**
- InnoDB
- utf8mb4

Pendiente:

- confirmar versión exacta ofrecida por Bluehost;
- validar límites de conexiones;
- validar tamaño;
- validar performance;
- validar procedimientos de backup.

## 8.4 Crawler

Baseline:

- **Python 3.12+**

Preferencia:

- HTTPX o Requests;
- lxml;
- BeautifulSoup.

Solo si HonduCompras exige JavaScript:

- Playwright;
- Chromium headless.

Dado el Lightsail de 1 GB RAM, se prefiere extracción HTTP directa antes que browser automation.

Estado técnico:

- SQLite o archivos estructurados.

## 8.5 API

API principal:

> **REST JSON versionada sobre HTTPS**

Namespace público/interno:

```text
/api/v1/
/api/internal/v1/
```

Contrato:

- **OpenAPI 3.1**

Convenciones:

- JSON;
- códigos HTTP coherentes;
- pagination;
- filtering;
- sorting;
- request IDs;
- correlation IDs;
- Idempotency-Key;
- versionado mayor para breaking changes.

## 8.6 MCP

Endpoint conceptual:

```text
/mcp
```

MCP será una capa de herramientas para agentes.

MCP:

- no reemplaza REST;
- no accede directo a MySQL;
- no contiene lógica empresarial exclusiva;
- llama Application Services/Actions de Laravel.

---

# 9. Autenticación y autorización

## 9.1 SPA Angular

Se propone:

- Laravel Sanctum;
- cookie de sesión HTTP-only;
- CSRF;
- first-party SPA.

Evitar:

- JWT persistido en localStorage.

## 9.2 Crawler

Identidad:

`crawler-lightsail`

Autenticación:

- service bearer token;
- scopes;
- rotación;
- opcional allowlist por IP.

Scopes ejemplo:

```text
crawler.runs.create
crawler.processes.import
crawler.runs.complete
```

## 9.3 Grok Autonomous Procurement

Identidad:

`grok-procurement`

Scopes:

```text
process.read
product.read
price.read
price.write
supplier.read
supplier_candidate.write
rfq.create
rfq.send
documents.read
```

Sin:

```text
invoice.create
purchase_order.approve
payment.approve
bid.submit
user.admin
```

## 9.4 Office Assistant

Debe operar con identidad delegada del empleado.

Propuesta:

- OAuth 2.1/OIDC;
- Authorization Code;
- PKCE.

Bastion debe registrar:

```text
AI actor = grok-office-assistant
requested_by = user_id
```

y aplicar los permisos reales del usuario.

## 9.5 ChatGPT

Acceso mediante:

- MCP;
- API;
- token u OAuth según uso.

Nunca acceso directo a MySQL.

---

# 10. Módulos de la aplicación

Menú aprobado:

```text
Inicio
Clientes
Proveedores
Órdenes de Compra
Cotizaciones
Facturas
Gastos
Reportes
Procesos
Colaboración
Configuración
```

---

# 11. Módulo Inicio

Dashboard operativo.

Debe mostrar:

- procesos nuevos;
- procesos actualizados;
- procesos prioritarios;
- procesos asignados;
- cotizaciones pendientes;
- cotizaciones por vencer;
- órdenes activas;
- facturas pendientes;
- gastos;
- RFQ pendientes;
- proveedores sin respuesta;
- aprobaciones;
- tareas;
- actividad AI;
- errores del crawler;
- backups;
- alertas.

Feed de actividad:

```text
17:31 Grok encontró precio para item X
17:22 ENEE modificó proceso Y
17:10 proveedor respondió RFQ-581
16:58 cotización Q-1432 aprobada
16:40 OC cliente recibida
```

---

# 12. Módulo Clientes

Vista 360°.

Información:

- datos legales;
- RTN;
- contactos;
- direcciones;
- instituciones;
- procesos;
- cotizaciones;
- órdenes de compra;
- facturas;
- pagos;
- documentos;
- notas;
- historial;
- actividad.

---

# 13. Módulo Proveedores

Debe contener:

- empresa;
- país;
- contactos;
- emails;
- categorías;
- marcas;
- productos;
- RFQ;
- cotizaciones recibidas;
- precios;
- lead times;
- MOQ;
- términos de pago;
- documentos;
- historial;
- evaluación;
- confiabilidad;
- tiempos de respuesta.

---

# 14. Módulo Procesos

Fuente principal:

HonduCompras crawler.

Estados de negocio posibles:

```text
Nuevo
Revisión
Asignado
Analizando
Interesante
Cotizando
Oferta preparada
Presentado
Adjudicado
Perdido
Cancelado
Cerrado
```

Funciones:

- ver proceso;
- ver histórico;
- ver cambios;
- ver documentos;
- asignar responsable;
- asignar equipo;
- prioridad;
- comentarios;
- tareas;
- análisis AI;
- score;
- items;
- crear cotización;
- iniciar research;
- iniciar procurement.

---

# 15. Módulo Cotizaciones

Origen:

```text
Proceso -> Cotización
```

o:

```text
Cliente -> Cotización directa
```

Datos:

- cliente;
- proceso;
- items;
- cantidades;
- costo;
- precio;
- impuestos;
- margen;
- flete;
- proveedores;
- documentos;
- datasheets;
- versiones;
- approvals.

Estados:

```text
Borrador
Costeo
Revisión
Aprobada internamente
Enviada
Aceptada
Rechazada
Vencida
```

---

# 16. Módulo Órdenes de Compra

En el menú inicial significa:

> órdenes de compra emitidas por clientes después de aprobar una cotización.

Entidad conceptual:

`customer_purchase_orders`

Campos:

- cliente;
- número de OC;
- cotización;
- fecha;
- items;
- monto;
- lugar de entrega;
- fecha de entrega;
- documentos;
- estado.

Estados:

```text
Recibida
Confirmada
Comprando
En tránsito
Recibido
Preparando entrega
Entregado
Facturado
Cerrado
```

A futuro debe existir separado:

`supplier_purchase_orders`

para órdenes emitidas por Castillo IT a proveedores.

---

# 17. Módulo Facturas

Relacionable con:

- cliente;
- cotización;
- orden de compra;
- items;
- impuestos;
- pagos.

Estados:

```text
Borrador
Emitida
Entregada
Pendiente
Parcialmente pagada
Pagada
Vencida
Anulada
```

---

# 18. Módulo Gastos

Datos:

- proveedor;
- categoría;
- fecha;
- monto;
- moneda;
- impuesto;
- proceso;
- cotización;
- OC;
- factura soporte;
- comprobante.

Categorías posibles:

```text
Compra producto
Flete
Aduana
Impuestos
Transporte
Viáticos
Software
Servicios
Publicidad
Oficina
Otros
```

Objetivo:

- rentabilidad real por proceso;
- rentabilidad por cotización;
- rentabilidad por OC;
- análisis financiero.

---

# 19. Módulo Reportes

Reportes:

- ventas;
- compras;
- margen;
- clientes;
- proveedores;
- procesos;
- cotizaciones;
- facturas;
- gastos;
- RFQ;
- AI;
- operaciones.

Ejemplos:

- margen por cliente;
- margen por institución;
- procesos ganados/perdidos;
- cotizaciones por usuario;
- tiempo promedio de cotización;
- proveedor más competitivo;
- cuentas por cobrar;
- gastos por categoría;
- rentabilidad por OC.

---

# 20. Módulo Colaboración

No debe intentar recrear Google Drive completo.

Debe permitir:

- archivos;
- carpetas;
- documentos;
- comentarios;
- menciones;
- tareas;
- notas;
- versiones;
- relación con entidades.

Ejemplo:

```text
Proceso ENEE-123/
├── TDR.pdf
├── oferta_tecnica.docx
├── datasheets/
├── cotizacion_proveedor.pdf
├── comentarios
└── tareas
```

Los archivos pueden vivir en S3.

MySQL guarda:

- metadata;
- entity link;
- owner;
- timestamps;
- version;
- S3 key.

---

# 21. Módulo Configuración

Debe incluir:

```text
Empresa
Usuarios
Roles
Permisos
Equipos
Impuestos
Monedas
Series de documentos
Plantillas
Email
Integraciones
AI Agents
Workflows
Aprobaciones
Categorías
Auditoría
Seguridad
```

Integraciones futuras:

- AWS;
- Gmail/email;
- Google Drive;
- GitHub;
- Grok;
- ChatGPT;
- S3;
- HonduCompras;
- otras fuentes.

---

# 22. Workflow Engine

Debe ser transversal.

Entidades:

- procesos;
- cotizaciones;
- OC;
- facturas;
- RFQ;
- gastos;
- tareas.

Ejemplo cotización:

```text
Borrador
   ↓
Costeo
   ↓
Revisión
   ↓
Aprobación
   ↓
Enviada
```

El sistema debe permitir extender o redefinir workflows.

---

# 23. Event-driven interno

Ejemplos:

`QuoteApproved`

puede disparar:

- audit;
- notifications;
- dashboard refresh;
- AI;
- workflow.

`CustomerPurchaseOrderReceived`

puede disparar:

- crear workflow;
- crear tareas;
- notificar responsable;
- iniciar procurement;
- actualizar dashboard.

Objetivo:

- desacoplar módulos;
- permitir funciones futuras.

---

# 24. Approval Engine

Separación:

## 24.1 Autónomo

AI puede:

- analizar;
- clasificar;
- investigar;
- buscar precios;
- buscar proveedores;
- crear candidatos;
- generar documentos;
- solicitar cotizaciones;
- hacer follow-up.

## 24.2 Según rol

- crear factura;
- editar cotización;
- enviar email al cliente;
- modificar ciertos registros.

## 24.3 Requiere aprobación

- presentar oferta;
- emitir OC a proveedor;
- comprometer dinero;
- autorizar pago;
- acciones críticas;
- acciones irreversibles.

---

# 25. Auditoría

Tabla conceptual:

`audit_events`

Debe guardar:

- actor_type;
- actor_id;
- requested_by;
- action;
- entity_type;
- entity_id;
- timestamp;
- source;
- before;
- after;
- result;
- correlation_id.

Ejemplo:

```text
actor: grok-procurement
action: RFQ_SENT
item: 843
supplier: 72
rfq: RFQ-2026-00581
```

Ejemplo asistente:

```text
actor: grok-office-assistant
requested_by: user_18
action: INVOICE_CREATED
quote: Q-1842
invoice: INV-3382
```

---

# 26. Crawler HonduCompras

El crawler es un componente crítico.

## 26.1 Responsabilidad

Debe:

```text
DISCOVER
EXTRACT
NORMALIZE
COMPARE
VERSION
ARCHIVE
DELIVER
```

No debe decidir:

- si un proceso es buen negocio;
- score;
- margen;
- responsable;
- proveedor;
- precio de venta.

## 26.2 Schedule

Mínimo:

```text
01:00
19:00
```

Debe permitir más ejecuciones sin cambiar código.

## 26.3 Filosofía

Nunca:

```text
TRUNCATE + RELOAD
```

Siempre:

```text
APPEND DE CONOCIMIENTO
```

## 26.4 Identidad

Prioridad:

1. ID oficial HonduCompras;
2. número de proceso;
3. URL canónica;
4. fingerprint/hash.

En Bastion:

```text
UNIQUE(source, external_id)
```

## 26.5 Estados de cambio

```text
NEW
UPDATED
UNCHANGED
```

## 26.6 Estados de completitud

Independientes:

```text
EMPTY
PARTIAL
COMPLETE
```

Ejemplo:

```text
NEW + EMPTY
```

posteriormente:

```text
UPDATED + COMPLETE
```

## 26.7 Versionado

Tablas:

```text
processes
process_versions
```

`processes`:

- estado actual;
- latest hash;
- first_seen_at;
- last_seen_at;
- current_version.

`process_versions`:

- versiones;
- snapshot;
- hash;
- cambios;
- detected_at.

## 26.8 Hash

No usar HTML bruto.

Construir:

`canonical_payload`

Normalizar:

- espacios;
- saltos;
- fechas;
- URLs;
- HTML;
- encoding;
- orden de documentos.

Después:

```text
SHA256(canonical_payload)
```

## 26.9 Tipos de cambio

```text
DESCRIPTION_CHANGED
DEADLINE_CHANGED
STATUS_CHANGED
DOCUMENT_ADDED
DOCUMENT_REMOVED
DOCUMENT_CHANGED
METADATA_CHANGED
```

## 26.10 Documentos

Por documento:

- nombre;
- URL;
- SHA256;
- first_seen;
- last_seen;
- version.

Si mismo nombre pero contenido distinto:

`DOCUMENT_UPDATED`

## 26.11 Runs

Cada run:

```text
HC-YYYYMMDD-HHMM
```

Ejemplo:

`HC-20261006-1900`

Campos:

- run_id;
- start;
- end;
- status;
- coverage;
- pages expected;
- pages processed;
- pages failed;
- records;
- retries;
- delivery;
- S3;
- ACK.

Estados:

```text
SUCCESS
PARTIAL_SUCCESS
FAILED
```

## 26.12 Tolerancia a fallos

Si:

```text
48 páginas esperadas
46 OK
2 FAIL
```

resultado:

`PARTIAL_SUCCESS`

Los 46 resultados se guardan.

Las 2 páginas van a retry queue.

## 26.13 Retries

- timeout configurable;
- exponential backoff;
- jitter opcional;
- retries por unidad;
- no repetir todo si solo falla una página.

## 26.14 Checkpoints

Guardar:

- páginas completadas;
- páginas fallidas;
- retry queue;
- pending delivery.

Estado técnico local:

- SQLite.

## 26.15 Artefactos

Cada run produce:

```text
processes.csv
processes.json
run.json
errors.json
```

## 26.16 S3

Ejemplo:

```text
s3://castilloit-honducompras-crawler/raw/
2026/
  10/
    06/
      1900/
        processes.csv
        processes.json
        run.json
        errors.json
```

## 26.17 Delivery

Hoy:

- crawler genera CSV;
- hace push a Bluehost;
- app carga MySQL.

El protocolo exacto del push actual debe documentarse.

Objetivo:

```http
POST /api/internal/v1/crawler/runs
POST /api/internal/v1/crawler/runs/{run_id}/processes
POST /api/internal/v1/crawler/runs/{run_id}/complete
```

Batches ejemplo:

100 registros/request.

## 26.18 ACK

Bluehost debe responder:

```json
{
  "received": 1847,
  "new": 21,
  "updated": 7,
  "unchanged": 1819,
  "errors": 0
}
```

## 26.19 Idempotencia

Usar:

`Idempotency-Key`

Para:

- batches;
- runs;
- RFQ;
- acciones reintentables.

## 26.20 Bluehost caído

Si scraping terminó:

```text
HonduCompras -> OK
S3 -> OK
Bluehost -> FAIL
```

estado:

`PENDING_DELIVERY`

Se reintenta entrega.

No se vuelve a scrapear.

## 26.21 Ausencia no significa borrado

Si hoy faltan procesos:

- no marcarlos eliminados automáticamente;
- usar missing_count;
- considerar cobertura;
- requerir varias observaciones;
- o estado explícito del sitio.

## 26.22 Replay

Debe poder:

```text
S3 raw
  ↓
replay
  ↓
importer
```

sin consultar HonduCompras.

## 26.23 Dry-run

Soportar:

```text
crawler --dry-run
```

## 26.24 CLI

Ideal:

```text
crawler run
crawler retry
crawler status
crawler replay <run_id>
crawler upload <run_id>
crawler validate <run_id>
```

## 26.25 Métricas

- tiempo total;
- tiempo por página;
- requests;
- errores;
- RAM;
- records;
- coverage;
- retries.

## 26.26 Health

Ejemplo:

```text
crawler: OK
last_success: ...
last_run: ...
last_coverage: 98%
pending_retries: 2
pending_uploads: 0
```

## 26.27 AI queue

Bastion decide:

```text
NEW + COMPLETE -> analizar
UPDATED relevante -> reanalizar
NEW + EMPTY -> esperar
UNCHANGED -> nada
```

---

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

# 32. Background jobs

Operaciones que no deben bloquear request HTTP:

- importaciones;
- generación PDF;
- emails;
- AI tasks;
- document processing;
- reportes pesados;
- notificaciones.

Inicial:

- Laravel Queue;
- driver database.

Si Bluehost no permite worker persistente:

- cron;
- `queue:work --stop-when-empty`;
- o worker separado.

Futuro:

- Redis;
- Amazon SQS.

---

# 33. Dashboard realtime

MVP:

- polling cada 15–30 s.

Fase 2:

- WebSocket administrado:
  - Ably;
  - Pusher.

Laravel Reverb solo si Bluehost soporta procesos persistentes.

---

# 34. Backups Bluehost -> S3

Arquitectura preferida:

```text
Bluehost
  │
SSH/SFTP
  │
Lightsail Backup Worker
  │
S3
```

Ventaja:

- evitar credenciales AWS completas en hosting compartido;
- centralizar lógica en Lightsail.

## 34.1 MySQL

Flujo:

```text
mysqldump
  ↓
gzip
  ↓
SHA256
  ↓
S3
  ↓
verify
```

## 34.2 Files

- rsync/SFTP;
- compresión;
- incremental cuando sea posible;
- S3 sync.

## 34.3 Schedule sugerido

```text
23:30 MySQL
02:30 archivos
```

## 34.4 Manifest

Cada backup:

```json
{
  "backup_id": "...",
  "source": "bluehost",
  "started_at": "...",
  "finished_at": "...",
  "size_bytes": 0,
  "sha256": "...",
  "s3_key": "...",
  "status": "success"
}
```

## 34.5 Retención

```text
diarios: 30 días
semanales: 12 semanas
mensuales: 12 meses
```

Después:

- Glacier según política.

---

# 35. Seguridad

Principio:

> Security by default + least privilege.

## 35.1 Aplicación

- HTTPS;
- WAF;
- rate limiting;
- input validation;
- CSRF;
- XSS protections;
- CSP cuando sea posible;
- secure cookies;
- session hardening;
- RBAC;
- scoped tokens;
- audit;
- approvals.

## 35.2 API

Identidades separadas:

```text
crawler-lightsail
grok-procurement
office-assistant
chatgpt
backup-worker
admin
```

## 35.3 AWS

- S3 private;
- block public access;
- encryption;
- versioning;
- lifecycle;
- IAM mínimo.

## 35.4 GitHub

- protected main;
- PR;
- checks;
- secret scanning;
- no `.env`;
- no keys;
- CI.

## 35.5 AI

Controles:

- scopes;
- tool allowlists;
- approvals;
- audit;
- no DB directa;
- protección ante prompt injection;
- sanitización de contenido externo;
- límites en email;
- no acciones financieras autónomas.

---

# 36. Observabilidad

Debe existir:

- logs estructurados;
- request_id;
- correlation_id;
- job_id;
- run_id;
- actor;
- timestamps;
- errors;
- latency;
- health checks.

Crawler:

- coverage;
- retry count;
- records;
- failed units;
- delivery status.

Backups:

- timestamp;
- size;
- checksum;
- S3 verification.

AI:

- task;
- actor;
- tools;
- result;
- duration;
- approval;
- error.

---

# 37. CI/CD

GitHub flow:

```text
feature branch
  ↓
commit
  ↓
tests
  ↓
pull request
  ↓
review
  ↓
merge
  ↓
deploy
```

Branching sugerido:

- `main`;
- feature branches.

Evitar complejidad innecesaria con `develop` si el equipo es pequeño.

---

# 38. Despliegue

## 38.1 Lightsail

GitHub Actions o deploy manual controlado:

- SSH;
- pull;
- venv;
- dependencies;
- restart scheduler/service.

## 38.2 Bluehost

Mecanismo a validar:

- SSH;
- SFTP;
- deploy nativo;
- Git support.

Debe quedar definido por senior según capacidades reales del plan.

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

# 40. Módulos futuros

Bastion debe permitir agregar:

```text
Inventario
Compras a proveedores
Logística
Importaciones
Bodega
Cuentas por cobrar
Cuentas por pagar
Bancos
Activos
Contratos
Proyectos
Mantenimiento
RRHH
CRM avanzado
Servicio técnico
Integraciones externas
```

---

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
| RAG index | servicio RAG futuro |

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

# 44. Email

El correo actual se encuentra en Bluehost.

El sistema debe poder:

- redactar RFQ;
- enviar RFQ;
- asociar correo con RFQ;
- leer respuesta;
- extraer precio;
- lead time;
- stock;
- MOQ;
- freight;
- términos;
- validez;
- datasheets.

Cada RFQ debe tener ID único:

```text
RFQ-2026-00582
```

para correlacionar replies.

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

# 46. Versionado y consistencia

## Procesos

Cada cambio significativo:

- nueva version;
- diff;
- hash;
- timestamp;
- trigger AI si corresponde.

## Cotizaciones

Deben soportar versionado cuando se modifica una cotización enviada o aprobada.

## Documentos

Document hash para detectar reemplazos.

---

# 47. Fallos previstos

## HonduCompras lento/inestable

Mitigación:

- retries;
- backoff;
- checkpoint;
- partial success;
- replay;
- schedule 2x/día.

## Bluehost temporalmente caído

Mitigación:

- store S3;
- pending delivery;
- retry delivery.

## AI no disponible

Mitigación:

- core sigue funcionando;
- tasks quedan pendientes;
- retry posterior.

## Email falla

Mitigación:

- estado;
- retry;
- no duplicar;
- audit.

## Backup inválido

Mitigación:

- checksum;
- size validation;
- manifest;
- alerta.

---

# 48. Roadmap de implementación

## Fase 0 — revisión senior

Validar:

- arquitectura;
- versiones;
- hosting;
- API;
- auth;
- crawler;
- S3;
- GitHub;
- queues;
- realtime;
- security;
- deploy.

## Fase 1 — estabilizar crawler

- GitHub;
- runs;
- retry;
- checkpoints;
- raw S3;
- idempotencia;
- append;
- versionado;
- ACK;
- health.

## Fase 2 — Bastion Core

- módulos;
- auth;
- RBAC;
- audit;
- processes;
- customers;
- suppliers;
- API;
- workflow base.

## Fase 3 — comercial

- quotes;
- purchase orders;
- invoices;
- expenses;
- reports.

## Fase 4 — AI analysis

- pending-analysis;
- score;
- classification;
- save analysis;
- audit.

## Fase 5 — Grok Procurement

Primero:

- read-only;
- recommendations.

Después:

- save price.

Después:

- supplier candidate.

Después:

- RFQ send.

## Fase 6 — Office Assistant

- read;
- create docs;
- quotes;
- invoices;
- email;
- delegated auth.

## Fase 7 — Collaboration

- S3 docs;
- folders;
- comments;
- tasks;
- versions.

## Fase 8 — RAG

Solo después de definir su objetivo funcional exacto.

---

# 49. Decisiones pendientes

El desarrollador senior debe validar:

1. Laravel 13.x como baseline.
2. PHP 8.5 o fallback 8.4 en Bluehost.
3. Angular 22.x.
4. TypeScript 6.x.
5. Node 24 LTS build.
6. MySQL exacto disponible.
7. modular monolith.
8. namespaces REST.
9. OpenAPI 3.1.
10. Sanctum SPA auth.
11. OAuth/OIDC para AI delegado.
12. service tokens.
13. queue strategy.
14. realtime strategy.
15. deploy Bluehost.
16. deploy Lightsail.
17. crawler stack HTTP vs Playwright.
18. protocolo actual de push CSV.
19. futura API crawler.
20. backups.
21. security model.
22. audit model.
23. AI scopes.
24. MCP design.
25. RAG deferred design.

---

# 50. Criterio de salida de la revisión senior

Antes de iniciar implementación formal deben quedar aprobados:

- stack;
- hosting;
- API contract;
- auth;
- RBAC;
- roles;
- crawler responsibilities;
- data ownership;
- S3 layout;
- GitHub strategy;
- queue strategy;
- backup strategy;
- audit;
- approvals;
- AI boundaries;
- deployment;
- rollback;
- observability.

---

# 51. Resultado final esperado

Bastion debe permitir que Castillo IT opere así:

```text
HonduCompras
    ↓
crawler automático
    ↓
procesos versionados
    ↓
AI analiza
    ↓
score
    ↓
asignación
    ↓
research de precios
    ↓
proveedores
    ↓
RFQ
    ↓
cotización
    ↓
cliente aprueba
    ↓
OC cliente
    ↓
entrega
    ↓
factura
    ↓
gastos
    ↓
reportes
```

En paralelo:

```text
Empleado
  ↓
Office Assistant
  ↓
Bastion API
  ↓
acciones según permisos
```

Y:

```text
Gerencia
  ↓
ChatGPT / Dashboard
  ↓
supervisión, análisis y autorizaciones
```

---

# 52. Resumen de responsabilidades

| Componente | Responsabilidad principal |
|---|---|
| Angular | experiencia de usuario |
| Laravel | lógica empresarial |
| MySQL | verdad transaccional |
| S3 | archivos, raw, histórico, backups |
| Lightsail | crawler y workers técnicos |
| GitHub | verdad del código |
| ChatGPT | supervisor/analista |
| Codex | ingeniería |
| Grok Autonomous | procurement automático |
| Grok Assistant | asistente de empleados |
| RAG | conocimiento AI futuro |
| Email | comunicación con clientes/proveedores |

---

# 53. Regla final del proyecto

> **Bastion debe ser el sistema operativo de la oficina de Castillo IT: modular, seguro, auditable, automatizable y capaz de incorporar agentes AI sin entregarles control irrestricto sobre los datos o las decisiones críticas.**

## Actualización de integración IA — 2026-10-07

Este maestro v0.5 se conserva como referencia histórica. Para la integración vigente prevalecen [pipeline IA](architecture/ai-integration.md), [contrato](contracts/ai-pipeline.md), [RAG de precios S3/MySQL](knowledge-rag/README.md) y [presentación](modules/ai-results.md). ChatGPT Work con skills puntúa pendientes e investiga; Grokbot atiende RFQ mediante Bastion; la decisión y creación de la oferta permanecen en el usuario. El RAG ya tiene este objetivo funcional; su infraestructura avanzada y criterios concretos siguen pendientes. Core/API/MCP y módulos conservan sus límites; crawler y backups siguen en Lightsail.
