# Ejecutar Procesos y receptor crawler

Requisitos y arranque compartidos: [Core](core-runbook.md). Migración global 0004_processes.sql; runner conserva checksum/lock/dirty. Nunca usar credenciales ni datos productivos para tests. BASTION_PROCESSES_ENABLED=true por defecto; false bloquea rutas/herramientas después de reiniciar sin borrar datos.

Un administrador asigna explícitamente procesos.view/mark/note/export/admin a los usuarios autorizados. No hay permisos comerciales implícitos por ser administrador. El crawler necesita un usuario propietario activo/TOTP con grants all crawler.runs.create/crawler.processes.import/crawler.runs.complete. El administrador de servicios, con reautenticación, crea una identidad con esos tres scopes y vigencia limitada. El token se entrega una sola vez y se conserva fuera de Git; revocación y retiro de permisos bloquean replays.

Crawler Python mantiene su CLI, esperas/reintentos, artefactos y ACK: [uso](../../backend/crawler/USO.md) y [contrato exacto](../../backend/crawler/docs/IMPORT_CONTRACT.md). BLUEHOST_MODE=api y BASTION_API_URL=https://api.bastion.castilloit.net/api/internal/v1/crawler; BASTION_TOKEN mediante entorno protegido. No activar una ejecución real ni S3 por correr pruebas. S3_ENABLED y recursos reales requieren configuración y validación autorizadas.

Entrega: registrar run → lotes de hasta 100 → cierre por snapshot. El cierre no prohíbe entregas posteriores del mismo run. Ante fallo, retry entrega artefactos existentes; no necesita volver a consultar HonduCompras. ACK IMPORTED confirma aplicación durable transaccional, no solo recepción. Un registro inválido revierte todo el lote. Datos atrasados quedan como evidencia sin revertir la ficha; completitud documental no equivale a cobertura de ejecución. No se elimina ni inactiva por ausencia.

UI /processes: filtros paginados SQL, detalle, marcas/notas personales y versiones; fuente externa como texto. CSV UTF-8 neutralizado para hojas de cálculo, respeta acceso y máximo explícito 10000 resultados/8 MiB. Estado técnico requiere procesos.admin y muestra las 20 recepciones recientes, sin tokens/payloads. Actualizar refresca lectura; no dispara crawling.

Pruebas: gofmt/vet/build y go test -race con BASTION_TEST_MYSQL_DSN exclusivamente bastion_test/MySQL5.7; npm ci/build/test; Playwright con browserseed exclusivamente bastion_browser_test. Regresión Python unittest sin red. CI realiza MySQL/API/Angular reales con datos sintéticos.

Cotizaciones, AI/Workflow, asignación/comentarios/tareas de colaboración y descarga del repositorio raw S3 mantienen sus fases y propietarios. No se ofrecen botones/rutas ficticios para esas dependencias; consultas no activan investigación ni envíos. Docs/status registra lo implementado y lo pendiente; pruebas no acreditan infraestructura productiva.
