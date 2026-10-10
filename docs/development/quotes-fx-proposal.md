# Propuesta de diseño FX para Cotizaciones

> **ESTADO: PROPUESTA NO NORMATIVA. NO HABILITA IMPLEMENTACIÓN.**
>
> Este documento separa hechos verificables de la fuente oficial, recomendaciones técnicas y decisiones comerciales todavía pendientes. Ninguna recomendación de este archivo debe convertirse en código hasta ser promovida explícitamente al contrato normativo de Cotizaciones.

Actualizado: 2026-10-10.

## 1. Objetivo

Permitir conversiones explícitas y reproducibles entre USD y HNL sin mezclar monedas silenciosamente, sin reescribir historia y sin convertir una tasa externa mutable en estado implícito de una cotización.

La cotización actual sigue teniendo **una moneda fuente única** (`HNL` o `USD`). FX sería una capacidad adicional para obtener una representación convertida o crear deliberadamente una nueva revisión/operación en otra moneda, nunca una conversión automática escondida dentro del calculador actual.

## 2. Hechos oficiales verificados

Fuentes primarias del Banco Central de Honduras (BCH):

- Resultados diarios del Tipo de Cambio de Referencia (TCR): https://www.bch.hn/operativos/INTL/Paginas/rd-resultados-diarios-tcr.aspx
- Marco descriptivo de política cambiaria: https://www.bch.hn/politica-institucional/politica-cambiaria/marco-descriptivo
- Tipo de Cambio de Compra y Venta: https://www.bch.hn/estadisticas-y-publicaciones-economicas/tipo-de-cambio-nominal
- Acuerdo BCH No.05/2023: https://www.bch.hn/administrativas/JUR/Marco%20Legal%20OM%202/ACUERDO%20NO.05-2023.pdf

Hechos que sí pueden considerarse fundamento de diseño:

1. El BCH publica una serie diaria del TCR y mantiene serie histórica.
2. Desde abril de 2023 el TCR se obtiene del tipo de cambio promedio ponderado de la Subasta de Divisas.
3. El Acuerdo No.05/2023 define el TCR como el promedio ponderado de la Subasta de Divisas del día y establece su publicación diaria por el BCH.
4. La publicación estadística de compra/venta del BCH indica que el **tipo de compra corresponde al TCR**.
5. La fuente oficial también publica información de compra y venta; por tanto, `TCR`, `compra` y `venta` no deben tratarse como nombres intercambiables dentro del dominio.

Estos hechos **no determinan** cuál tasa conviene comercialmente a Castillo IT en cada operación. En particular, que el TCR sea referencia oficial no prueba que deba usarse para valorar una erogación donde la empresa necesita comprar USD.

## 3. Recomendación de arquitectura

### 3.1 Separar fuente de tasa y política comercial

Crear dos conceptos distintos:

- **Observación FX:** dato publicado/capturado de una fuente y período de vigencia.
- **Política de conversión:** regla comercial que decide qué observación/tipo usar para una operación concreta.

Esto evita codificar `USD→HNL = BCH TCR` como una verdad universal.

### 3.2 Fuente primaria recomendada

**Recomendación:** BCH como proveedor primario para USD/HNL por ser la fuente oficial nacional y disponer de TCR diario/histórico.

No se recomienda que el frontend consulte BCH directamente. La captura debe ocurrir server-side mediante un adapter/proveedor FX y persistir una observación inmutable antes de utilizarla.

No se recomienda depender de scraping de HTML como único mecanismo. El adapter debe poder consumir una representación oficial estructurada cuando esté disponible y conservar evidencia suficiente para auditar de dónde salió la observación.

### 3.3 Modelo propuesto de observación

Nombre conceptual: `fx_rate_observations`.

Campos propuestos, sujetos a contrato final:

- `id`
- `base_currency` = `USD`
- `quote_currency` = `HNL`
- `rate_kind` = `tcr | buy | sell | manual`
- `rate` como decimal exacto, nunca float
- `valid_from`
- `valid_to` nullable
- `source` = `bch | manual`
- `source_reference` (URL/identificador oficial)
- `source_published_at` nullable
- `captured_at`
- `captured_by` nullable para captura automática
- `evidence_hash` del payload normalizado/evidencia cuando sea técnicamente viable
- `supersedes_id` nullable para correcciones explícitas
- `reason` obligatorio para entrada/corrección manual
- timestamps técnicos

