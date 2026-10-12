# Contrato Reportes

Contrato previo a implementación. Alcance de la unidad 1: tres reportes canónicos e Inicio. Unidad 2: Excel privado de órdenes. Unidad 3: aceptación integral. No escribe tablas comerciales ni recalcula precios, impuestos, inversión o utilidad.

## Persistencia y consultas propietarias

Migración 0033 registra `reportes.view` y `reportes.export`, sin concesiones automáticas. No hay tablas comerciales nuevas. Proyecciones de lectura documentadas sobre `invoices`, `customer_purchase_orders` y `expenses`, sin joins a ítems/referencias que multipliquen filas. Subtotal de OC es la suma de bases guardadas; ISV es suma de ISV guardados; inversión, gastos adicionales y utilidad son sus valores persistidos. Referencias documentales de gastos no ajustan OC ni utilidad. Reportes no realiza FX.

Cada respuesta usa una transacción REPEATABLE READ: autorización vigente antes del primer snapshot, conteo, totales SQL y página comparten snapshot. Se bloquea usuario antes de capturar permisos; todos los scopes/membresías se leen en el mismo snapshot. La petición siguiente reevalúa cualquier revocación confirmada; no se promete cancelar retroactivamente una lectura que ya capturó su snapshot. No se agregan filas en memoria. Fechas DATE inclusivas `invoice_date`, `order_date`, `expense_date`; orden fecha DESC, id DESC. Inicio consume exactamente la misma función de totales y restricciones. Consultas de Inicio de tres fuentes comparten una transacción.

## Permisos y flags

Identidad activa/MFA/sesión Core. `reportes.view` y permiso fuente (`facturas.view`, `ordenes.view`, `gastos.view`) obligatorios; intersección independiente de scopes all/self/team por propietario/equipo de cada registro. Permiso Reportes nunca concede el módulo fuente. Flags deshabilitados eliminan ruta/métricas de la fuente; `BASTION_REPORTS_ENABLED` acepta únicamente true/false. Inicio omite fuentes denegadas; no devuelve conteos ni ceros simulados para ellas. No expone métricas sin `reportes.view`.

## REST y DTO

GET `/api/v1/reports/invoices`, `/orders`, `/expenses`: filtros `date_from`, `date_to` (YYYY-MM-DD reales, orden válido), `currency` (HNL/USD), `status` aplicable, `page` 1..100000, `per_page` 1..100; valores por defecto 1/25. Parámetros desconocidos/repetidos o valores inválidos: 422. Gastos rechaza status. Facturas acepta draft/issued; cancelled no es una opción válida de reporte y siempre se excluye. Órdenes acepta los nueve estados propietarios. Papelera OC/Gastos excluida siempre. Sin intervalo se consulta todo el conjunto autorizado.

Respuesta `{report, filters, data, total, page, per_page, totals}`. IDs y montos cadenas, counts enteros; `totals` array `{currency,count,values}` con montos decimales exactos de SUM SQL sobre TODO el conjunto filtrado, independiente de página. Sin filas => arrays vacíos y total 0. Cada fila es un objeto de campos de cadena definidos por tipo:

- Facturas: id, invoice_number (vacío en borrador), customer, invoice_date, due_date (vacío si no emitida), currency, status, total, paid_total, balance. No presenta borradores como cuentas por cobrar emitidas; status visible.
- Órdenes: id, order_number, customer, destination, order_date, currency, status, subtotal, tax_amount, total, products_investment, additional_expenses, investment_total, profit.
- Gastos: id, invoice_number, supplier, expense_date, category, currency, taxable_amount, exempt_amount, tax_amount, total.

GET `/api/v1/reports/home` usa mismos filtros de período/moneda, rechaza status/paginación explícitos; responde `{filters,reports}` donde reports contiene exclusivamente fuentes habilitadas y autorizadas con sus arrays de totales. Nunca suma monedas ni combina ventas facturadas con OC ni gastos documentales. Inicio mantiene el endpoint técnico Core `/dashboard` sin dependencia Core→negocio.

Errores Core: 401 identidad/sesión; 403 sin permiso; 404 fuente deshabilitada; 422 filtros; 500 sanitizado. No estados/transiciones comerciales, eventos ni jobs en la unidad 1. Sin cache de permisos/datos.

## Excel (unidad 2)

POST `/api/v1/reports/orders/export` cuerpo filtros de período, moneda y estado, sin paginación. Usuario humano, CSRF, `reportes.view`, `reportes.export`, `ordenes.view`, reautenticación reciente; scopes de los tres permisos se intersectan. Stream XLSX autenticado, `Cache-Control: no-store`, sin URL pública ni archivo persistido; acceso expira con sesión/reauth, no se crea enlace reutilizable. Límite explícito de 10000 filas: 422 si supera, nunca truncar. Operación de lectura repetible no crea job ni requiere clave idempotente; cada descarga recalcula el snapshot autorizado y audita generación exitosa antes de enviar bytes. Filtros y fecha/hora UTC de generación incluidas.

Hoja Órdenes con encabezados españoles, filtros, moneda por fila, texto protegido de inyección de fórmulas, totales por moneda y fórmulas auxiliares solo cuando la precisión Excel lo permita. Valores de más de 15 dígitos significativos deben conservarse como texto exacto con advertencia visible en el libro; nunca float financiero ni fórmula que mezcle monedas. Especificación normativa OOXML/fórmulas en [Excel](reports-export.md).

## Alcance evolutivo

Estas proyecciones cubren ventas/facturas/saldos, rentabilidad OC/inversión/margen, gastos/proveedor/categoría y operaciones comerciales. Métricas adicionales de procesos/cotizaciones/RFQ/IA y comparaciones por institución/proveedor se incorporarán por contrato propietario en sus fases; no inferir procesos ganados/perdidos ni competitividad de proveedor desde datos inexistentes. IA futura consume estas métricas autorizadas; este módulo funciona sin IA.

## Aceptación

MySQL 5.7 real: permisos separados, self/team y pérdida de membresía/revocación; filtros inclusivos/invalidación; exclusiones; página no cambia totales; HNL/USD y precisión grande; conciliar con cálculos guardados; Inicio idéntico; flags; HTTP auth/CSRF/export. Angular: filtros/estados, paginación, separación de monedas, respuestas tardías y limpieza ante error/revocación. Navegador: UI real contra Go/MySQL, selección/filtros/Inicio y descarga Excel en segunda unidad. CI/race/builds/paridad documental obligatorios antes de siguiente unidad.
