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

Procesos incorpora ruta /processes con filtros, interés/nota personal, versiones/diffs, CSV y estado protegido. Conversión a borrador de cotización disponible con Quotes habilitado y permisos de ambos módulos; resultados AI pendientes.

Cotizaciones: ruta /quotes, borradores/cálculo backend, snapshots, historial/clones/papelera y filtros URL. PDF de borrador y adjuntos implementados; correo/estados/bulk pendientes; no módulo completo.
