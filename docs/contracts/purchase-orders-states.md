# Contrato de estados — Órdenes de Compra de clientes

Este contrato continúa `purchase-orders.md` después del cierre certificado de la Unidad 1. Aplica únicamente a `customer_purchase_orders` y `customer_purchase_order_items`: órdenes recibidas de clientes. No define órdenes emitidas por Castillo IT a proveedores.

## 1. Fuente y precedencia

La especificación histórica conserva dos vocabularios incompatibles. Para el runtime nuevo se adopta como **canónica** la secuencia más reciente y repetida en la presentación vigente del módulo:

```text
Recibida
Confirmada
Comprando
En tránsito
Recibido
Preparando entrega
Entregado
Facturado
Cerrado
```

Los valores legacy `Pendiente`, `Listo`, `Entregado`, `Facturado` y `Converted` no son nuevos destinos alternativos del runtime. Su conservación se trata como compatibilidad/migración histórica; no autoriza a mantener dos máquinas de estado paralelas.

Unidad 1 ya persiste `received` = **Recibida**. Este contrato conserva ese identificador.

## 2. Estados canónicos de cabecera

| Código técnico | UI | Alcance |
| --- | --- | --- |
| `received` | Recibida | implementado en Unidad 1 |
| `confirmed` | Confirmada | Unidad 2 |
| `purchasing` | Comprando | Unidad 2 |
| `in_transit` | En tránsito | Unidad 2 |
| `goods_received` | Recibido | Unidad 2 |
| `preparing_delivery` | Preparando entrega | Unidad 2 |
| `delivered` | Entregado | Unidad 2, acción sensible |
| `invoiced` | Facturado | reservado al módulo Facturas/integración OC→Factura |
| `closed` | Cerrado | reservado hasta definir cierre con Facturas/Pagos |

`received` y `goods_received` son conceptos distintos: el primero significa que Castillo IT recibió la OC del cliente; el segundo, que la compra física necesaria para atenderla ya fue recibida.

## 3. Grafo habilitado en Unidad 2

La Unidad 2 implementa únicamente avance adyacente y hacia adelante:

```text
received
  -> confirmed
  -> purchasing
  -> in_transit
  -> goods_received
  -> preparing_delivery
  -> delivered
```

No se permiten en esta unidad:

- saltos de estados;
- retrocesos/reaperturas;
- transición manual a `invoiced`;
- transición manual a `closed`;
- `Converted` como estado;
- cambios automáticos ejecutados por IA/jobs;
- cambios implícitos de cabecera al editar datos o ítems.

Esta política es deliberadamente restrictiva. Si operación real requiere saltos, cancelación, reapertura, entrega parcial u otras ramas, deben agregarse como transición explícita con contrato y prueba propios; no se infieren.

## 4. Estados canónicos de ítem

El vocabulario vigente de ítem es:

| Código técnico | UI |
| --- | --- |
| `pending` | Pendiente |
| `purchased` | Comprado |
| `shipped` | Enviado |
| `received` | Recibido |
| `delivered` | Entregado |

Unidad 1 ya crea `pending`. Unidad 2 habilita sólo avance adyacente:

```text
pending -> purchased -> shipped -> received -> delivered
```

No existen saltos, retrocesos ni estados automáticos de ítem en esta unidad.

## 5. Relación entre estado de cabecera e ítems

Los estados de ítem son seguimiento operativo; no recalculan automáticamente el estado de cabecera. La cabecera cambia sólo mediante una acción explícita autorizada.

Única invariante agregada de Unidad 2:

- para llevar la OC a `delivered`, todos sus ítems deben estar en `delivered`.

No se imponen otras inferencias entre cabecera e ítems porque la documentación no define si una compra puede ser parcial, local, consolidada o con múltiples proveedores. En particular, `purchasing`, `in_transit`, `goods_received` y `preparing_delivery` no se derivan silenciosamente de porcentajes/contadores de ítems.

## 6. Facturación y cierre

`invoiced` y `closed` forman parte del vocabulario canónico, pero **no son transiciones ejecutables de Unidad 2**.

- `delivered -> invoiced` sólo podrá ocurrir mediante el propietario Invoices/OC→Factura cuando ese módulo exista y persista `invoice_id`/evidencia durable.
- `invoiced -> closed` queda bloqueado hasta que el contrato de Facturas/Pagos defina qué significa cierre; no se presume que emitir, cobrar o entregar sean equivalentes.

Por tanto, PurchaseOrders no expone botones/endpoints que simulen facturación o cierre antes de sus propietarios.

## 7. Permisos y reautenticación

Añadir capacidades sólo al implementar sus efectos:

