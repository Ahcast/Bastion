# Despliegue

Fuente: [Documento maestro v0.5](../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

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
11. OAuth/OIDC para IA delegado.
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
23. IA scopes.
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
- IA boundaries;
- deployment;
- rollback;
- observability.

---

