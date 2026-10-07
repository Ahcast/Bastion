# Módulos backend

Namespace objetivo: `App\Modules`. [Core](../Core/README.md) vive separado en `app/Core`.

| Módulo | Alcance |
| --- | --- |
| [Customers](Customers/README.md) | Clientes |
| [Suppliers](Suppliers/README.md) | Proveedores |
| [Processes](Processes/README.md) | Procesos |
| [Quotes](Quotes/README.md) | Cotizaciones |
| [PurchaseOrders](PurchaseOrders/README.md) | Órdenes de compra de clientes |
| [Invoices](Invoices/README.md) | Facturas |
| [Expenses](Expenses/README.md) | Gastos |
| [Reports](Reports/README.md) | Reportes |
| [Collaboration](Collaboration/README.md) | Colaboración |
| [Workflow](Workflow/README.md) | Flujos y aprobaciones |
| [AI](AI/README.md) | Tareas y proveedores de IA |

Cada módulo conserva sus reglas, tablas y Policies. Consume Core mediante contratos y aporta rutas REST, herramientas MCP y eventos según necesidad. La estructura interna se crea al implementar: Actions, Models, Policies, Http, Mcp, Events, Jobs y Providers, sin carpetas vacías obligatorias.

Workflow y AI son transversales y no agregan entradas al menú. Inicio y Configuración son pantallas del Core frontend.

Ver [contrato modular](../../../../docs/contracts/modules.md).
