# Contrato ejecutable de Órdenes de Compra de clientes

Unidad comercial posterior al cierre de Cotizaciones. PurchaseOrders representa únicamente **órdenes recibidas de clientes**. Tablas propias iniciales: `customer_purchase_orders` y `customer_purchase_order_items`; REST `/api/v1/purchase-orders`. Las futuras órdenes emitidas por Castillo IT a proveedores pertenecen a `supplier_purchase_orders` y no forman parte de este módulo.

La Unidad 1 implementa un agregado íntegro y versionado con estado inicial `received` (UI: **Recibida**). La Unidad 2 añade seguimiento y entrega según el [contrato normativo de estados](purchase-orders-states.md). Documentos, clonación, Cotización→OC y OC→Factura se implementarán después en unidades separadas. La sección legacy de `modules/purchase-orders.md` no autoriza a mezclar sus estados `Pendiente/Listo/Entregado` con el flujo nuevo.

## Reglas de dominio de la unidad 1

Una OC pertenece a un cliente recibido como snapshot. `customer_id` es opcional: si viene informado, PurchaseOrders debe resolver el cliente activo mediante el servicio público autorizado de Customers dentro de la misma transacción y persistir su snapshot; no lee ni escribe tablas privadas de Customers directamente. Sin ID se permite cliente manual, sin crear un maestro automáticamente.

`order_number` es el número externo de la OC del cliente. Es obligatorio, se conserva literalmente salvo trim de extremos y **no se presume globalmente único**, porque no existe una regla aprobada que garantice unicidad entre clientes/instituciones. La idempotencia evita duplicar una misma intención técnica; una regla comercial de detección de duplicados se definirá aparte si corresponde.

Campos editables de cabecera en esta unidad:

```text
order_number
customer { id?, name, rtn, contact, phone, email, address }
destination
order_date
required_delivery_date nullable
currency = HNL | USD
comments
general_discount_type = amount | percent
general_discount
exoneration_reference
additional_expenses
team_id nullable
version
items[]
```

`source_quote_id` puede existir nullable en persistencia para preparar trazabilidad futura, pero esta unidad no expone ni ejecuta conversión Cotización→OC. No se crea referencia a Facturas hasta que exista el módulo propietario.

Cada ítem editable contiene:

```text
description
unit
quantity
unit_sale_price
unit_cost
discount_type = amount | percent
discount
tax_type = isv15 | isv18 | exempt | exonerated
supplier_id nullable
supplier_name_snapshot
tracking
comments
```

El estado técnico de ítem creado por esta unidad es `pending` (UI: **Pendiente**). Esta unidad no expone transición a `purchased`; el futuro cambio de estado tendrá permiso y prueba propios. Si `supplier_id` se informa, Suppliers debe resolver un proveedor activo/autorizado y PurchaseOrders conserva ID, versión y nombre snapshot; sin ID se permite nombre manual sin crear proveedor.

Máximo inicial: 200 ítems, igual que Cotizaciones. Textos, correo, RTN, IDs y fechas siguen las validaciones Core/Customers ya aprobadas; fecha de negocio `DATE`, timestamps UTC.

## Cálculo canónico

Nunca `float64` para importes/cantidades. DTO JSON usa strings decimales. Cantidad `DECIMAL(18,4)`, precio/costo `DECIMAL(18,6)`, totales `DECIMAL(18,2)`. Redondeo monetario half-up a dos decimales.

La parte de venta debe producir exactamente la misma semántica certificada por el calculador de Cotizaciones versión 1:

```text
gross
item_discount
general_discount
base_15
base_18
exempt
exonerated
isv_15
isv_18
total
```

Incluye descuento absoluto por unidad o porcentaje, distribución proporcional determinista del descuento general y exigencia de referencia para ítems `exonerated`. PurchaseOrders no reimplementará una variante fiscal divergente; el código común debe conservar los vectores de prueba de Quotes.

La parte de inversión de esta unidad es:

```text
products_investment = Σ round(quantity × unit_cost, 2)
additional_expenses = importe explícito >= 0
investment_total = products_investment + additional_expenses
```

No se define ni persiste `profit` en esta primera unidad: `modules/purchase-orders.md` exige una definición única compartida, pero no fija normativamente si impuestos/descuentos/gastos participan de la utilidad. Se incorporará cuando esa regla esté cerrada; no se inventa en código.

## Persistencia y concurrencia

