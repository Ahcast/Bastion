# Estado del proyecto

Fecha de referencia: 9 de octubre de 2026.

| Área | Estado real |
| --- | --- |
| Documentación/plan | Consolidación para desarrollo por agente |
| Crawler Python | Implementación y pruebas offline existentes; no prueba en vivo en esta revisión |
| Backend/Core/API/MCP Go | Pendiente de implementación |
| Frontend Angular | Pendiente de implementación |
| MySQL Bluehost | Motor reportado; acceso remoto cifrado no validado |
| S3 | Bucket crawler reportado; integración/despliegue no verificados |
| Work/Grokbot/SMTP real | Conexión y capacidad no validadas; sin envíos activados |
| Política Git | Solo main; workflow elimina ramas ancestro de main y falla ante ramas con cambios no incorporados. No equivale a ruleset preventivo |

## Próxima unidad

Fase 0 del plan, seguida de Core. No declarar módulos implementados por existir sus especificaciones.

## Registro que mantiene el agente

Por unidad: fase/alcance, archivos, migraciones/contratos cambiados, comandos/pruebas y resultados, bloqueos/evidencia, próximo paso. Estados: pendiente / en curso / implementado localmente / integrado en entorno validado / desplegado autorizado. Nunca usar completado para una conexión aún simulada.

## Bloqueos que requieren datos del entorno

- Host/MySQL remoto, allowlist IP Lightsail, certificado/TLS o túnel verificado, límites y soporte Percona.
- Instancia, distribución Linux, DNS/certificado API y acceso de despliegue.
- Buckets de documentos/backups, IAM y política de retención/restauración.
- SMTP/IMAP y cuentas; autorización para envíos reales.
- Series/fiscalidad y plantillas de documentos oficiales.
- Work/MCP y ejecutor RFQ; criterios comerciales/hora/lotes.

## Verificación de consolidación

9 de octubre de 2026: 17 pruebas offline del crawler satisfactorias; enlaces internos verificados. Commit cbe7a5ac5168f17dea9fc5430339eaad828b34b3 publicado en main; CI Documentation checks, Crawler tests y Single branch policy satisfactorio. API GitHub verificó exactamente una rama: main; tres refs anteriores eliminadas, commits preservados en historial.

GitHub devolvió 403 al consultar rulesets: exige GitHub Pro o repositorio público para habilitarlos en este repositorio. Mantener privado. No se habilitó bloqueo preventivo de creación; regla obligatoria y limpieza reactiva activas. Esta limitación requiere cambio de plan para garantía preventiva, no otro workflow.

## Guía de ejecución local

9 de octubre de 2026: se añadió `backend/crawler/USO.md` y se enlazó desde su README. Explica instalación Python 3.12, una corrida completa con S3/API deshabilitados, inspección de lotes, estados de salida y recuperación de pendientes. No cambia código ni contratos. Validación: 17 pruebas offline satisfactorias con Python 3.12.14; CLI `--help` verificada. Ejecución real de HonduCompras e integración externa aún pendientes.
