# Matriz de aceptación OC

| Capacidad | Evidencia propietaria | Dependencia |
| --- | --- | --- |
| Cabecera/ítems íntegros, cálculo HNL/USD, descuentos e ISV | purchase_orders_test, calculator_test, HTTP/list y escenario purchase-orders | Unidad 1 certificada |
| Avance explícito, historial, entrega humana con reauth | purchase_order_states_test y escenario purchase-orders | Unidad 2 certificada |
| Cotización aceptada → OC, snapshots/origen, atomicidad | quote_purchase_order_conversion_test y zzzzquote-conversion | Unidad 3 certificada |
| Archivos privados, cuarentena, scopes, IDs estables | purchase_order_documents_test, OrderDocuments.spec y escenario purchase-orders | Unidad 4 certificada |
| Clonación independiente, historial de origen, replay autorizado | cloning_test, purchase_order_cloning_test, PurchaseOrders.spec y escenario purchase-orders | Unidad 5 certificada en 6667fe62f952fd1702c627ae88138bab9924a522 |
| Eliminación reversible y utilidad canónica | Contrato de aceptación integral, migración 0020 y pruebas propietarias | Unidad 6 implementada, en validación |
| OC → Factura, búsqueda por factura y número asociado | Servicio Invoices + contrato de conversión | Fase siguiente; no simulado |
| Facturado → Cerrado con pagos | Contrato Invoices/Payments | Pendiente; transición manual denegada |
| Reportes e Inicio concilian utilidad | Función canónica y lectura autorizada de propietarios | Fase Reportes; sin agregados ficticios |
| Sugerencias IA de costos/proveedores/evidencia | AI Domain Actions, permisos y aprobación | Fases IA; operación manual vigente |
| Datos legacy, estados e importes | Mapa aprobado y migración específica con muestras autorizadas | No ejecución sobre datos reales |
| Producción, S3/escáner, Bluehost TLS/SMTP | Validación externa y autorización de despliegue | No acreditado por CI |