No sobrescribir una observación utilizada. Una corrección crea otra fila y conserva relación con la anterior.

### 3.4 Snapshot por uso

Una conversión que llegue a formar parte de una versión de cotización debe congelar como mínimo:

- moneda origen y destino;
- importe origen exacto;
- tasa exacta;
- `rate_kind`;
- observación utilizada;
- vigencia utilizada;
- fuente;
- importe convertido exacto;
- regla/version de redondeo;
- momento y actor de la operación.

Una modificación posterior de BCH o de la política comercial no reescribe una cotización histórica, una aprobación ni un PDF emitido.

### 3.5 Calculador

Mantener `Calculate()` actual estrictamente monomoneda.

La conversión FX debe ser una operación separada y explícita. Esto permite distinguir:

1. cálculo fiscal/comercial dentro de la moneda de la cotización;
2. conversión de representación o creación de nueva revisión en otra moneda.

No convertir línea por línea y después volver a totalizar salvo que el contrato lo exija; hacerlo puede producir diferencias de centavos frente a convertir totales. El nivel exacto de conversión debe ser una decisión normativa.

### 3.6 Exactitud

Recomendación técnica:

- almacenar tasa con `DECIMAL`, nunca `FLOAT`/`DOUBLE`;
- transportar tasa e importes por API como cadenas;
- versionar la regla de cálculo;
- reutilizar la infraestructura decimal existente de Cotizaciones.

**Pendiente de aprobación:** escala exacta de la tasa y momento exacto del redondeo. Aunque el calculador actual usa half-up a dos decimales para importes monetarios, no se debe extender esa regla a FX sin decisión explícita.

## 4. Política de vigencia

La fuente BCH maneja una tasa con vigencia y no debe asumirse simplemente que `fecha de publicación == fecha comercial`.

**Recomendación:** modelar intervalo de vigencia (`valid_from`, `valid_to`) y seleccionar por la fecha/hora efectiva de la operación, no por “última tasa descargada”.

No aplicar silenciosamente:

- la tasa más reciente si falta una tasa para la fecha solicitada;
- la tasa del viernes a un fin de semana;
- la tasa del día anterior;
- una tasa futura ya publicada.

Cualquiera de esas políticas puede ser válida, pero debe quedar aprobada. Mientras no exista, ausencia de una observación válida debe producir un error explícito.

## 5. Política comercial que aún debe decidirse

### 5.1 Tasa para una cotización comercial

Hay al menos tres semánticas diferentes:

- **TCR/referencia:** útil para una referencia oficial reproducible.
- **Compra:** el BCH indica que corresponde al TCR en su reporte nominal.
- **Venta:** puede ser relevante cuando la empresa necesita adquirir USD entregando HNL.

No se debe seleccionar una por intuición. La aplicación debería admitir políticas con nombres explícitos y registrar cuál se utilizó.

**Recomendación preliminar, no aprobada:**

- no usar una única tasa global para todo;
- definir la política según el propósito de la conversión;
- considerar `sell` para estimar una obligación real donde Castillo IT necesita comprar USD;
- considerar `tcr` cuando el documento/contrato exija una referencia oficial neutral;
- conservar siempre la tasa concreta utilizada, aunque la política cambie después.

### 5.2 Diferencial o colchón comercial

No mezclar margen de seguridad cambiario con la tasa oficial.

Si se decide proteger una cotización contra depreciación, modelar el colchón como un concepto comercial separado (`fx_buffer` o equivalente), visible y auditable internamente. No alterar el valor BCH y seguir etiquetándolo como TCR.

### 5.3 Override manual

Recomendación: permitirlo solo como excepción explícita con:

- permiso específico;
- motivo obligatorio;
- fuente/referencia opcional pero trazable;
- auditoría;
- no sobrescribir observaciones oficiales.

Pendiente decidir si el override manual requiere aprobación de segunda persona antes de llegar a una cotización aprobada/emitida.

## 6. Efecto sobre estados y evidencia

Recomendación:

- un cambio de tasa aplicado a una cotización crea una **nueva revisión**;
- una cotización `approved` no cambia in-place: cualquier reconversión vuelve al flujo de revisión;
- un PDF `issued` conserva para siempre su snapshot FX;
- una autorización de correo ligada al PDF emitido no se recalcula por un cambio posterior de tasa;
- clonar conserva el snapshot comercial de origen como referencia histórica, pero la decisión de recalcular FX en la nueva cotización debe ser explícita;
- bulk no debe reconvertir moneda de manera implícita.

