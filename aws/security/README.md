# Seguridad AWS

Fuente: [Documento maestro v0.5](../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

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

