# Frontend Angular — Core

Implementados: login/invitación, TOTP/recuperación, cliente API entre orígenes, guards, shell, Inicio y Configuración con administración/archivos, Clientes y Proveedores. Módulos comerciales se incorporan al registro/rutas cuando estén implementados; no hay pantallas falsas ni menú de AI/Workflow.

```bash
cd frontend
npm ci
npm start
npm run build
npm test -- --watch=false
```

public/config.json contiene únicamente la URL pública API. Desarrollo: http://localhost:8080; producción: HTTPS propuesto api.bastion.castilloit.net. Sin tokens en storage; CSRF solo memoria; HttpOnly cookies. [Runbook](../docs/development/core-runbook.md) y [prueba de navegador](../docs/development/core-runbook.md#prueba-de-navegador).

Desplegar solo dist/bastion-ui/browser a Bluehost; nunca node_modules, fuentes privadas ni secretos. [Arquitectura](../docs/architecture/README.md), [idioma](../docs/idioma.md).

Proveedores: src/app/modules/suppliers, ruta /suppliers y navegación según proveedores.view/módulo habilitado. Evidencia y ficha consumen el mismo contrato Go; valores decimales se transportan como cadenas.
