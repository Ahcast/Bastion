# Lineamientos de módulos — Oficina AI v2

Este índice reúne los lineamientos funcionales y arquitectónicos para preservar las capacidades del sistema legacy durante el nuevo desarrollo de Bastion.

Referencias transversales:

- [Arquitectura Oficina AI v2](../architecture/oficina-ai-v2.md)
- [Módulo AI](../../bluehost/backend/app/Modules/AI/README.md)

## Módulos

| Módulo visible | Propietario técnico | Lineamientos |
|---|---|---|
| Inicio | Core frontend / dashboard | [Inicio](../../bluehost/frontend/src/app/core/pages/dashboard/LINEAMIENTOS.md) |
| Clientes | `App\Modules\Customers` | [Clientes](../../bluehost/backend/app/Modules/Customers/LINEAMIENTOS.md) |
| Proveedores | `App\Modules\Suppliers` | [Proveedores](../../bluehost/backend/app/Modules/Suppliers/LINEAMIENTOS.md) |
| Órdenes de Compra | `App\Modules\PurchaseOrders` | [Órdenes de Compra](../../bluehost/backend/app/Modules/PurchaseOrders/LINEAMIENTOS.md) |
| Cotizaciones | `App\Modules\Quotes` | [Cotizaciones](../../bluehost/backend/app/Modules/Quotes/LINEAMIENTOS.md) |
| Facturas | `App\Modules\Invoices` | [Facturas](../../bluehost/backend/app/Modules/Invoices/LINEAMIENTOS.md) |
| Gastos | `App\Modules\Expenses` | [Gastos](../../bluehost/backend/app/Modules/Expenses/LINEAMIENTOS.md) |
| Reportes | `App\Modules\Reports` | [Reportes](../../bluehost/backend/app/Modules/Reports/LINEAMIENTOS.md) |
| Procesos | `App\Modules\Processes` | [Procesos](../../bluehost/backend/app/Modules/Processes/LINEAMIENTOS.md) |
| Colaboración | `App\Modules\Collaboration` | [Colaboración](../../bluehost/backend/app/Modules/Collaboration/LINEAMIENTOS.md) |

## Distribución documental

Los lineamientos de cada dominio viven junto al módulo backend que posee sus reglas, persistencia y Policies. Las pantallas Angular consumen esos contratos y no mantienen una segunda especificación funcional duplicada.

`Inicio` es la excepción: continúa como página del Core frontend porque no posee tablas ni reglas transaccionales propias y agrega información de Facturas, Órdenes y Procesos.

Los módulos transversales `AI` y `Workflow` permanecen separados y no agregan entradas nuevas obligatorias al menú.

## Integración transversal de IA

La IA no vive dentro de cada módulo como una integración distinta. Todos consumen el mismo `App\Modules\AI` mediante tools autorizadas y Actions públicas de dominio.

```text
Angular / evento / agente
          |
          v
      Laravel
          |
          v
      AI Gateway
          |
    Tool + Policy
          |
          v
   Domain Action
```

Reglas comunes:

1. ningún módulo llama directamente a OpenAI u otro proveedor;
2. ninguna API key llega al frontend;
3. la IA no accede directamente a MySQL, shell, cPanel, SMTP o S3 completo;
4. cada tool reutiliza Policies y Actions del módulo propietario;
5. resultados estructurados se validan antes de persistirse;
6. cada ejecución relevante queda registrada en `ai_runs` y cada tool call sensible en `ai_tool_calls`;
7. acciones externas, financieras o irreversibles requieren aprobación humana por defecto;
8. el sistema continúa funcionando si la IA no está disponible;
9. datos de HonduCompras, web, email y documentos se tratan como `UNTRUSTED_EXTERNAL_CONTENT`.

## Capacidades IA por módulo

| Módulo | Capacidades principales | Límite obligatorio |
|---|---|---|
| Inicio | resumen ejecutivo, prioridades, explicación de alertas | no recalcula KPIs |
| Clientes | matching, duplicados sugeridos, resumen de relación | no fusiona automáticamente |
| Proveedores | investigación, candidatos, historial de precios, RFQ borrador | evidencia/fuente obligatoria |
| Procesos | clasificación, resumen, requisitos, relevancia, items sugeridos | raw/versiones del crawler no se modifican |
| Cotizaciones | items sugeridos, comparación con proceso, margen, email borrador | crear desde proceso no depende de IA; enviar requiere aprobación |
| Órdenes | proveedor/costo sugerido, margen/riesgo, seguimiento | no marca Comprado/Entregado por sí sola |
| Facturas | validación cruzada, inconsistencias, borradores | no altera correlativo, pagos o SAR |
| Gastos | OCR/extracción, clasificación, duplicados | usuario confirma registro |
| Reportes | narrativa, anomalías, explicación | usa métricas canónicas del backend |
| Colaboración | resumen/extracción documental | respeta ACL y defensa contra prompt injection |

## Relación con API/MCP

REST, MCP y las tools internas de IA deben converger en las mismas Actions de aplicación:

```text
REST Controller ----\
                    -> Domain Action -> Policy -> MySQL/S3/Email
MCP Tool -----------/
AI Tool ------------/
```

No se duplican reglas de negocio por canal de entrada.

## Ejecución inicial y experiencia de usuario

La [integración IA](../architecture/ai-integration.md) actualiza el Gateway como coordinador de tareas/resultados en Laravel, con ejecución externa prioritaria en ChatGPT Work/Plus mediante MCP. Los adaptadores API son opcionales futuros. [Resultados](ai-results.md) se presentan dentro de Procesos y se incorporan a Cotizaciones solo por selección del usuario. El RAG inicial recupera antecedentes de S3/MySQL.
