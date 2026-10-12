# Facturas — unidad 7, aceptación integral de desarrollo

Alcance: comprobar conjuntamente las unidades 1–6 con MySQL 5.7 real, Go `-race`, compilaciones server/worker/admin, Angular y navegador contra API y base reales. Los escenarios deben utilizar datos sintéticos y pasar sin desactivar seguridad ni omitir la regresión del repositorio.

| Invariante | Evidencia automatizada |
| --- | --- |
| Contado, crédito, USD/HNL, descuentos, exoneración, ISV, costo/profit y saldos exactos | `invoice_drafts_test.go`, `invoice_payments_test.go` |
| Borrador histórico, versionado, permisos de registro/equipo, reauth, CSRF e idempotencia | `invoice_drafts_test.go`, `invoice_emission_test.go`, navegador Facturas |
| Fiscal: configuración, correlativo único, vigencia, snapshots inmutables, rollback y concurrencia | `invoice_emission_test.go`, navegador emisión |
| Pagos auditables, anulaciones, sobrepago, vencimiento, saldo y cierre OC explícito | `invoice_payments_test.go`, `purchase_order_invoice_billing_test.go`, navegador OC y pagos |
| Quote→Factura y OC→Factura sin duplicados, versiones/origen, competencia y cierre/reapertura | `quote_invoice_conversion_test.go`, `purchase_order_invoice_billing_test.go`, navegador conversiones |
| Factura PDF, nota de entrega y recibos activo/anulado, acceso/estado y PDF sintético visual | `invoice_documents_test.go`, navegador documentos y muestras renderizadas de unidad 6 |
| Anulación fiscal conservadora conserva número/snapshots y bloquea cobro/edición/documentos | `invoice_cancellation_test.go`, navegador anulación |
| Clonación histórica crea borrador aislado, concurrencia, rollback, replay/revocación | `invoice_cloning_test.go`, navegador clonación |
| Ausencia de dependencia de IA en Facturas y no modificación automática de SAR/serie/pagos | APIs de Facturas invocan sólo servicios de dominio; cualquier acción IA futura requiere contrato y Domain Action autorizada |

El cierre requiere checks de documentación, rama única y Core completos en verde después de los últimos cambios. El desarrollo sintético no acredita configuración fiscal aprobada en producción, revisión legal de casos de anulación excluidos, S3/Bluehost/Lightsail, correo, pagos bancarios ni Siafi real. Estos límites se mantienen en estado operativo y no impiden cerrar el alcance de desarrollo probado.
