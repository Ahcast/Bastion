# Aceptación integral de Órdenes de Compra

Unidad 6, después de la clonación certificada. No habilita Facturado/Cerrado ni conversiones antes del propietario Facturas. No introduce estados alternativos legacy.

## Utilidad canónica

Extiende la regla ya aprobada de Quotes (margen antes de impuestos): `profit = base_15 + base_18 + exempt + exonerated - investment_total`, equivalente a ventas netas tras ambos descuentos, sin ISV, menos costo de productos y gastos adicionales. Puede ser negativa; decimal exacto a 2 lugares. No representa utilidad contable fiscal ni incorpora Gastos del módulo futuro por duplicado. Función pública compartida en internal/finance para OC y futuros Invoices/Reports/Home; Quotes mantiene margen sin gastos adicionales. Calculator version 1 fiscal no cambia, se agrega profit_version 1 para esta proyección. No mezcla HNL/USD. Persistir profit DECIMAL(18,2), backfill SQL decimal de columnas existentes. DTO Calculation añade profit y profit_version; ninguna respuesta pública Quote lo incorpora. Crear/editar/conversión/clonación usan la misma fórmula; transiciones/adjuntos no alteran cálculo.

## Eliminación reversible

DELETE `/api/v1/purchase-orders/{id}` y POST `/{id}/restore`, DTO estricto `{version:string,reason:string}` y clave idempotente. Humano, permiso ordenes.delete + ordenes.view con scopes propios y reautenticación reciente, también en replay. Razón no vacía 1..1000 caracteres. Permiso registrado sin grants.

Solo puede eliminarse una OC Recibida con todos los ítems Pendiente, sin vínculo Quote de conversión, sin documentos (incluida papelera), sin historial operativo y que no sea origen de clones. No borra físicamente filas ni números; clones independientes sin estos impedimentos pueden eliminarse. Si no cumple: 409. Restaurar requiere versión actual y que esté eliminada; conserva exactamente snapshots, ítems, cálculo, relación origen, dueño/equipo. Incrementa versión una vez y conserva deleted_at DATETIME(6) nullable. Respuesta `{id,version,deleted_at}` con IDs/versiones strings y timestamp UTC servidor nullable; no inferir fechas desde reloj del navegador. Auditoría con razón y evento purchase_order.deleted/restored en TX con idempotencia; fallo revierte íntegramente.

Listar: filtro deleted=false predeterminado o true explícito, mismas autorizaciones/count/paginación. Ficha muestra deleted_at nullable UTC. Registro eliminado solo es consultable por actor con view+delete; no permite guardar, transicionar, clonar, adjuntar ni descargar documentos mientras eliminado. Runtime no enumera registros fuera de scope y reautoriza antes de replay. Restauración conserva la identidad y el número externo originales. Este número no tiene unicidad global; otros registros pueden compartirlo conforme al contrato de OC. UI papelera, confirmación de identidad y motivo explícito, sin borrado automático.

Migración 0020 agrega deleted_at, profit DECIMAL(18,2), profit_version SMALLINT UNSIGNED con valor 1, índice de papelera y permiso ordenes.delete. Orden de cambios se valida con MySQL 5.7.

Aceptación: impuestos mixtos, descuentos, gastos y utilidad negativa/monedas; backfill y round-trip; eliminación/identidad/documentos/vínculos/historial protegidos; restauración intacta; scopes/revocación/reauth y replay; rollback, DTO/CSRF, UI/navegador/regresión. Matriz integral enlaza unidades 1..6 y registra OC→Factura/Reportes/Inicio/IA en sus dependencias sin declarar esas integraciones disponibles. Cierre OC significa alcance propietario operativo validado, no despliegue ni fiscalidad real.
