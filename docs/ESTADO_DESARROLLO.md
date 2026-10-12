# Estado operativo de desarrollo — Bastion

Este archivo es el **checkpoint corto para reanudar desarrollo**. La documentación de arquitectura, contratos, decisiones y runbooks sigue siendo normativa; este archivo indica dónde quedó el trabajo real y qué debe hacerse después.

Actualizado: 2026-10-12.

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
- OC de clientes: **Unidades 1..6 y aceptación integral cerradas y validadas en CI**.
- Facturas/pagos: módulo activo, unidades 1 financieras, 2 Borradores, 3 emisión y 4 pagos certificadas; siguiente unidad 5 conversiones por propietarios. Gastos, Reportes y Colaboración siguen después.
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

## 2026-10-11 — OC: aceptación integral cerrada

HEAD certificado `a36de697a4c0d39d16f7095a76d443258deec3f5`. [Core checks 38175683879](https://github.com/Ahcast/Bastion/actions/runs/38175683879), [documentación 38175683895](https://github.com/Ahcast/Bastion/actions/runs/38175683895) y [rama única 38175683826](https://github.com/Ahcast/Bastion/actions/runs/38175683826): success. Go fmt/vet, pruebas race contra MySQL 5.7 y builds server/worker/admin aprobados; Angular build y **77 pruebas**; navegador **10/10**.

Papelera/restauración preserva identidad, número externo, ítems, snapshots y cálculo; bloquea cambios operativos, documentos, conversión e historial protegidos; replay reautoriza permiso/reauth vigente y rollback conserva versión. Número externo sin unicidad global. Utilidad exacta sin ISV menos inversión total, negativa permitida y monedas separadas; backfill 0020 validado. En navegador, clon independiente con gastos 9.99 conserva utilidad 85.01 HNL tras eliminar, recargar y restaurar. Fixture sintético posee ordenes.delete explícito; no grants productivos automáticos.

Unidades 1..6 y alcance operativo propietario de OC cerrados. OC→Factura/Facturado/Cerrado, búsqueda fiscal, Reportes/Inicio e IA se incorporan con sus propietarios según matriz de aceptación. Siguiente módulo: **Facturas/pagos**, comenzando por contrato financiero y borradores; emisión fiscal exige contrato de series/SAR y configuración validada. No producción ni integraciones externas certificadas.

## 2026-10-11 — Facturas/pagos, unidad 1 en validación

Contrato propietario `docs/contracts/invoices.md` definido antes del código. Motor interno InvoiceFinancials reutiliza finance.Calculate y finance.Profit; cálculo mixto ISV15/18/exento/exonerado, descuentos, inversión por línea/gastos y utilidad exacta negativa. Moneda explícita HNL/USD. ProjectPayments/ApplyPayment proyectan abonos, saldo y pago completo sin floats ni sobrepago, también por encima de 2^53. No escriben pagos, cambian estados, asignan número/SAR ni exponen acciones.

Archivos: backend/internal/modules/invoices/financials.go y financials_test.go. Unidad pura sin migración ni permisos/rutas nuevos. Formato WASM gofmt, documentación y diff aprobados localmente; Go fmt/vet/race y regresión MySQL/Angular/navegador pendientes en CI. No declarar facturación operativa. Después de certificar: contrato/migración y agregado Borrador con REST/UI, autorización, versionado y evidencia. Series/SAR e integración productiva permanecen pendientes.

## 2026-10-11 — Facturas/pagos, unidad 1 certificada

HEAD `c85a1e37e0d120d84415e02299c48dbebbc04588`: [Core checks 38176231851](https://github.com/Ahcast/Bastion/actions/runs/38176231851), [documentación 38176231870](https://github.com/Ahcast/Bastion/actions/runs/38176231870) y [rama única 38176231869](https://github.com/Ahcast/Bastion/actions/runs/38176231869): success. Go fmt/vet/race, regresión MySQL 5.7 y builds server/worker/admin aprobados; Angular build/**77 pruebas** y navegador **10/10**. Cuatro pruebas propietarias financieras cubren impuestos mixtos/descuentos/inversión/utilidad negativa/monedas, límites, abonos/sobrepago/pago completo y precisión por encima de 2^53.

Unidad 1 cerrada. Facturas/pagos sigue activo: siguiente unidad, Borradores persistentes con REST/UI y seguridad/evidencia; definir contrato y migración antes del código. El motor no expone acciones, registra pagos ni emite documentos fiscales. Producción, series/SAR y demás unidades siguen pendientes.

## 2026-10-11 — Facturas/pagos, unidad 2 Borradores en validación

Contrato/OpenAPI y migración 0021 definidos: invoices/invoice_items normalizados, cabecera y cliente manual/registrado, snapshots históricos, contado/crédito, moneda HNL/USD, notas públicas y cálculo/inversión/utilidad del motor certificado. Número/SAR nullable y sin contador/emisión. Saldo inicial igual al total, pagos cero; no cobro real.

Servicios REST create/get/list/update/calculate, permisos facturas.view/create/update sin grants, scopes all/self/team, versión/idempotencia/TX/auditoría/eventos e IDs de ítems preservados al editar/reordenar. Cambio de equipo requiere all+reauth; replay reautoriza destino. Flag BASTION_INVOICES_ENABLED, composición opcional Customers y UI española con menú/ruta protegidos. Cliente conserva clave tras ACK incierto, resultado confirmado si falla recarga y bloquea writes hasta recuperar la ficha; vista previa obsoleta se oculta si cambia cualquier dato financiero.

Pruebas: cuatro financieras certificadas; validación DTO/condiciones, cinco integraciones nuevas (round-trip/IDs, snapshots/revocación/scopes, 12 replays y ocho ediciones concurrentes/rollback, HTTP/CSRF/estado protegido, equipos/reauth/revocación), composición/flag, cinco Angular y escenario navegador real de Facturas. Localmente Angular build y **82 pruebas** aprobados; documentación/diff y compilación de **11 escenarios**. CI Go fmt/vet/race/MySQL/builds, Angular y navegador pendientes; no cerrar esta unidad hasta verde. Emisión fiscal, pagos auditados, conversiones, documentos/anulación/clonación y aceptación integral siguen en orden. No despliegue ni migración productiva.

## 2026-10-11 — Borradores: corrección de accesibilidad y selección

Primera ejecución 11f9dae5fa7a09b3afb4b7519f30c563dcfa2369 / Core checks 38177764409: backend y frontend success; navegador 10/11, falla únicamente al localizar etiqueta exacta del selector de condición de pago. Etiquetas for/id separan opciones del nombre accesible; nueva prueba DOM elige Crédito. Selección de cliente por nombre consulta Customers solo con permisos/flag, conserva snapshot al repetir identidad y recupera anterior ante revocación; selector de equipo por nombre solo con core.teams.read y elecciones filtradas por membresía/scopes. Sin pedir IDs en el formulario. Tres pruebas Angular adicionales: accesibilidad, catálogo/snapshot/revocación y consultas denegadas. Build y **85 pruebas** locales aprobados; repetir CI completo sobre el commit corregido antes de cierre.

## 2026-10-11 — Facturas/pagos, unidad 2 Borradores certificada

HEAD `88577fa0fa810958a65589beda3d9e331b8c3317`: [Core checks 38179176643](https://github.com/Ahcast/Bastion/actions/runs/38179176643), [documentación 38179176704](https://github.com/Ahcast/Bastion/actions/runs/38179176704) y [rama única 38179176676](https://github.com/Ahcast/Bastion/actions/runs/38179176676): success. Go fmt/vet/race, MySQL 5.7 y builds server/worker/admin aprobados; Angular build y **86 pruebas**; navegador **11/11** (32.5 s).

Borradores persistentes con REST/UI, contado/crédito, snapshots históricos, ítems estables, cálculo y saldos exactos, permisos/scopes, reauth al cambiar equipo, idempotencia concurrente, rollback y recuperación de respuesta confirmada validados. Facturas no asigna número/SAR, cobra ni cambia estados fiscales. Se corrigió la actualización del destinatario de correo Quotes y se comprueba su valor antes de autorizar; regresión real de PDF, aprobación y worker/SMTP sintético aprobada. Ninguna prueba acredita correo real ni facturación productiva.

Unidad 2 cerrada. Siguiente: unidad 3, configuración fiscal/correlativo/emisión, empezando por política interna de rango, vigencia y formato; después persistencia transaccional, permisos, REST/UI y emisión. Configuración SAR real, autorización del rango, formato legacy y validación fiscal productiva continúan pendientes; no inventar datos ni reglas legales.

## 2026-10-11 — Facturas, unidad 3: política fiscal interna en validación

Contrato previo en contracts/invoices.md. fiscal_policy.go implementa validación de configuración explícita, rango inclusivo, vigencia actual y fecha factura, moneda y formato preservado; proyección exacta de número/siguiente/snapshot sin mutaciones. Pruebas cubren agotamiento, overflow, fechas imposibles, backdating tras vencimiento y precisión mayor que 2^53. Bloque puro, sin migración, rutas, grants, contador persistido ni emisión. Gofmt WASM/documentación/diff locales aprobados; Go/CI pendiente.

Siguiente bloque de esta misma unidad, tras certificar: contrato/migración de configuración propietaria, document_counters transaccional, permisos/reauth/idempotencia y emisión REST/UI con snapshot histórico. No cambiar unidad ni declarar emisión implementada por esta proyección. Pendientes productivos concretos: autorización SAR, rango/fecha límite, formato legacy y modalidad/moneda fiscal aplicable. Consulta al portal oficial no sustituye esos datos ni validación legal; no activar producción.

## 2026-10-11 — Facturas, unidad 3: política interna certificada

HEAD `5d03d523f5340acf55e87c4659305ef445fe1f54`: [Core checks 38179768823](https://github.com/Ahcast/Bastion/actions/runs/38179768823), [documentación 38179768828](https://github.com/Ahcast/Bastion/actions/runs/38179768828) y [rama única 38179768819](https://github.com/Ahcast/Bastion/actions/runs/38179768819): success. Go fmt/vet/race/MySQL/builds, Angular build/86 pruebas y navegador 11/11 aprobados. Política pura de rango/vigencia/formato/moneda y precisión exacta certificada; no emisión ni contador persistido.

La unidad 3 sigue abierta. Próximo bloque: configuración persistente bajo contrato previo añadido a contracts/invoices.md, migración 0022 y contador document_counters, permisos/reauth/REST/UI y evidencia atómica. Después emisión. Activación fiscal productiva y autorización real continúan pendientes.

## 2026-10-11 — Facturas, unidad 3: configuración persistente en validación

Migración 0022 invoice_fiscal_settings y permisos fiscal.read/manage sin grants; configuración y primer contador document_counters.invoice atómicos. GET/PUT propietario con DTO estricto, permisos globales, humano/reauth/CSRF, versión/idempotencia/audit/event. Editar no cambia contador; protege formato tras numeración y snapshots históricos. Bootstrap/replay/rollback/error de locks protegidos. UI en Facturas, sin nuevo menú, solicita consulta explícita y reauth; clave de respuesta incierta conservada, resultado confirmado bloquea otros writes hasta recargar. PUT incluido en CORS/Client y probado.

Cuatro integraciones MySQL nuevas: scopes/revocación/reauth y versión, 12 replays concurrentes y rollback alta/edición, agotamiento/formato histórico, HTTP DTO/CSRF/reauth/CORS. Cuatro pruebas Angular nuevas y navegador con operador separado de configuración, sin crear/cobrar/emitir facturas. Build y **90 pruebas** locales aprobados; CI Go/race/MySQL/builds y navegador **12 escenarios** pendientes. No cerrar bloque ni iniciar código de emisión hasta verde. Producción y autorización fiscal real pendientes.

Configuración fiscal: UI distingue autorización deshabilitada/rango agotado usando enteros exactos; conserva estado de última consulta al editar. Prueba adicional cubre límite 2^53 y edición pendiente. Scopes self no habilitan configuración global. **91 pruebas Angular** locales aprobadas; nueva revisión requiere CI completo antes de cierre.

Prueba de configuración fiscal: el login contraseña+MFA ya constituye reauth reciente, como establece Core; el recorrido ejerce confirmación explícita con código de recuperación sintético independiente. Rechazo de sesión sin reauth y replay tras revocación siguen cubiertos en MySQL/HTTP. Primera ejecución 38180612222: Angular success y navegador 11/12; fallo era la expectativa incorrecta sobre login, no omisión del control backend. Repetir CI sobre la corrección antes de cerrar.

## 2026-10-11 — Facturas, unidad 3: configuración persistente certificada

HEAD `133b9dc05ef5c5465d13e50550437c86a1bdb940`: [Core checks 38180883877](https://github.com/Ahcast/Bastion/actions/runs/38180883877), [documentación 38180883969](https://github.com/Ahcast/Bastion/actions/runs/38180883969) y [rama única 38180883848](https://github.com/Ahcast/Bastion/actions/runs/38180883848): success. Go fmt/vet/race/MySQL 5.7 y builds server/worker/admin aprobados; Angular build/**91 pruebas**; navegador **12/12**. Cuatro integraciones nuevas validan configuración/counter atómicos, rollback alta/edición, 12 replays, versión/scopes/revocación/reauth, formato consumido, agotamiento y HTTP/CSRF/CORS.

La UI distingue estado consultado/edición pendiente, conserva clave ante ACK incierto y bloquea nuevo guardado tras confirmación hasta recargar. Operador de configuración sintético separado no crea/cobra/emite facturas. Login con contraseña+MFA cuenta como confirmación reciente; navegador también usa confirmación explícita de recuperación y backend rechaza sesiones sin reauth.

Bloque de configuración cerrado; unidad 3 sigue activa y continúa emisión. Contrato de emisión previo definido en contracts/invoices.md: migración 0023, versiones/snapshots de empresa y autorización, vencimiento y correlativo/evidencia en una TX. Aún sin código de emisión/pagos/PDF. Autorización/modalidad/moneda fiscal reales y despliegue productivo siguen pendientes.

## 2026-10-11 — Facturas, unidad 3 emisión en validación

Contrato/OpenAPI previos y migración 0023: datos de emisión/empresa/vencimiento nullable en Borrador; facturas.emit sin grants productivos. EmissionContext/Emit humanos por scopes view/emit, reauth/CSRF/versiones/idempotencia. Valida configuración/vigencia/fecha Honduras/moneda/cálculo e ítems exactos y empresa con RTN explícito; no corrige importes silenciosamente. Número/estado issued, snapshots fiscal/empresa y versiones, vencimiento, contador/audit/event en una TX. Replay reautoriza registro/acción/reauth y conserva evidencia aun tras configuración posterior. Emisión no registra pagos ni PDF.

UI revisa datos guardados, etiqueta número estimado sin reservar, conserva claves/respuesta confirmada y congela ficha mientras emite o hasta recuperar recarga fallida. Listado/ficha usan número fiscal y estado Emitida; mostrados CAI/empresa históricos. Dos pruebas de reloj/vencimiento, cinco integraciones MySQL (con subcasos de versiones/vigencia/cálculo/moneda/scopes/HTTP), 12 replays y seis emisiones por dos humanos concurrentes/rollback. Seis pruebas Angular nuevas (97 totales), compilación local aprobada y escenario navegador de emisión con operadores separados y API real. CI Go fmt/vet/race/MySQL/builds, Angular y **13 recorridos** pendientes; no cerrar emisión ni empezar pagos hasta verde.

Navegador usa datos sintéticos: empresa con RTN configurado por administrador, autorización configurada por fiscal, emisor self independiente, lectura después de cambiar CAI conservando snapshot. No activación productiva, validación legal de RTN/CAI ni envío al SAR. Producción requiere autorización real/rango/modalidad/moneda y revisión fiscal.


## 2026-10-11 — Facturas, unidad 3 emisión certificada; continúa unidad 4

Código certificado `fa8670bcf68c11a07a2ed9ff7aedc7d23a4bc890`: [Core checks 38183414256](https://github.com/Ahcast/Bastion/actions/runs/38183414256), [documentación 38183414230](https://github.com/Ahcast/Bastion/actions/runs/38183414230) y [rama única 38183414289](https://github.com/Ahcast/Bastion/actions/runs/38183414289): success. Go fmt/vet/race con MySQL 5.7 y builds server/worker/admin aprobados; Angular build y **97 pruebas**, navegador **13/13 en 44.4 s**. Emisión conserva snapshots fiscal/empresa, valida tres versiones/cálculo/vigencia/moneda/fecha Honduras, asigna correlativo con evidencia atómica y protege replays/scopes/reauth/rollback/concurrencia. Recorrido sintético verifica RTN persistido antes de revisión y CAI histórico tras modificar configuración.

Unidad 3 cerrada para desarrollo sintético. Continúa unidad 4 bajo contrato previo y OpenAPI de payments: cobros auditables, anulación sin borrado y proyección comercial exacta. Unidad 4 aún sin implementación; conversiones, documentos/anulación/clonación y aceptación integral siguen en orden. Ningún recibo/pago bancario/Siafi externo ni validación fiscal/despliegue productivo acreditados.


## 2026-10-11 — Facturas, unidad 4 pagos en validación

Contrato previo 2c733b22a84ba3436ed88553f6c1a7dac7bc985a y migración 0024 payments; permisos payment/payment.void sin grants productivos. Ledger normalizado inmutable/anulable con versión, fecha/método/moneda/importes/referencia/razón y receipt_number NULL. Servicio humano view/action con reauth/CSRF/versión/intención; factura bajo lock, saldo recalculado desde filas activas, no sobrepago ni FX/retenciones. Register/void cambian exclusivamente proyecciones/versiones con audit/event y evidencia atómica; rollback y replay reautorizados. Estado documental issued se conserva y commercial_status deriva Pendiente/Parcialmente pagada/Pagada/Vencida en reloj Honduras; filtro/listado/ficha y ledger sin agregados entre monedas.

UI consulta explícitamente ledger vigente, registra/anula con razón y reauth, conserva clave ACK incierto y resultado confirmado si falla recarga, bloquea padre durante HTTP y nuevas mutaciones hasta recuperar versión. Seis integraciones MySQL nuevas cubren persistencia/anulación/cuatro métodos, total cero/importes >2^53, versiones/invalidaciones/moneda/fechas, 12 replays, ocho cobros en contención por dos humanos, rollback register/void, HTTP/CSRF/scopes/reauth/revocación. Prueba pura de vencimiento y prioridades; seis Angular nuevas. Build local aprobado; 102 pruebas locales previas al último caso de padre (objetivo 103) y suite de navegador ampliada con abono/anulación/cobro completo y contador/snapshots inmutables. Go fmt/vet/race/MySQL/builds, Angular y navegador completos pendientes; no cerrar pagos ni empezar conversiones hasta CI verde. Sin recibos/PDF/Siafi bancario ni validación fiscal productiva.


## 2026-10-12 — Facturas, unidad 4 pagos certificada

Código certificado `0db033c44d57326d42bb1e83459b5a1709a04c46`: [Core checks 38195876549](https://github.com/Ahcast/Bastion/actions/runs/38195876549), [documentación 38195876513](https://github.com/Ahcast/Bastion/actions/runs/38195876513) y [rama única 38195876471](https://github.com/Ahcast/Bastion/actions/runs/38195876471): success. Go fmt/vet/race con MySQL 5.7 y builds server/worker/admin aprobados; Angular build y **103 pruebas**; navegador **13/13**. Ledger de abonos/anulaciones, saldo exacto, sobrepago, scopes/revocación/reauth, rollback, idempotencia y cobros concurrentes certificados. Número/correlativo y snapshots fiscales/empresa se conservan; ningún recibo ni integración bancaria/Siafi real acreditados.

Las fallas previas de aceptación provenían de claves de prueba inferiores al mínimo Core y lectura de una emisión incompleta fabricada por el test; se corrigieron escenarios sin relajar reglas productivas. La corrección inicial cambió accidentalmente dos nombres version del test HTTP; se restauraron y la suite íntegra final aprobó. Unidad 4 cerrada. Siguiente unidad 5: conversiones Cotización→Factura y OC→Factura por servicios propietarios, contrato/migración previos y evidencia transaccional/idempotente. Documentos/anulación/clonación y aceptación integral mantienen su orden; Facturas completo y producción todavía pendientes.
