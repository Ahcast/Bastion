# Contrato FX de Cotizaciones — registro de observaciones

## Alcance normativo de esta unidad

Esta unidad **solo** registra y consulta observaciones históricas USD/HNL. No selecciona una tasa para una cotización, no convierte importes, no modifica el calculador, no cambia moneda de una cotización y no autoriza TCR/compra/venta como política comercial por defecto.

La política de conversión sigue bloqueada hasta aprobar las decisiones comerciales descritas en `../development/quotes-fx-proposal.md`.

## Persistencia

Migración `0014_quote_fx_observations.sql` crea `quote_fx_observations` como ledger append-only. Cada fila contiene:

- par fijo de esta unidad: `USD` → `HNL`;
- `rate_kind`: `tcr | buy | sell | manual`;
- `rate`: decimal exacto positivo, hasta 12 enteros y 12 decimales;
- `valid_from` obligatorio y `valid_to` opcional, almacenados en UTC con microsegundos;
- `source`: `bch | manual`;
- `source_reference` obligatorio;
- `source_published_at` opcional;
- `evidence_hash` opcional, SHA-256 hexadecimal en minúsculas;
- `supersedes_id` opcional;
- `reason`;
- actor y fecha de captura.

No existe UPDATE/DELETE del ledger. Una corrección crea una nueva observación y referencia a la anterior mediante `supersedes_id`. Una observación solo puede tener un reemplazo directo; una corrección posterior puede formar una cadena corrigiendo la fila más reciente. La fila original conserva su tasa exacta y se expone como `superseded=true` cuando tiene sucesora.

Una observación manual, una observación con `rate_kind=manual` o cualquier corrección exige motivo no vacío. Esta unidad no verifica que una observación BCH sea auténtica mediante red; `source_reference` y `evidence_hash` son evidencia capturada, no una afirmación de conectividad productiva.

## Autorización

Permisos nuevos, sin grants automáticos:

- `cotizaciones.fx.view`
- `cotizaciones.fx.manage`

El ledger es información global y no pertenece a un usuario o equipo. Por ello ambos permisos requieren scope `all`; grants `self` o `team` no habilitan acceso global. Registrar/corregir observaciones exige actor humano y reautenticación reciente. Lectura no exige reautenticación adicional.

## REST

Contrato machine-readable: `quotes-fx-openapi.json`.

El namespace FX se separa deliberadamente de `/api/v1/quotes/{id}/...` para que una palabra reservada de FX nunca pueda interpretarse como ID de cotización por el router HTTP.

- `GET /api/v1/quote-fx/observations`
- `GET /api/v1/quote-fx/observations/{id}`
- `POST /api/v1/quote-fx/observations`

Listado acepta `rate_kind`, `source`, `page` y `per_page` (máximo 100). Escritura exige `Idempotency-Key`; DTO completo y estricto. `base_currency`, `quote_currency`, actor, timestamps técnicos y estado `superseded` son server-side y no se aceptan en el POST.

El replay de la misma intención devuelve la misma observación. Misma clave con payload distinto es conflicto. En una corrección el chequeo de `supersedes_id` ocurre dentro de la intención idempotente para que un replay confirmado no falle porque la corrección ya existe.

## Auditoría y eventos

Registrar una observación confirma en la misma transacción:

- fila FX;
- respuesta idempotente;
- auditoría `quotes.fx_observation_recorded`;
- evento de dominio `quote.fx_observation_recorded`.

Si cualquiera falla, toda la intención revierte y la misma clave puede reintentarse. No se encola trabajo de conversión, no se consulta BCH y no se modifica ninguna cotización.

## Exactitud

`rate` se recibe y devuelve como string JSON. Backend usa `shopspring/decimal`; no hay `float64`. Esta unidad **no redondea** una tasa aceptada: persiste exactamente el decimal normalizado, sujeto al máximo de 12 decimales. Esta precisión de almacenamiento no define todavía el redondeo de una futura conversión monetaria.

## Errores

- `401`: sesión requerida/vencida.
- `403`: permiso global insuficiente, actor no humano o reautenticación requerida en escritura.
- `404`: observación/superseded target no visible/existente.
- `409`: intención distinta con la misma clave o segundo reemplazo directo.
- `422`: DTO, tasa, hash, fecha, intervalo, enum o motivo inválidos.

## Aceptación de la unidad

Debe verificarse en MySQL 5.7 y race detector:

1. migración y segunda ejecución del runner;
2. `self/team` no acceden al ledger global;
3. tasa exacta sin redondeo;
4. normalización UTC;
5. replay idempotente y conflicto por payload distinto;
6. corrección append-only, replay de corrección y segundo reemplazo rechazado;
7. manual/corrección sin motivo rechazados;
8. ventana temporal y escala inválidas rechazadas;
9. fallo de auditoría revierte observación e intención;
10. REST DTO estricto y OpenAPI sincronizado;
11. el namespace REST no colisiona con rutas `/quotes/{id}/...`;
12. Angular y navegador dejan explícito que registrar una observación no convierte cotizaciones.

Esta aceptación **no cierra FX completo**. La siguiente subunidad no puede convertir cotizaciones hasta promover una política explícita de selección/vigencia/redondeo desde la propuesta FX al contrato correspondiente.
