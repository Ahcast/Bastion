# Nucleo

Fuente: [Documento maestro v0.5](../../../../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 4. Principios de arquitectura

Las reglas base del proyecto son:

1. **MySQL es la fuente de verdad de datos estructurados y transaccionales.**
2. **S3 es almacenamiento histórico, documental, raw y de backups.**
3. **GitHub es la fuente de verdad del código.**
4. **Laravel controla la lógica empresarial.**
5. **Angular es la interfaz principal para usuarios.**
6. **Lightsail ejecuta extracción y tareas técnicas, no la lógica principal de negocio.**
7. **Los agentes IA nunca acceden directamente a MySQL.**
8. **IA utiliza API/MCP con permisos limitados.**
9. **Las acciones críticas requieren autorización humana.**
10. **Todo cambio importante queda auditado.**
11. **El sistema debe operar aunque ChatGPT o Grok no estén disponibles temporalmente.**
12. **El crawler nunca reemplaza el histórico: observa, acumula, compara, versiona y reporta.**
13. **El sistema debe permitir agregar módulos futuros sin rediseñar el núcleo.**
14. **No se implementarán microservicios prematuramente.**
15. **La arquitectura inicial será un modular monolith.**

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
IA actor = grok-office-assistant
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
IA Agents
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

## 35.5 IA

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

