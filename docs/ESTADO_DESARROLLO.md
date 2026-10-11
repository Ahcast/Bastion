# Estado operativo de desarrollo — Bastion

Este archivo es el **checkpoint corto para reanudar desarrollo**. La documentación de arquitectura, contratos, decisiones y runbooks sigue siendo normativa; este archivo indica dónde quedó el trabajo real y qué debe hacerse después.

Actualizado: 2026-10-11.

## Regla para reanudar

1. Trabajar únicamente sobre `main`; GitHub es la fuente de verdad.
2. Leer este archivo, `development/decisions.md`, el contrato y el runbook del módulo activo.
3. Verificar el HEAD y el último CI antes de modificar código.
4. Si el CI está rojo, corregir primero la regresión y repetir pruebas; no avanzar de módulo.
5. Desarrollar una unidad verificable, agregar pruebas, esperar CI verde y actualizar este checkpoint.
6. No declarar validadas integraciones productivas solo porque pasen mocks/adapters/pruebas sintéticas.

## Estado global

- Core: implementado y validado en CI.
- Clientes: unidad cerrada y validada.
- Proveedores: unidad cerrada y validada.
- Procesos/receptor crawler: unidad base cerrada y validada.
- Cotizaciones: **cerrado y validado en CI para el alcance monomoneda HNL/USD**.
- OC de clientes: **Unidades 1..5 cerradas y validadas en CI; módulo activo para aceptación integral**.
- Facturas/pagos y módulos posteriores: pendientes; no iniciar antes de cerrar OC por unidades.
- Producción: no desplegada ni migrada.

## Cotizaciones — cierre certificado

Alcance cerrado:

- borradores, cálculo decimal exacto, versiones, idempotencia, auditoría y snapshots;
- conversión desde Procesos;
- adjuntos y PDF de borrador;
- Costeo/Revisión, historial y control optimista;
- aprobación humana, revocación y emisión PDF inmutable;
- correo comercial durable, autorización explícita, worker, revalidación y estado SMTP incierto;
- estados comerciales finales/reapertura con motivo;
- bulk durable de estado, clonación y exportación, con resultado por ítem, recuperación, CSV público congelado y neutralización de fórmulas;
- observaciones FX append-only como registro auxiliar, sin conversión automática de cotizaciones;
- HNL y USD permanecen monedas explícitas y separadas.

### HEAD final certificado de Cotizaciones

HEAD: `0a98bf756c4e7a3965b7b625473258509fbaa571`.

Core checks: run `38094367694`, conclusión `success`:
- Go format/vet: aprobado;
- `go test -race -count=1 ./...` contra MySQL 5.7: aprobado;
- builds Go: aprobados;
- Angular build/tests: aprobados;
- Playwright contra API + Angular + MySQL reales de CI: aprobado.

Este CI sustituye como evidencia de cierre a los checkpoints anteriores de Cotizaciones. No acredita S3, SMTP, Percona/Bluehost ni despliegue productivo reales.

## Módulo activo — Órdenes de Compra de clientes

Fuente funcional: `docs/modules/purchase-orders.md`. El maestro y `development/decisions.md` resuelven la nomenclatura vigente:

- el menú **Órdenes de Compra** significa órdenes recibidas de clientes;
- tablas canónicas: `customer_purchase_orders` y `customer_purchase_order_items`;
- REST: `/purchase-orders`;
- las futuras órdenes emitidas por Castillo IT a proveedores pertenecen a `supplier_purchase_orders` y no se mezclan en este módulo;
- no crear tablas genéricas `orders`/`order_items`.

### Unidad 1 — cerrada y certificada

Alcance implementado:

