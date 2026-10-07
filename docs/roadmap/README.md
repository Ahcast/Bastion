# Roadmap y decisiones pendientes

Fuente: [Documento maestro v0.5](../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

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

