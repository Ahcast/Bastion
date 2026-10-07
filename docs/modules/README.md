# Lineamientos de módulos — Oficina AI v2

Este índice reúne los lineamientos funcionales y arquitectónicos para preservar las capacidades del sistema legacy durante el nuevo desarrollo de Bastion.

La referencia transversal es [Oficina AI v2 — Lineamientos globales e integración modular](../architecture/oficina-ai-v2.md).

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

Los lineamientos de cada dominio viven junto al módulo backend que posee sus reglas, persistencia y Policies. Las pantallas Angular correspondientes consumen esos contratos y no mantienen una segunda especificación funcional duplicada.

`Inicio` es la excepción: continúa siendo una página del Core frontend, porque no posee tablas ni reglas transaccionales propias y agrega información de Facturas, Órdenes y Procesos.

Los módulos transversales `AI` y `Workflow` permanecen separados y no agregan entradas nuevas al menú. Sus reglas se integran mediante los contratos globales y las herramientas autorizadas descritas en la arquitectura v2.