- migración MySQL 5.7 de `customer_purchase_orders` y `customer_purchase_order_items`;
- estado inicial único `received` / **Recibida**; los ítems nacen `pending` / **Pendiente**;
- agregado completo de cabecera + ítems guardado transaccionalmente;
- cliente manual o snapshot autorizado del módulo Clientes;
- proveedor por ítem manual o snapshot autorizado del módulo Proveedores;
- snapshots históricos preservados aunque cambie el maestro o se reordenen ítems;
- cálculo exacto compartido con la infraestructura fiscal canónica: subtotal/bruto, descuentos, ISV 15/18, exento, exonerado y total;
- costo de productos, gastos adicionales e inversión total exactos, sin definir utilidad todavía;
- HNL o USD explícitos por OC, sin mezcla ni conversión implícita;
- alta, consulta, edición versionada y listado paginado con filtros;
- scopes `self/team/all`, autorización por recurso y sin fuga de `COUNT` fuera del scope;
- idempotencia, control optimista, auditoría/hecho y rollback atómico;
- REST estricto con cinco operaciones: calculate, list, get, create y update;
- `Idempotency-Key` obligatorio en create/update, campos desconocidos rechazados y `required_delivery_date` presente pero nullable;
- contrato OpenAPI machine-readable y guard bidireccional Go ↔ OpenAPI;
- composición global con `BASTION_PURCHASE_ORDERS_ENABLED`; Clientes/Proveedores son integraciones opcionales y un ID sin propietario disponible falla cerrado;
- Angular en español: listado, filtros, alta, edición, detalle y vista previa de cálculo;
- pruebas unitarias, integración MySQL, HTTP real, composición, Angular y Playwright completo contra API + Angular + MySQL reales de CI.

Fuera de Unidad 1 (consultar los cierres posteriores; no es la lista vigente de pendientes):

- grafo completo de estados y transiciones;
- entrega/logística avanzada;
- cotización → OC;
- documentos/evidencia;
- OC → factura;
- utilidad/margen comercial;
- conversión FX automática;
- clone/AI;
- órdenes emitidas por Castillo IT a proveedores.

### HEAD certificado de Unidad 1

HEAD: `bd95df744119ada1e0ee68ed644e44b953ea38b5`.

Core checks: run `38150153611`, conclusión `success`:
- Angular build/tests: aprobado;
- Playwright: **9/9 escenarios aprobados**, incluido el flujo real de OC;
- Go vet/formato: aprobado;
- `go test -race`: aprobado;
- builds Go: aprobados.

Documentation checks del mismo HEAD: run `38150153697`, conclusión `success`.

El escenario E2E de OC valida cálculo HNL 201.25, inversión HNL 83.00, alta manual sin permisos de catálogos, persistencia de fecha requerida `null`, recarga, edición de versión 1 → 2 y búsqueda por descripción del ítem.

Este cierre certifica la Unidad 1 en CI, no las integraciones productivas externas listadas más abajo.

## Contrato vigente de estados de OC

El contrato único `contracts/purchase-orders-states.md` está congelado e implementado. Cabecera: Recibida → Confirmada → Comprando → En tránsito → Recibido → Preparando entrega → Entregado. Ítems: Pendiente → Comprado → Enviado → Recibido → Entregado. Facturado/Cerrado quedan reservados a sus futuros propietarios. No se aceptan alias ni transiciones del grafo legacy; cualquier tratamiento de datos legacy reales exige revisión explícita antes de migración productiva.

## Integraciones cruzadas posteriores

- Cotización → OC debe ser server-side, transaccional/idempotente y conservar `source_quote_id`.
- Quotes no escribe directamente tablas privadas de OC; la conversión se completa mediante el servicio propietario PurchaseOrders.
- OC → Factura se implementará cuando exista el propietario Invoices y conservará `invoice_id` durable.
- Vistas cruzadas se incorporan cuando ambos propietarios estén implementados; no crear stubs ni datos ficticios.

## Validaciones externas aún pendientes

No están certificadas todavía por las pruebas actuales:
- S3/IAM productivo y scanner externo;
- SMTP/IMAP real de Bluehost y límites reales de tamaño/destinatarios;
- Percona/MySQL real de Bluehost, TLS/túnel y cuentas de mínimo privilegio;
- DNS/HTTPS/CORS/CSP/WAF productivos;
- backup/restore productivo;
- despliegue real en Lightsail/Bluehost;
- migración de datos reales;
- OAuth delegado/ChatGPT Work;
- carga/memoria conjunta real del crawler, API y workers.

Estas validaciones externas son gates de producción; no deben falsearse con fixtures de CI.

