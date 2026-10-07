# Módulos backend aprobados

Estructura exacta de la sección 8.2 del [Documento maestro](../../../../docs/Proyecto_Bastion_Documento_Maestro.md).

| Módulo | Alcance |
| --- | --- |
| Core | Núcleo transversal y configuración |
| Customers | Clientes |
| Suppliers | Proveedores |
| Processes | Procesos |
| Quotes | Cotizaciones |
| PurchaseOrders | Órdenes de compra de clientes |
| Invoices | Facturas |
| Expenses | Gastos |
| Reports | Reportes |
| Collaboration | Colaboración |
| Workflow | Workflows y autorizaciones transversales |
| AI | Integración y estado de agentes AI |

Inicio y Configuración son entradas del menú; no se añaden módulos backend distintos de los 12 definidos en 8.2.
Cada módulo puede contener Models, Services, Actions, DTOs, Policies, Events, Listeners, Jobs, Controllers, Requests, Resources y Routes. Estas carpetas internas se definirán al implementar.
