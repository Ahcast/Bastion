# Ejecutar Órdenes de Compra de clientes

Requiere Core, migraciones globales hasta `0016_customer_purchase_order_states.sql` en MySQL 5.7 aislado y Angular. Seguir [Core](core-runbook.md). No ejecutar seeds, pruebas ni migraciones contra datos reales.

`BASTION_PURCHASE_ORDERS_ENABLED=true` registra el módulo; deshabilitarlo retira todas sus rutas y conserva los datos. Clientes/Proveedores son opcionales: los snapshots manuales funcionan sin sus permisos; un ID requiere el propietario habilitado y acceso autorizado.

[Agregado](../contracts/purchase-orders.md), [estados](../contracts/purchase-orders-states.md) y [OpenAPI](../contracts/purchase-orders-openapi.json). Importes/cantidades/versiones viajan como cadenas. Cabecera e ítems se guardan en una transacción. HNL y USD separados, sin FX implícito.

## Operación

Asignar permisos explícitos `ordenes.view/create/update` para alta/consulta/edición. La edición completa sólo se admite en **Recibida** y con todos los ítems **Pendiente**; después no se reemplazan los detalles ni se reinicia el avance.

Asignar `ordenes.transition` para avance adyacente de la cabecera hasta **Preparando entrega**, `ordenes.items.transition` para avance adyacente de cada ítem y `ordenes.deliver` para entregar la OC. Cada acción también exige lectura de la OC y mantiene scopes `self/team/all`. Los roles existentes no reciben estos permisos automáticamente.

Guardar o descartar cambios antes de transicionar. Completar el avance de cada ítem hasta **Entregado**. La cabecera no se calcula automáticamente desde los ítems; realizar su avance explícito. Para entregar la OC, todos sus ítems deben estar entregados y el operador debe confirmar su identidad con contraseña y TOTP/recuperación vigente.

Timeout: repetir misma intención/payload. Conflicto: consultar la versión actual. La UI conserva la clave ante fallo y la versión/estado confirmados si falla la recarga. Los replays se reautorizan y la entrega vuelve a exigir identidad reciente. Historial conserva actor/fecha UTC, versión, origen/destino y posición 1-based; la UI presenta hora de Honduras y página de 25 entradas.

Facturado/Cerrado son vocabulario reservado; no existen acciones para simular sus efectos. Conversiones, documentos, entrega parcial, reapertura, utilidad, clonación y OC a proveedores siguen fuera de esta unidad.

## Validación

`python scripts/check_repository.py`; `gofmt`, `go vet`, `make build` y `go test -race -count=1 ./...` con MySQL 5.7 dedicado `bastion_test`; Angular `npm ci`, build y tests. Playwright utiliza `bastion_browser_test` y el seed sintético: cálculo/alta/edición, avance de cabecera/ítems, bloqueo de entrega incompleta, confirmación de identidad, entrega y recarga persistida.

S3, SMTP, Percona Bluehost, DNS y despliegue productivos mantienen sus validaciones externas. Esta unidad no envía mensajes ni factura por cambiar un estado.
