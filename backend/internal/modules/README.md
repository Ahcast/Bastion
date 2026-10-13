# Módulos

Customers, Suppliers, Processes/receptor crawler, Quotes, PurchaseOrders, Invoices/payments, Expenses, Reports y Collaboration tienen implementación y aceptación de desarrollo certificadas. Evidencia vigente: [checkpoint](../../../docs/ESTADO_DESARROLLO.md). [Índice funcional](../../../docs/modules/README.md) y [contrato modular](../../../docs/contracts/modules.md).

AI U1 implementa tareas/ejecuciones/reservas mediante REST/MCP; [contrato](../../../docs/contracts/ai-tasks.md). Workflow de aprobaciones está integrado; su configuración futura conserva fase propia. Ambos son transversales sin menú adicional. Core no importa módulos concretos; internal/composition registra dependencias y rutas desde cmd/server. Pruebas sintéticas no acreditan producción ni integraciones externas.
