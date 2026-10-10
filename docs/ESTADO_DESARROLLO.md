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
- Cotizaciones: **cierre monomoneda implementado; validación final en curso**.
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

## Cierre activo — Cotizaciones en monedas separadas

Decisión explícita del usuario 2026-10-10: cotizar en HNL o USD separados; sin conversión automática. FX queda como ampliación futura, no bloquea esta entrega. La propuesta FX conserva carácter no normativo.

Implementación de cierre: lotes con efecto/ledger atómicos y replay reautorizado; UI conserva intención pendiente y bloquea bulk durante edición; replay de encolado comercial; revalidación del aprobador al ejecutar; SMTP aceptado produce estado Enviada; aceptación/rechazo/vencimiento y reapertura con motivo/auditoría. Vencida solo después del último día de validez en Honduras.

Pruebas nuevas: rollback del ledger, concurrencia, pérdida de scopes, autorización de correo revocada, replay, estados finales; navegador cubre correo/worker con adaptador SMTP sin red y recorrido comercial. **Cierre sujeto al CI de estos cambios; no usar el CI del hito bulk anterior como evidencia del nuevo código.**

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

Ejecutar y verificar CI de este cierre. Registrar SHA/resultados en status.md; solo declarar cerrado el alcance monomoneda cuando backend/MySQL5.7/race, Angular y navegador pasen. Mantener las integraciones productivas sin activar. No iniciar otro módulo en esta tarea.
