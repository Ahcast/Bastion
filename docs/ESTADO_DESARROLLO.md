# Estado operativo de desarrollo — Bastion

Este archivo es el **checkpoint corto para reanudar desarrollo**. La documentación de arquitectura, contratos, decisiones y runbooks sigue siendo normativa; este archivo indica dónde quedó el trabajo real y qué debe hacerse después.

Actualizado: 2026-10-10.

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
- OC de clientes: **módulo activo siguiente**.
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

Correcciones de cierre:

- el namespace FX quedó separado en `/quote-fx/observations` para evitar colisión de rutas;
- reautenticación/ARIA del ledger FX y pruebas navegador quedaron estabilizadas;
- bulk conserva selección válida al refrescar filas y usa la versión recargada;
- integración MySQL prueba dos lotes de estado consecutivos: primero exitoso y segundo con conflictos controlados, sin bloqueo;
- Playwright espera el reset real de la selección entre acciones bulk, sin sleeps ni reducción de aserciones.

### HEAD final certificado de Cotizaciones

HEAD: `0a98bf756c4e7a3965b7b625473258509fbaa571`.

Core checks: run `38094367694`, conclusión `success`:
- Go format/vet: aprobado;
- `go test -race -count=1 ./...` contra MySQL 5.7: aprobado;
- builds Go: aprobados;
- Angular build/tests: aprobados;
- Playwright contra API + Angular + MySQL reales de CI: aprobado.

También aprobados para el mismo HEAD:
- Documentation checks: run `38094367696`;
- Single branch policy: run `38094367728`.

Este CI sustituye como evidencia de cierre a los checkpoints anteriores de Cotizaciones. No acredita S3, SMTP, Percona/Bluehost ni despliegue productivo reales.

## Módulo activo — Órdenes de Compra de clientes

Fuente funcional: `docs/modules/purchase-orders.md`. El maestro y `development/decisions.md` resuelven la nomenclatura vigente:

- el menú **Órdenes de Compra** significa órdenes recibidas de clientes;
- tablas canónicas: `customer_purchase_orders` y `customer_purchase_order_items`;
- REST: `/purchase-orders`;
- las futuras órdenes emitidas por Castillo IT a proveedores pertenecen a `supplier_purchase_orders` y no se mezclan en este módulo;
- no crear tablas genéricas `orders`/`order_items`.

El documento de módulo conserva estados legacy y una secuencia actual más amplia. Para no mezclar grafos incompatibles, la primera unidad implementará únicamente el agregado íntegro de OC de cliente con estado inicial `Recibida`; las transiciones se cerrarán en una unidad posterior con contrato/pruebas propios.

### Primera unidad a implementar

1. Definir contrato exacto de cabecera/ítems, permisos, índices y DTO.
2. Crear migración global siguiente, compatible con MySQL 5.7.
3. Implementar servicio Go para alta, consulta, listado y edición versionada; una OC y sus ítems se guardan en una sola transacción.
4. Implementar cálculo canónico exacto en Go usando DECIMAL/cadenas, sin `float64` financiero.
5. Registrar auditoría/hecho e idempotencia; IDs JSON como cadenas y `409` en versión obsoleta.
6. Exponer REST y contrato OpenAPI; Angular en español después de cerrar backend de la unidad.
7. Probar unitarias, integración MySQL, autorización, navegador y CI antes de añadir estados, conversiones, documentos o factura.

Fuera de esta primera unidad: grafo completo de estados, entrega, cotización→OC, OC→factura, documentos, clone, AI y OC a proveedores.

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

Cerrar el **contrato mínimo de la unidad 1 de OC de clientes** a partir del maestro, modelo de datos, decisiones y spec `purchase-orders.md`; después implementar migración + servicio Go + pruebas de invariantes. No implementar todavía transiciones, conversiones, documentos ni Facturas.