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
- OC de clientes: **Unidad 1 cerrada y validada en CI; módulo activo para la siguiente unidad**.
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

Fuera de Unidad 1 y aún pendiente:

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

## Próxima unidad — contrato de estados de OC

La documentación histórica contiene **dos grafos incompatibles** y no deben coexistir implícitamente en código:

- legacy de cabecera: `Pendiente → Listo → Entregado`, con `Facturado` y `Converted`;
- secuencia actual de OC de cliente: `Recibida → Confirmada → Comprando → En tránsito → Recibido → Preparando entrega → Entregado → Facturado → Cerrado`.

También existe nomenclatura legacy de ítems (`Pendiente`, `Comprado`, `Received`, `Entregado`) frente a la secuencia más reciente (`pendiente`, `comprado`, `enviado`, `recibido`, `entregado`).

**Regla:** no implementar ninguna transición adicional hasta congelar un contrato normativo que defina estados, transiciones permitidas, permisos, reautenticación, invariantes, eventos y tratamiento de valores legacy. La Unidad 1 permanece limitada a `Recibida`/`Pendiente`.

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

## Próximo paso exacto

Cerrar el **contrato normativo de estados de Órdenes de Compra de clientes** antes de modificar el runtime. Comparar las dos nomenclaturas existentes, definir el grafo vigente y dejar explícito cómo se tratarán los estados legacy. Después implementar la siguiente unidad con migración/servicio/REST/UI/pruebas propias, sin iniciar todavía Facturas ni órdenes a proveedores.