# Gastos — unidad 3, aceptación integral de desarrollo

Cerrar el módulo sólo con documentación, rama única y Core checks en verde sobre la última revisión. Ejecutar Go fmt/vet/race en MySQL 5.7, builds server/worker/admin, Angular build/pruebas y navegador real contra API/BD. Datos sintéticos y seguridad activa; no simular rutas productivas ni omitir escenarios.

| Invariante | Evidencia |
| --- | --- |
| Fórmula legacy 15%, redondeo half-up, HNL/USD y límites exactos | `expenses/model_test.go`, `expenses_test.go` |
| CRUD, proveedor libre/activo/snapshot histórico, categorías/filtros/totales por moneda | `expenses_test.go`, navegador registro de Gastos |
| Scopes all/self/team, reauth, CSRF, versiones, idempotencia/12 replays, competencia/rollback, duplicados sólo visibles | `expenses_test.go`, `expense_links_acceptance_test.go`, `expense_final_acceptance_test.go` |
| Soft-delete/restauración de gasto y comprobante sin borrar bytes ni alterar importes | `expense_links_test.go`, `expense_final_acceptance_test.go`, navegador vínculos/comprobantes |
| Fuentes Process/Quote/OC/Invoice por propietario, snapshots y estados terminales | `expense_reference_ports_test.go`, `expense_links_test.go` |
| Permiso view adicional incluso replay, scopes/paginación de documentos y carrera con dos actores | `expense_final_acceptance_test.go` |
| Subida privada, cuarentena, finalize/retry y vínculo; ningún scan automático | pruebas Angular `expense-links.spec.ts`, navegador vínculos; Core tests de verificación/scan/download |
| Coordinación del padre, ACK incierto por etapa, misma intención y respuestas tardías descartadas | `expense-links.spec.ts`, `expenses.spec.ts` |
| IDs numéricos alternativos no producen bindings inaccesibles al usar ID canónico | integración de bindings y metadata de `expense_final_acceptance_test.go` |
| Operación manual independiente de IA/OCR y consulta documental opcional por permisos | composición sin dependencias de IA; Angular con core.documents.read ausente |

Revisión final: canonizar owner del binding usando el ID leído del agregado; metadata recupera por el mismo ID. No cambiar Core.ID ni contratos de otros módulos. Migración aditiva 0032 añade índice `(entity_type,entity_id,deleted_at,id)` en documents para la paginación por propietario; no modificar migraciones anteriores ni ejecutar migraciones productivas.

Las referencias siguen siendo documentales: no asignan nuevamente costos ya registrados, no convierten moneda ni ajustan pagos o rentabilidad de la fuente. Reportes deberá explicitar conciliación y bases de cálculo. OCR/IA proponen en su fase; ningún registro automático por extraer datos. Producción, S3 real, conexiones Bluehost/Lightsail y tratamiento fiscal real siguen fuera de la certificación sintética.