`customer_purchase_orders` conserva snapshot de cliente, destino, fechas, moneda, comentarios, descuento/exoneración, estado, cálculo de venta, inversión, `version`, propietario/equipo, actor y timestamps. `customer_purchase_order_items` conserva posición, datos editables, snapshot de proveedor, estado de compra y resultados calculados de línea.

Cabecera e ítems se crean/reemplazan en **una sola transacción**. Una OC no puede quedar parcial. POST/PATCH son idempotentes; PATCH exige `version` exacta y devuelve 409 si está obsoleta. IDs BIGINT se exponen como strings. Auditoría y `domain_events` se confirman en la misma transacción del agregado. No hay jobs/correos/AI implícitos.

No se crea una OC vacía en esta unidad: requiere al menos un ítem. El futuro borrador de conversión mencionado por el spec tendrá una regla explícita independiente.

## Permisos de la unidad 1

Registrar únicamente capacidades implementadas:

```text
ordenes.view
ordenes.create
ordenes.update
```

Scopes `all/self/team`; propietario = `created_by`, equipo = `team_id`. Acceso ajeno se redacta como 404 siguiendo módulos existentes. Crear en equipo ajeno exige `ordenes.create all`; cambiar `team_id` exige `ordenes.update all` y reautenticación. No registrar aún `ordenes.delete`, `ordenes.deliver` ni `ordenes.convert` como capacidades funcionales de esta unidad.

Identidades técnicas no reciben escritura. No hay MCP de escritura en esta unidad.

## REST de la unidad 1

```http
POST /api/v1/purchase-orders/calculate
GET  /api/v1/purchase-orders
POST /api/v1/purchase-orders
GET  /api/v1/purchase-orders/{id}
PATCH /api/v1/purchase-orders/{id}
```

`calculate` no persiste y reutiliza el cálculo canónico. Listado paginado devuelve resumen, no el agregado incompleto. Filtros iniciales: búsqueda literal por número/cliente/descripción/comentario, `customer_id`, estado (`received`), total mínimo/máximo, página y `per_page`; total y data comparten el mismo predicado autorizado.

POST/PATCH requieren `Idempotency-Key`. Errores siguen Core: 401/403/404/409/422. La Unidad 2 añade exclusivamente las tres rutas de transición/historial del contrato de estados. No existe DELETE ni endpoints de conversión/documentos hasta su unidad correspondiente.

## DTO de salida

La ficha devuelve:

```text
id
order_number
status
customer snapshot
source_quote_id nullable
destination
order_date
required_delivery_date nullable
currency
comments
general_discount_type
general_discount
exoneration_reference
calculation {
  gross, item_discount, general_discount,
  base_15, base_18, exempt, exonerated,
  isv_15, isv_18, total,
  products_investment, additional_expenses, investment_total,
  calculator_version
}
version
created_by
team_id
items[]
```

El resumen de listado contiene `id`, `order_number`, cliente, fecha, moneda, total, estado y versión.

## Aceptación de la unidad 1

Antes de implementar estados/transiciones:

- migración reproducible MySQL 5.7, sin tablas genéricas `orders`;
- cálculo comparte/vigila los mismos vectores exactos de Quotes y prueba inversión/gastos sin float;
- create/update atómico con ítems, idempotencia, versión obsoleta y rollback de auditoría/evento;
- scopes all/self/team, cambio de equipo sensible, revocación y IDOR;
- snapshot de cliente y proveedor autorizado sin escrituras cruzadas;
- filtros/paginación sin límites silenciosos y BIGINT como string;
- REST estricto + contrato OpenAPI;
- Angular en español y Playwright real contra API/MySQL;
- regresión completa y CI verde del mismo HEAD.

No desplegar ni migrar producción por aprobar esta unidad.
## Unidad 3 — conversión desde Cotizaciones

Implementación y reglas en el [contrato único de conversión](quote-purchase-order-conversion.md). Esta unidad incorpora Cotización→OC mediante propietarios, sin rutas de conversión en `/purchase-orders`. La ficha añade `source_quote_version` nullable string e inmutable junto a `source_quote_id`. Edición posterior conserva ambos campos. Documentos/clonación de OC e Invoices siguen pendientes.

## Documentos privados OC e identidad de ítems

Contrato propietario vigente: [documentos privados OC](purchase-order-documents.md). Migración 0018 y vínculos durables a IDs de ítem; edición/reordenamiento preserva identidad, papelera no permite eliminar el ítem vinculado.

## Clonación OC

Contrato vigente: [clonación OC](purchase-order-cloning.md). Migración 0019 conserva origen/versión, reinicio operacional y relación inmutable sin copiar adjuntos.