```text
ordenes.transition
ordenes.items.transition
ordenes.deliver
```

Reglas:

- `ordenes.transition`: transiciones ordinarias de cabecera hasta `preparing_delivery`;
- `ordenes.items.transition`: transición explícita de un ítem;
- `ordenes.deliver`: transición de cabecera a `delivered`;
- entrar a `delivered` exige reautenticación vigente;
- todos mantienen scopes `self/team/all` sobre la OC propietaria;
- ningún permiso se concede automáticamente a roles existentes.

`ordenes.update` no equivale a permiso de transición. La IA no recibe permisos comerciales de escritura por esta unidad.

## 8. Concurrencia, idempotencia y atomicidad

Toda transición de cabecera o ítem:

- recibe la `version` actual del agregado como string;
- exige `Idempotency-Key`;
- bloquea la OC dentro de una transacción;
- vuelve a autorizar actor, scope y estado actual dentro de la misma operación;
- valida que origen→destino sea exactamente una arista permitida;
- incrementa la versión de la OC una vez;
- persiste historial, auditoría y `domain_event` dentro de la misma transacción;
- devuelve `409` ante versión obsoleta o intención idempotente conflictiva;
- no reintenta ciegamente escrituras ambiguas.

Cambiar un ítem también incrementa la versión del agregado para que una ficha completa nunca tenga concurrencia independiente de sus detalles.

## 9. Historial durable

Crear `customer_purchase_order_status_history` cuando se implemente Unidad 2. Debe conservar como mínimo:

```text
id
purchase_order_id
item_position nullable
from_status
to_status
actor_id
purchase_order_version
created_at
```

`item_position = NULL` representa transición de cabecera; posición no nula representa el ítem de la versión afectada. No se usa el historial como fuente del estado actual: la cabecera/ítem conservan el estado vigente y el historial es evidencia append-only.

Si más adelante los ítems adquieren IDs estables, una migración futura puede añadir `purchase_order_item_id`; no se inventa ahora porque Unidad 1 reemplaza los ítems por posición.

## 10. REST de Unidad 2

Añadir únicamente:

```http
POST /api/v1/purchase-orders/{id}/transition
POST /api/v1/purchase-orders/{id}/items/{position}/transition
GET  /api/v1/purchase-orders/{id}/status-history
```

Cuerpos de transición:

```json
{
  "to_status": "confirmed",
  "version": "3"
}
```

Para ítem, `to_status` usa el vocabulario de ítem. La posición es 1-based y se valida contra el agregado bloqueado.

No añadir en esta unidad endpoints `/invoice`, `/close`, `/reopen`, `/cancel`, entregas parciales ni aliases legacy.

## 11. Eventos

Publicar hechos de dominio, no jobs implícitos:

```text
purchase_order.status_changed
purchase_order.item_status_changed
purchase_order.delivered
```

El payload mínimo identifica OC, versión, estado anterior/nuevo y actor. `delivered` puede coexistir con `status_changed` dentro de la misma transacción como hecho especializado; no dispara correo, factura, IA ni otro efecto por defecto.

## 12. Compatibilidad legacy

No ejecutar un mapeo silencioso general porque varios valores no son semánticamente equivalentes:

- legacy `Entregado` puede mapearse a `delivered` sólo cuando la migración pueda acreditar esa semántica;
- legacy `Facturado` puede mapearse a `invoiced` sólo cuando exista evidencia durable de factura;
- legacy `Pendiente` es ambiguo frente a `received`, `confirmed` o `purchasing`;
- legacy `Listo` es ambiguo frente a `goods_received` o `preparing_delivery`;
- legacy `Converted` describe una relación/acción, no un estado canónico suficiente.

La futura migración de datos reales debe conservar el valor legacy original y aplicar un mapa probado por registros/evidencia o marcar el registro para revisión. No se pierde ni se fuerza información para satisfacer el nuevo enum.

## 13. Aceptación de Unidad 2

Antes de declararla cerrada:

- migración MySQL 5.7 del historial + permisos;
- servicio puro de validación del grafo con vectores positivos/negativos;
- transición de cabecera e ítem transaccional con idempotencia, versión, scopes y auditoría;
- `delivered` exige reautenticación y todos los ítems `delivered`;
- `invoiced/closed` rechazados por PurchaseOrders mientras no exista su propietario;
- no existe transición automática por edición/IA/job;
- REST estricto + OpenAPI y guard de paridad;
- Angular muestra estado actual y sólo la siguiente acción autorizada;
- Playwright real cubre el flujo `received -> ... -> delivered` y los ítems;
- regresión completa y CI verde del mismo HEAD.

No desplegar/migrar producción por aprobar esta unidad.