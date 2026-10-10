# Estado operativo de desarrollo — Bastion

Este archivo es el **checkpoint corto para reanudar desarrollo**. La documentación de arquitectura, contratos, decisiones y runbooks sigue siendo normativa; este archivo indica dónde quedó el trabajo real y qué debe hacerse después.

Actualizado: 2026-10-10.

## Regla para reanudar

1. Trabajar únicamente sobre `main`; GitHub es la fuente de verdad.
2. Leer este archivo, `development/decisions.md`, el contrato y el runbook del módulo activo.
3. Verificar el HEAD y el último CI antes de modificar código.
4. Si el CI está rojo, corregir primero la regresión y repetir pruebas; no avanzar de módulo.
5. Desarrollar una unidad verificable, agregar pruebas, esperar CI verde y actualizar este checkpoint.
6. No declarar validadas integraciones productivas solo porque pasen mocks/adapters/pruebas sintéticas.

## Estado global

- Core: implementado y validado en CI.
- Clientes: unidad cerrada y validada.
- Proveedores: unidad cerrada y validada.
- Procesos/receptor crawler: unidad base cerrada y validada.
- Cotizaciones: **módulo activo; bulk cerrado, aceptación integral aún no cerrada**.
- OC de clientes y módulos posteriores: no iniciar todavía.
- Producción: no desplegada ni migrada.

## Cotizaciones — completado

- Borradores, cálculo exacto, versiones, idempotencia y auditoría.
- Conversión desde Procesos preservando snapshots autorizados.
- Adjuntos y PDF de borrador.
- Estados preliminares Costeo/Revisión.
- Aprobación humana de dos personas y revocación.
- Emisión segura e inmutable de PDF aprobado.
- Transporte Core de correo y programación durable.
- Autorización comercial específica de destinatarios, asunto, mensaje, PDF emitido, adjuntos y horario.
- Worker de correo con revalidación de permisos y binding antes del efecto externo.
- Ledger durable; resultado SMTP incierto queda `uncertain` y no se reenvía a ciegas.
- Integridad SHA/size de PDF y adjuntos antes del SMTP.
- Revocación de autorización sigue disponible aunque la evidencia haya cambiado.
- UI de correo comercial y pruebas de navegador.
- Bulk durable de cotizaciones:
  - migración `0013_quote_batches.sql` con cabecera e ítems persistentes;
  - `POST /api/v1/quotes/bulk/status`;
  - `POST /api/v1/quotes/bulk/clone`;
  - `POST /api/v1/quotes/bulk/export`;
  - `batch_id` estable y resultado por cotización;
  - reintentos con identidad de lote e idempotencia derivada por ítem;
  - errores cliente por ítem sin ocultar fallos parciales;
  - errores internos dejan trabajo pendiente para retry seguro;
  - exportación congelada, neutralizada contra fórmulas y sin costos, márgenes ni notas privadas;
  - selección múltiple Angular y acciones de estado/clonado/exportación;
  - la UI bulk no aparece en la papelera.

## Último hito certificado — Bulk de Cotizaciones

HEAD certificado: `e3f70fee85a8fd1be6dcdd56fd9fc0ada20f6328`.

Core checks: GitHub Actions run `38088666258`, conclusión `success`:
- Go format/vet: aprobado.
- `go test -race -count=1 ./...` contra MySQL 5.7: aprobado.
- Builds Go: aprobados.
- Angular build/tests: aprobados.
- Navegador contra API + Angular + MySQL reales de CI: aprobado.

También aprobados para el mismo HEAD:
- Documentation checks: run `38088666202`.
- Single branch policy: run `38088666297`.

Cobertura específica bulk:
- integración MySQL verifica lote trazable/idempotente, replay sin duplicados, fallo parcial y rechazo de selección duplicada;
- exportación congela el snapshot público y neutraliza fórmulas de hoja de cálculo;
- REST verifica DTO estricto e `Idempotency-Key`;
- Angular verifica retry con la misma clave y exposición de fallos parciales;
- Playwright real crea dos cotizaciones, selecciona ambas desde la UI bulk, descarga CSV y comprueba que la nota privada no se filtra.

Regresión corregida antes de certificar:
- el primer montaje de bulk tenía un bloque Angular mal cerrado;
- `QuoteBulk` estaba importado pero no montado en `quotes.html`;
- ambos defectos quedaron corregidos y cubiertos por build/tests/navegador.

## Unidad activa siguiente — Contrato FX de Cotizaciones

El sistema actual soporta moneda explícita `HNL` o `USD`, pero **no realiza conversión implícita**. El alcance menciona FX fechado, pero el repositorio todavía no define de forma normativa:

- fuente de la tasa;
- tasa de compra, venta u otra;
- fecha/hora efectiva y zona horaria;
- política de snapshot e historial;
- precisión y redondeo de tasa e importes convertidos;
- autoridad/permisos para crear, corregir o sustituir una tasa;
- tratamiento de cotizaciones ya aprobadas/emitidas cuando cambia la tasa;
- presentación exacta de la tasa en UI/PDF/exportaciones.

**No implementar una conversión monetaria inventando estas reglas.** Primero debe quedar cerrado el contrato comercial FX y sus invariantes; después se implementan migración/servicio/API/UI y pruebas exactas.

No existe evidencia en la documentación actual que autorice asumir automáticamente una tasa solo porque exista una fuente externa disponible.

## Integraciones cruzadas posteriores

- Cotización → OC y Cotización → Factura deben ser server-side, transaccionales/idempotentes y conservar referencia de origen.
- Quotes no debe escribir directamente tablas privadas de OC o Facturas.
- La integración se completa mediante los servicios propietarios cuando existan esos módulos.
- Este pendiente no autoriza a iniciar OC antes de resolver la aceptación restante de Cotizaciones.

## Validaciones externas aún pendientes

No están certificadas todavía por las pruebas actuales:
- S3/IAM productivo y scanner externo;
- SMTP/IMAP real de Bluehost y límites reales de tamaño/destinatarios;
- Percona/MySQL real de Bluehost, TLS/túnel y cuentas de mínimo privilegio;
- DNS/HTTPS/CORS/CSP/WAF productivos;
- backup/restore productivo;
- despliegue real en Lightsail/Bluehost;
- migración de datos reales;
- OAuth delegado/ChatGPT Work;
- carga/memoria conjunta real del crawler, API y workers.

Estas validaciones externas son gates de producción; no deben falsearse con fixtures de CI.

## Próximo paso exacto

1. Cerrar el contrato FX de Cotizaciones sin asumir reglas no aprobadas.
2. Implementar FX fechado con números exactos, snapshots, autorización e historial según ese contrato.
3. Agregar pruebas unitarias, MySQL/race, REST, Angular y navegador para FX.
4. Actualizar contrato/OpenAPI/runbook/checkpoint y ejecutar CI completo.
5. Solo entonces reevaluar cierre de Cotizaciones y el inicio de Órdenes de Compra de clientes.
