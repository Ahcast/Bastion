# Seguridad

Fuente: [Documento maestro v0.5](../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

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

