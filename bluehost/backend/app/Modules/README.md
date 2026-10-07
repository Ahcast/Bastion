# Módulos backend

Namespace objetivo: `App\Modules`. [Core](../Core/README.md) vive separado en `app/Core`.

| Módulo | Alcance | Lineamientos funcionales / arquitectura v2 |
| --- | --- | --- |
| [Customers](Customers/README.md) | Clientes | [LINEAMIENTOS](Customers/LINEAMIENTOS.md) |
| [Suppliers](Suppliers/README.md) | Proveedores | [LINEAMIENTOS](Suppliers/LINEAMIENTOS.md) |
| [Processes](Processes/README.md) | Procesos | [LINEAMIENTOS](Processes/LINEAMIENTOS.md) |
| [Quotes](Quotes/README.md) | Cotizaciones | [LINEAMIENTOS](Quotes/LINEAMIENTOS.md) |
| [PurchaseOrders](PurchaseOrders/README.md) | Órdenes de compra de clientes | [LINEAMIENTOS](PurchaseOrders/LINEAMIENTOS.md) |
| [Invoices](Invoices/README.md) | Facturas | [LINEAMIENTOS](Invoices/LINEAMIENTOS.md) |
| [Expenses](Expenses/README.md) | Gastos | [LINEAMIENTOS](Expenses/LINEAMIENTOS.md) |
| [Reports](Reports/README.md) | Reportes | [LINEAMIENTOS](Reports/LINEAMIENTOS.md) |
| [Collaboration](Collaboration/README.md) | Colaboración | [LINEAMIENTOS](Collaboration/LINEAMIENTOS.md) |
| [Workflow](Workflow/README.md) | Flujos y aprobaciones | Transversal |
| [AI](AI/README.md) | AI Gateway, tools, providers y ejecuciones IA | Transversal |

Cada módulo conserva sus reglas, tablas y Policies. Consume Core mediante contratos y aporta rutas REST, herramientas MCP y eventos según necesidad. La estructura interna se crea al implementar: Actions, Models, Policies, Http, Mcp, Events, Jobs y Providers, sin carpetas vacías obligatorias.

Workflow y AI son transversales y no agregan entradas al menú. Inicio y Configuración son pantallas del Core frontend; los lineamientos de Inicio están en [`frontend/src/app/core/pages/dashboard/LINEAMIENTOS.md`](../../../frontend/src/app/core/pages/dashboard/LINEAMIENTOS.md).

## Contrato común con IA

Los módulos de negocio **no llaman directamente a OpenAI ni a otro proveedor**. Toda capacidad IA pasa por `App\Modules\AI`.

```text
REST / MCP / UI / Event
        |
        v
Domain Action
        |
        v
AI Gateway
        |
  Tool + Policy
        |
        v
Domain Action / Provider
```

Reglas:

- la IA no accede directamente a MySQL;
- cada tool reutiliza Policies y Actions del módulo propietario;
- las API keys nunca llegan a Angular;
- outputs estructurados se validan antes de persistir;
- acciones sensibles requieren Workflow/aprobación por defecto;
- cada ejecución relevante se registra en `ai_runs` y cada tool call sensible en `ai_tool_calls`;
- los módulos funcionan aunque la IA esté fuera de servicio;
- HonduCompras, web, email y documentos son `UNTRUSTED_EXTERNAL_CONTENT`.

La especificación completa está en [AI/README.md](AI/README.md).

Índice funcional completo: [docs/modules/README.md](../../../../docs/modules/README.md). Arquitectura objetivo: [Oficina AI v2](../../../../docs/architecture/oficina-ai-v2.md). Contrato base: [contrato modular](../../../../docs/contracts/modules.md).
