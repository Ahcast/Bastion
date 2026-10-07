# Backend Laravel

Fuente: [Documento maestro v0.5](../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

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

Cardona debe registrar:

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

