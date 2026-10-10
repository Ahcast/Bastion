# Estado operativo de desarrollo — Bastion

Este archivo es el **checkpoint corto para reanudar desarrollo**. La documentación de arquitectura, contratos, decisiones y runbooks sigue siendo normativa; este archivo solo indica dónde quedó el trabajo real y qué debe hacerse después.

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
- Cotizaciones: **módulo activo; no cerrado todavía**.
- OC de clientes y módulos posteriores: no iniciar hasta cerrar aceptación de Cotizaciones.
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
- UI de correo comercial y pruebas de navegador existentes.

### Último hito certificado

HEAD certificado: `81fe6f06730be9b03a6eb1dc260f66fa33c9b6cd`.

Core checks: GitHub Actions run `38084634079`, conclusión `success`:
- Go format/vet: aprobado.
- `go test -race -count=1 ./...` contra MySQL 5.7: aprobado.
- Builds Go: aprobados.
- Angular build/tests: aprobados.
- Navegador contra API + Angular + MySQL reales de CI: aprobado.

Regresiones corregidas inmediatamente antes de ese checkpoint:
- `document_versions.storage_key` incorrecto en worker de correo; el esquema real usa `object_key`.
- Ruta REST de revocación conectada a la variante drift-safe.
- CI ahora conserva el log de `go test -race` como artefacto cuando falla.

## Unidad activa siguiente — Bulk de Cotizaciones

Contrato funcional pendiente de implementar:
- selección múltiple en UI;
- `POST /api/v1/quotes/bulk/status`;
- `POST /api/v1/quotes/bulk/clone`;
- `POST /api/v1/quotes/bulk/export`;
- cada lote debe devolver `batch_id` y resultado por cotización;
- el lote debe ser trazable y reintentable; no implementar bulk como llamadas independientes sin identidad de lote;
- reutilizar las Actions/reglas existentes de estado y clonación para no duplicar lógica comercial;
- conservar permisos live, scopes `all/self/team`, versionado e idempotencia;
- importes continúan como cadenas exactas; ningún cálculo monetario en JavaScript/float.

### Criterio técnico propuesto para bulk

- Persistir cabecera de lote y sus ítems en MySQL.
- Crear el lote de forma idempotente.
- Ejecutar cada ítem usando las operaciones existentes con claves derivadas deterministas.
- Registrar éxito/fallo por ítem.
- Un retry no vuelve a ejecutar ítems ya resueltos; continúa los pendientes.
- Errores de autorización/conflicto/validación se registran por ítem sin ocultarlos.
- Un error interno/transitorio no debe convertir un resultado desconocido en éxito ni provocar efectos duplicados.
- Exportación no debe revelar costo, margen ni notas privadas salvo que un contrato futuro lo autorice explícitamente.

## Después de bulk — FX

El contrato actual solo fija que HNL y USD son explícitos y **no existe conversión implícita**. `FX fechado` está marcado como alcance pendiente, pero aún no define fuente de tasa, tipo de tasa, fecha efectiva, snapshot, política de redondeo ni autoridad para modificarla.

No inventar esas reglas. Antes de implementar conversión FX hay que completar o localizar el contrato comercial correspondiente. Este vacío no impide cerrar primero bulk.

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

## Próximo paso exacto

Implementar **backend durable de bulk de Cotizaciones**, agregar integración MySQL/race tests y REST; no tocar OC todavía. Solo después de CI verde agregar selección múltiple/acciones bulk al frontend y pruebas de navegador, actualizar contratos/runbook/checkpoint y continuar con el pendiente FX.