## 7. API propuesta, no aprobada

Evitar un endpoint genérico que cambie el valor global de todas las cotizaciones.

Capacidades candidatas:

- consultar observaciones FX autorizadas por fecha/tipo;
- importar/capturar observación oficial;
- registrar corrección/override con permiso específico;
- solicitar una vista previa de conversión sin guardar;
- aplicar una conversión a una cotización con `Idempotency-Key` + versión confirmada, generando nueva versión/snapshot.

Todos los IDs/versiones seguirían siendo cadenas JSON y todas las escrituras sensibles seguirían las reglas de idempotencia/auditoría del Core.

## 8. UI propuesta

En español y sin ocultar la semántica:

- mostrar moneda original como fuente de verdad;
- mostrar `Tasa utilizada`, `Tipo de tasa`, `Fuente` y `Vigencia`;
- diferenciar visualmente “vista previa convertida” de “cotización guardada en moneda X”;
- no mostrar “tasa BCH” si existe un buffer/override mezclado;
- indicar cuando la tasa quedó congelada por una versión aprobada/emitida;
- no ofrecer “usar tasa de hoy” sobre un documento histórico sin generar revisión nueva.

## 9. PDF/exportaciones

Pendiente decidir si una cotización debe mostrar:

- solo moneda contractual;
- moneda contractual + equivalente informativo;
- tasa/fecha/fuente explícitas;
- equivalente solo en PDF pero no en el agregado comercial.

Recomendación: si se presenta un equivalente, el PDF debe mostrar tasa exacta, tipo y vigencia del snapshot utilizado. Un PDF emitido nunca consulta la tasa live durante render o descarga.

## 10. Ingesta y resiliencia

Recomendación de futura implementación:

1. adapter `FXRateProvider` server-side;
2. proveedor BCH implementado detrás de esa interfaz;
3. captura idempotente por fuente/tipo/vigencia;
4. job programado con retry/backoff, sin bloquear operación monomoneda;
5. evidencia persistida antes de habilitar uso de la tasa;
6. observabilidad de última captura, atraso y errores;
7. operación manual HNL/HNL o USD/USD continúa aunque BCH esté caído;
8. conversión que requiera una tasa inexistente falla cerrado con mensaje explícito.

No introducir una dependencia live de BCH dentro de `POST /quotes/calculate` ni del render PDF.

## 11. Decisiones obligatorias antes de escribir código

Deben pasar al contrato normativo `docs/contracts/quotes.md` antes de una migración FX:

1. ¿Qué tasa usa cada propósito: `tcr`, `buy`, `sell` o política configurable?
2. ¿Cuál es la regla exacta para días sin nueva publicación/vigencias que abarcan varios días?
3. ¿Cuál es la escala almacenada de la tasa?
4. ¿Dónde se redondea la conversión: línea, subtotal, impuestos, total u otra capa?
5. ¿Se convierte una vista informativa o se crea una nueva versión en otra moneda?
6. ¿Se permite override manual y quién lo autoriza?
7. ¿Un buffer cambiario existe y cómo se representa sin falsificar la tasa oficial?
8. ¿Qué información FX aparece en UI, PDF y CSV?
9. ¿Cómo se tratan clones y conversiones futuras Cotización→OC/Factura?
10. ¿La ingesta oficial será automática, manual controlada o ambas?

## 12. Recomendación de orden de implementación una vez aprobado

1. ADR/contrato normativo FX con las diez decisiones anteriores.
2. Migración para observaciones y snapshots.
3. Dominio decimal/provider + tests unitarios.
4. Adapter BCH y mecanismo de captura con fixtures oficiales versionados, sin depender de internet en CI.
5. Servicio de preview/aplicación versionada e idempotente.
6. REST + OpenAPI.
7. Angular en español.
8. PDF/exportación según política aprobada.
9. MySQL/race/concurrencia/rollback/replay.
10. Playwright con datos sintéticos deterministas.
11. Validación externa de captura BCH y operación en infraestructura autorizada.

Hasta entonces, Cotizaciones continúa operando correctamente en una sola moneda por agregado y **FX permanece deshabilitado por diseño**, no como stub oculto.
