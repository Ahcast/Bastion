# Ejecutar Órdenes de Compra de clientes

Requiere Core, migraciones globales hasta `0019_customer_purchase_order_clones.sql` en MySQL 5.7 aislado y Angular. Seguir [Core](core-runbook.md). No ejecutar seeds, pruebas ni migraciones contra datos reales.

`BASTION_PURCHASE_ORDERS_ENABLED=true` registra el módulo; deshabilitarlo retira todas sus rutas y conserva los datos. Clientes/Proveedores son opcionales: los snapshots manuales funcionan sin sus permisos; un ID requiere el propietario habilitado y acceso autorizado.

[Agregado](../contracts/purchase-orders.md), [estados](../contracts/purchase-orders-states.md) y [OpenAPI](../contracts/purchase-orders-openapi.json). Importes/cantidades/versiones viajan como cadenas. Cabecera e ítems se guardan en una transacción. HNL y USD separados, sin FX implícito.

## Operación

Asignar permisos explícitos `ordenes.view/create/update` para alta/consulta/edición. La edición completa sólo se admite en **Recibida** y con todos los ítems **Pendiente**; después no se reemplazan los detalles ni se reinicia el avance.

Asignar `ordenes.transition` para avance adyacente de la cabecera hasta **Preparando entrega**, `ordenes.items.transition` para avance adyacente de cada ítem y `ordenes.deliver` para entregar la OC. Cada acción también exige lectura de la OC y mantiene scopes `self/team/all`. Los roles existentes no reciben estos permisos automáticamente.

Guardar o descartar cambios antes de transicionar. Completar el avance de cada ítem hasta **Entregado**. La cabecera no se calcula automáticamente desde los ítems; realizar su avance explícito. Para entregar la OC, todos sus ítems deben estar entregados y el operador debe confirmar su identidad con contraseña y TOTP/recuperación vigente.

Timeout: repetir misma intención/payload. Conflicto: consultar la versión actual. La UI conserva la clave ante fallo y la versión/estado confirmados si falla la recarga. Los replays se reautorizan y la entrega vuelve a exigir identidad reciente. Historial conserva actor/fecha UTC, versión, origen/destino y posición 1-based; la UI presenta hora de Honduras y página de 25 entradas.

Facturado/Cerrado son vocabulario reservado; no existen acciones para simular sus efectos. OC→Factura, entrega parcial, reapertura, utilidad y OC a proveedores siguen pendientes. La conversión desde Cotizaciones se explica más abajo.

## Validación

`python scripts/check_repository.py`; `gofmt`, `go vet`, `make build` y `go test -race -count=1 ./...` con MySQL 5.7 dedicado `bastion_test`; Angular `npm ci`, build y tests. Playwright utiliza `bastion_browser_test` y el seed sintético: cálculo/alta/edición, avance de cabecera/ítems, bloqueo de entrega incompleta, confirmación de identidad, entrega y recarga persistida.

S3, SMTP, Percona Bluehost, DNS y despliegue productivos mantienen sus validaciones externas. Esta unidad no envía mensajes ni factura por cambiar un estado.

## Conversión desde Cotizaciones

Ambos módulos habilitados; permisos explícitos `cotizaciones.view/convert` y `ordenes.create/view` con scopes actuales. Abrir una cotización **Aceptada**, indicar número externo OC, fechas, destino, gastos y comentarios; confirmar la conversión. No existe conversión de Borrador/Aprobada internamente ni alta vacía. La fuente queda **Convertida a OC** y enlaza el destino; la OC conserva ID/versión de fuente. Revisar ambos con sus permisos independientes. No se copian PDFs/adjuntos ni notas privadas; documentos de OC se gestionan mediante Core.Files en su contrato propietario.

Si se pierde el ACK, repetir la misma intención y datos. Otra intención tras convertir devuelve conflicto; no otra OC. La migración añade unicidad del origen: datos legacy con orígenes duplicados deben revisarse antes de una migración productiva; no se depuran automáticamente. Playwright incluye conversión USD 201.25, inversión 83.50, recarga y navegación cruzada con usuario sin permisos de catálogos.

## Documentos privados (Unidad 4)

Permisos explícitos: ordenes.view, ordenes.documents.write, core.documents.read/write con scope correspondiente; no se otorgan automáticamente. Abra una OC guardada, consulte Documentos privados y elija Cabecera o un ítem como destino. Guarde/descarte ediciones y confirme identidad para vincular, eliminar o restaurar. Carga PDF/PNG/JPEG/TXT hasta 20 MB; permanece en cuarentena hasta análisis autorizado. Disponible habilita preview y descarga Core.

Filtros por ítem y papelera, paginación de 25. Papelera conserva el vínculo; no se puede eliminar de la OC un ítem con documento incluso eliminado. Reordenar conserva el ID y el adjunto. API de edición debe enviar los IDs retornados para conservar ítems. Timeout: repetir misma intención, no crear otro documento; la UI conserva etapa de carga/clave. Permisos se reevalúan en cada petición. No se acredita S3/escáner productivo con el adapter sintético de CI.

Unidad 4 certificada en `1073c261de0c8f7ab12aed0ff6bb7b6a5f361e7c`; [Core checks 38163339892](https://github.com/Ahcast/Bastion/actions/runs/38163339892) aprobado. Clonación y cierre integral son el siguiente paso.

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
