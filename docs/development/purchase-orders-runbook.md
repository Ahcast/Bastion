# Ejecutar Órdenes de Compra de clientes

Requiere Core, migraciones globales hasta `0017_quote_purchase_order_conversion.sql` en MySQL 5.7 aislado y Angular. Seguir [Core](core-runbook.md). No ejecutar seeds, pruebas ni migraciones contra datos reales.

`BASTION_PURCHASE_ORDERS_ENABLED=true` registra el módulo; deshabilitarlo retira todas sus rutas y conserva los datos. Clientes/Proveedores son opcionales: los snapshots manuales funcionan sin sus permisos; un ID requiere el propietario habilitado y acceso autorizado.

[Agregado](../contracts/purchase-orders.md), [estados](../contracts/purchase-orders-states.md) y [OpenAPI](../contracts/purchase-orders-openapi.json). Importes/cantidades/versiones viajan como cadenas. Cabecera e ítems se guardan en una transacción. HNL y USD separados, sin FX implícito.

## Operación

Asignar permisos explícitos `ordenes.view/create/update` para alta/consulta/edición. La edición completa sólo se admite en **Recibida** y con todos los ítems **Pendiente**; después no se reemplazan los detalles ni se reinicia el avance.

Asignar `ordenes.transition` para avance adyacente de la cabecera hasta **Preparando entrega**, `ordenes.items.transition` para avance adyacente de cada ítem y `ordenes.deliver` para entregar la OC. Cada acción también exige lectura de la OC y mantiene scopes `self/team/all`. Los roles existentes no reciben estos permisos automáticamente.

Guardar o descartar cambios antes de transicionar. Completar el avance de cada ítem hasta **Entregado**. La cabecera no se calcula automáticamente desde los ítems; realizar su avance explícito. Para entregar la OC, todos sus ítems deben estar entregados y el operador debe confirmar su identidad con contraseña y TOTP/recuperación vigente.

Timeout: repetir misma intención/payload. Conflicto: consultar la versión actual. La UI conserva la clave ante fallo y la versión/estado confirmados si falla la recarga. Los replays se reautorizan y la entrega vuelve a exigir identidad reciente. Historial conserva actor/fecha UTC, versión, origen/destino y posición 1-based; la UI presenta hora de Honduras y página de 25 entradas.

Facturado/Cerrado son vocabulario reservado; no existen acciones para simular sus efectos. OC→Factura, entrega parcial, reapertura, utilidad, clonación de OC y OC a proveedores siguen pendientes. La conversión desde Cotizaciones se explica más abajo.

## Validación

`python scripts/check_repository.py`; `gofmt`, `go vet`, `make build` y `go test -race -count=1 ./...` con MySQL 5.7 dedicado `bastion_test`; Angular `npm ci`, build y tests. Playwright utiliza `bastion_browser_test` y el seed sintético: cálculo/alta/edición, avance de cabecera/ítems, bloqueo de entrega incompleta, confirmación de identidad, entrega y recarga persistida.

S3, SMTP, Percona Bluehost, DNS y despliegue productivos mantienen sus validaciones externas. Esta unidad no envía mensajes ni factura por cambiar un estado.

## Conversión desde Cotizaciones

Ambos módulos habilitados; permisos explícitos `cotizaciones.view/convert` y `ordenes.create/view` con scopes actuales. Abrir una cotización **Aceptada**, indicar número externo OC, fechas, destino, gastos y comentarios; confirmar la conversión. No existe conversión de Borrador/Aprobada internamente ni alta vacía. La fuente queda **Convertida a OC** y enlaza el destino; la OC conserva ID/versión de fuente. Revisar ambos con sus permisos independientes. No se copian PDFs/adjuntos ni notas privadas; documentos de OC tendrán su propia unidad.

Si se pierde el ACK, repetir la misma intención y datos. Otra intención tras convertir devuelve conflicto; no otra OC. La migración añade unicidad del origen: datos legacy con orígenes duplicados deben revisarse antes de una migración productiva; no se depuran automáticamente. Playwright incluye conversión USD 201.25, inversión 83.50, recarga y navegación cruzada con usuario sin permisos de catálogos.

## Documentos privados (Unidad 4)

Permisos explícitos: ordenes.view, ordenes.documents.write, core.documents.read/write con scope correspondiente; no se otorgan automáticamente. Abra una OC guardada, consulte Documentos privados y elija Cabecera o un ítem como destino. Guarde/descarte ediciones y confirme identidad para vincular, eliminar o restaurar. Carga PDF/PNG/JPEG/TXT hasta 20 MB; permanece en cuarentena hasta análisis autorizado. Disponible habilita preview y descarga Core.

Filtros por ítem y papelera, paginación de 25. Papelera conserva el vínculo; no se puede eliminar de la OC un ítem con documento incluso eliminado. Reordenar conserva el ID y el adjunto. API de edición debe enviar los IDs retornados para conservar ítems. Timeout: repetir misma intención, no crear otro documento; la UI conserva etapa de carga/clave. Permisos se reevalúan en cada petición. No se acredita S3/escáner productivo con el adapter sintético de CI.

Unidad 4 certificada en `1073c261de0c8f7ab12aed0ff6bb7b6a5f361e7c`; [Core checks 38163339892](https://github.com/Ahcast/Bastion/actions/runs/38163339892) aprobado. Clonación y cierre integral son el siguiente paso.

## 2026-10-11 — Unidad 5 OC: clonación en validación

Contrato `purchase-order-cloning.md`, migración 0019 y POST clone con DTO estricto. Snapshots comerciales, impuestos/descuentos y moneda conservados; nueva cabecera Recibida/ítems Pendiente, identidad nueva, sin documentos/entrega/vínculo Quote. Fuente inmutable y relación durable al origen/versión. Permisos view/create vigentes y scopes del equipo; replay reautoriza también destino. TX única incluye idempotencia, cabecera/ítems, trazabilidad, auditoría y eventos; UI conserva clave/destino ante fallos. No capacidades nuevas asignadas automáticamente.

Pruebas añadidas: vectores HNL/USD con 15/18/exento/exonerado y descuentos, reinicio operacional/snapshots, 12 replays concurrentes, fuente inmutable, documentos aislados, intención/versión obsoleta, autorización fuente/destino/revocación, identidad técnica denegada, rollback de evento y retry, HTTP estricto/CSRF, UI reintento y navegación confirmada, navegador clonando OC entregada con documentos. Build/pruebas Angular y regresión en curso; no marcar cerrada antes del CI verde. Después: aceptación integral OC antes de Facturas/pagos. Sin despliegue ni conexiones productivas.