## Unidad 2 — seguimiento de estados y entrega cerrada

El contrato normativo `contracts/purchase-orders-states.md` está congelado. Base de grafo/historial/permisos publicada en `a18ed6b8074fd111746fd84431ee47d26019037c`, Core checks `38150855851` aprobado.

Implementación: transiciones humanas adyacentes de cabecera/ítem, historial paginado, lock agregado, versión/idempotencia y auditoría/eventos atómicos. Entrega exige permiso específico, reauth y todos los ítems entregados. PATCH sólo permite Recibida con todos los ítems Pendiente. Interfaz española muestra acción siguiente/historial, conserva claves ante timeout y estado confirmado ante fallo de recarga. Filtros canónicos y OpenAPI/paridad actualizados.

Corregida la lectura del descuento general desde persistencia: guardar, recargar y avanzar estados preserva su importe. La regresión verifica descuento 5.00 y total 201.25.

HEAD de código certificado: `0fa94796ffee0c248035f94a9166f22c0a955b42`. [Core checks 38154939036](https://github.com/Ahcast/Bastion/actions/runs/38154939036), [documentación 38154939037](https://github.com/Ahcast/Bastion/actions/runs/38154939037) y [rama única 38154939034](https://github.com/Ahcast/Bastion/actions/runs/38154939034): success. Backend: formato/vet, pruebas con MySQL 5.7 y race detector, builds. Frontend: build y 64 pruebas. Navegador: 9/9 escenarios contra API/Angular/MySQL, incluido ciclo completo de OC, historial, entrega con reauth y conservación financiera. Localmente: 17 pruebas offline del crawler con dependencias fijadas, build/64 pruebas Angular y paridad documental. Go/MySQL y navegador se certifican por CI; no se ejecutaron localmente en esta sesión.

Las pruebas intermedias corrigieron el uso de TOTP ya consumido sustituyéndolo por recovery sintético de un uso para reauth. El escenario de emisión existente recibió diagnóstico por etapas y un presupuesto total mayor tras agotar 30 segundos; conserva sus aserciones acotadas y aprobó las ejecuciones finales.

## Unidad 3 — conversión Cotización → OC cerrada

Contrato único `contracts/quote-purchase-order-conversion.md`, migración 0017 y servicios propietarios implementados. Una Cotización Aceptada produce una OC íntegra en una TX: snapshots históricos de cliente/versión, moneda, ítems, precios/costos, descuentos, impuestos y exoneración preservados. Número OC/destino/fechas/gastos/comentarios se confirman explícitamente. Fuente única, `source_quote_id/source_quote_version`, `purchase_order_id`, estado Convertida a OC, versiones, auditoría y hechos durables. No refresca catálogos ni copia documentos/notas privadas automáticamente. OC recibida/ítems pendientes; Quote convertida no se reabre; clonar Quote reinicia su vínculo.

REST estricto registrado sólo con ambos propietarios habilitados. Permisos de fuente y destino, scopes self/team/all y replay reautorizado. UI española conserva intención tras timeout y resultado confirmado tras fallo de recarga; navegación cruzada mediante permisos independientes.

HEAD de código certificado: `541bb101db51b391265b95ca9a8acd0ff0a3ae85`. [Core checks 38159858594](https://github.com/Ahcast/Bastion/actions/runs/38159858594), [documentación 38159858792](https://github.com/Ahcast/Bastion/actions/runs/38159858792) y [rama única 38159858687](https://github.com/Ahcast/Bastion/actions/runs/38159858687): success. Backend: gofmt/vet, `go test -race -count=1 ./...` con MySQL 5.7 y builds server/worker/admin. Angular: build/68 pruebas. Navegador: **10/10 escenarios aprobados**, incluida conversión USD 201.25/inversión 83.50, recarga y ambos vínculos.

Nuevas integraciones cubren cálculo mixto ISV15/18/exento/exonerado, descuentos generales/por ítem, snapshot de cliente preservado tras cambiar maestro/revocar catálogo, inmutabilidad/origen/versiones, 12 replays concurrentes y ocho actores/intenciones distintos, rollback de auditoría/eventos/snapshot, acceso ajeno/equipos/revocación, estados inválidos, DTO estricto y flag. El backend y navegador se ejecutaron en CI; localmente aprobaron Angular build/68 pruebas, compilación de los 10 escenarios y paridad/enlaces. Crawler y migraciones 0001..0016 permanecen sin cambios.

CI intermedio detectó dependencia de números fijos y datos globales en el escenario Quotes, y un selector de Moneda incompatible con el texto accesible del select. Corregidos números capturados del servidor, filtro por expediente y select por nombre; se acotó además el título al documento comercial. La suite completa final aprobó; no se extendieron timeouts ni se omitieron aserciones para resolver estos fallos.

## Próximo paso exacto

Continuar dentro de OC con clonación por servicio propietario: contrato, reinicio de estados/origen/vínculos, snapshots, permisos, idempotencia, auditoría, UI y aceptación integral. Documentos privados de cabecera/ítems cerrados; no copiar vínculos ni bytes automáticamente al clonar. OC→Factura/utilidad requieren sus propietarios/reglas; no iniciar Facturas/pagos antes del cierre correspondiente. Ningún despliegue, envío ni migración productiva realizado.

## 2026-10-11 — Unidad 4 OC: documentos privados cerrada

Contrato único `purchase-order-documents.md`, migración 0018 y política propietaria mediante Core.Files. Documentos privados de cabecera/ítems; tabla de vínculos con FK compuesto, ID estable de ítem conservado al editar/reordenar, actor/fecha y documento único. Core conserva versiones, checksum/MIME/tamaño, cuarentena/análisis, preview/descarga autorizada y papelera/restauración sin borrar vínculos ni bytes. Permiso explícito ordenes.documents.write sin concesión automática; scopes de OC y documento independientes, actor humano/reauth, replay reautorizado, versión OC/idempotencia/auditoría/evento en una TX. Writes operativos hasta entregada; estados protegidos denegados.

Angular español: destino cabecera/ítem, upload privado con etapa/clave conservada tras fallo, vínculo de documento existente, filtros/paginación/papelera, reauth y controles por estado/permiso. Versión confirmada conservada tras fallo de recarga; respuestas tardías no afectan otra OC abierta. No copia adjuntos de la cotización origen.

HEAD de código certificado: `1073c261de0c8f7ab12aed0ff6bb7b6a5f361e7c`. [Core checks 38163339892](https://github.com/Ahcast/Bastion/actions/runs/38163339892), [documentación 38163339970](https://github.com/Ahcast/Bastion/actions/runs/38163339970) y [rama única 38163339924](https://github.com/Ahcast/Bastion/actions/runs/38163339924): success. Backend: gofmt/vet, `go test -race -count=1 ./...` con MySQL 5.7 y builds server/worker/admin. Angular: build/**73 pruebas**. Navegador: **10/10 escenarios**, incluidos upload/finalize real contra adapter sintético privado, adjuntos de cabecera/ítem en cuarentena, filtro, papelera/restauración y recarga.

Local: build/73 pruebas Angular, compilación de 10 escenarios Playwright, paridad/enlaces y diff sin errores. Go/BD/navegador certificados por CI, no por ejecución local. Pruebas nuevas: identidad estable al reordenar/editar, eliminación de ítem vinculado bloqueada incluso en papelera, IDs duplicados/ajenos, FK/pertenencia, cuarentena/scan/preview/descarga, 12 replays concurrentes, autorización de ambos propietarios/revocación, scopes técnicos self/team y membresía, estados protegidos, rollback del vínculo/versiones ante fallo de evento, DTO HTTP estricto/CSRF y flag de composición.

CI intermedio detectó sincronización pendiente al volver al listado Quotes: ahora el escenario espera navegación/carga inicial antes del filtro y verifica respuesta filtrada/total. Se agregó además defensa de producción contra respuestas de listado fuera de orden y su prueba. El escenario de conversión captura el cuerpo HTTP al recibirlo para impedir que navegación lo descarte. Suite completa final aprobada; sin omitir comprobaciones ni ampliar timeouts para estas correcciones. CI conserva contexto de navegador sólo ante fallo. Crawler y migraciones 0001..0017 intactos. S3/escáner productivo no certificados; no despliegue, envío ni migración productiva.

Próximo: clonación de OC y aceptación integral, antes de Facturas/pagos.

## 2026-10-11 — Unidad 5 OC: clonación cerrada

Contrato `purchase-order-cloning.md`, migración 0019 y POST clone estricto. Copia snapshots históricos de cliente/proveedor y versiones, moneda HNL/USD, cantidades, impuestos y descuentos. Número, fechas, destino, comentarios y gastos nuevos explícitos. OC independiente en Recibida/ítems Pendiente con identidad nueva, sin documentos, entrega, historial operativo ni vínculo Quote directo. La fuente conserva su versión/estado y el destino registra origen/versión durable. Permisos view/create y equipo actuales; replay reautoriza también lectura del destino. Idempotencia, cabecera/ítems, relación, auditoría/eventos comparten TX. Sin grants automáticos ni escritura técnica/MCP.

HEAD certificado `6667fe62f952fd1702c627ae88138bab9924a522`. [Core checks 38165562083](https://github.com/Ahcast/Bastion/actions/runs/38165562083), [documentación 38165562010](https://github.com/Ahcast/Bastion/actions/runs/38165562010) y [rama única 38165561926](https://github.com/Ahcast/Bastion/actions/runs/38165561926): success. Go fmt/vet, race/MySQL 5.7 y builds server/worker/admin; Angular build/**75 pruebas**; navegador **10/10** con clonación de OC entregada y adjuntos, cálculo preservado, reinicio operacional, adjuntos aislados y recarga.

Pruebas nuevas: vectores mixtos HNL/USD 15/18/exento/exonerado y descuentos, 12 replays concurrentes, versión/intención conflictivas, fuente inmutable, estados/IDs nuevos, documentos/historial no copiados, scopes de fuente/destino/revocación, catálogo actualizado y acceso de catálogo retirado con snapshots históricos conservados, creación en equipo ajeno y membresía perdida, concurrencia con transición y orden uniforme de locks, rollback de evento y retry, DTO/CSRF estricto, UI clave/destino conservados ante fallo. Local: build/75 pruebas Angular, compilación 10 escenarios, paridad/enlaces y diff. Go/MySQL/navegador acreditados por CI.

CI intermedio identificó formato de plantilla que separaba una expresión monetaria y fallos del buffer CDP al leer respuestas comerciales. Plantilla anterior conservada; pruebas de conversión/clonación capturan la respuesta del servidor mediante forwarding real sin alterar petición/cuerpo, y la devuelven al navegador. No datos simulados, aserciones retiradas ni timeouts ampliados. Regresión completa final aprobada. Sin despliegue, correo, migración productiva ni certificación S3/escáner/Bluehost.

Siguiente unidad: aceptación integral OC, incluyendo política de eliminación reversible y utilidad canónica, antes de Facturas/pagos.

## 2026-10-11 — Unidad 6 OC: aceptación integral en validación

Contrato purchase-orders-acceptance.md y matriz development/purchase-orders-acceptance.md. Migración 0020 con deleted_at, profit/profit_version, backfill decimal e índice de papelera; ordenes.delete sin concesión automática. Eliminación reversible solo Recibida/ítems Pendiente y sin conversión, documentos (incluida papelera), historial o clones derivados. Reauth/razón, scopes view/delete, versión/idempotencia/auditoría/evento en TX; protege número y snapshots. Ficha/papelera autorizadas; mutaciones/archivos de eliminadas bloqueados. Restauración conserva datos y devuelve fecha servidor. Utilidad canónica = ventas netas sin ISV menos inversión total; función finance.Profit compartida también por margen Quotes sin gastos adicionales. Monedas separadas, pérdidas permitidas.

Backend/UI implementados; pruebas de preservación, revocación/reauth/replay/rollback, evidencia protegida, identidad y número externo conservados sin unicidad global, backfill real MySQL, HTTP/CSRF, UI respuesta confirmada y navegador papelera/restauración/utilidad añadidas. Regresión y CI pendientes; no cerrar módulo ni iniciar Facturas hasta completar esta aceptación. Producción e integraciones externas pendientes.
