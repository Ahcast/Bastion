# Instrucciones obligatorias para desarrollar Bastion

## Objetivo y orden de lectura

Implementar la aplicación completa por fases, preservando funcionalidades documentadas. Leer README.md, docs/Proyecto_Bastion_Documento_Maestro.md, docs/architecture/README.md, docs/development/stack.md y docs/development/plan.md. Después leer Core, modelo de datos y especificación del módulo que se va a modificar. No depender de memoria del chat ni de versiones antiguas de Git.

## Reglas de trabajo

1. Única rama permitida: main, local y remota. No crear ramas temporales, feature, develop, staging, release ni ramas para PR; no forks. Versionar mediante commits pequeños. No force-push, no reescribir historial. Actualizar main y resolver conflictos preservando trabajo existente antes de publicar. Un único escritor por checkout; no agentes concurrentes editando archivos.
2. Backend Go/Core/API/MCP/crawler Python/workers en Lightsail. Frontend Angular, MySQL y correo en Bluehost. Archivos en S3. Sin OCR, modelos locales o servidor vectorial en Lightsail. No backend PHP intermediario ni MySQL en AWS.
3. Monolito modular. Core no importa módulos de negocio. REST y MCP invocan los mismos servicios de aplicación internamente. Crawler no escribe directamente a la BD de negocio.
4. UI y documentos para usuarios en español; identificadores técnicos en inglés. Mantener el menú aprobado; AI y Workflow son transversales sin entrada adicional.
5. No ampliar alcance con SaaS, multiempresa, microservicios, Redis/SQS, proveedores de inferencia pagados, passkeys o compra a proveedores sin instrucción específica.
6. Nunca secretos, datos comerciales reales, dumps o documentos privados en Git, logs, mocks o frontend. Ejemplos sintéticos. No imprimir credenciales.
7. Seguridad e integridad pertenecen al backend: denegación por defecto, MFA, RBAC por acción/registro, CSRF, auditoría, idempotencia, transacciones y controles de versión. No float64 financiero.
8. Preservar crawler y sus tiempos/reintentos, interfaz CLI, esquema y ACK. Cambiar contrato implica migración y pruebas, no una sustitución silenciosa.
9. Implementar y probar una fase antes de declarar completada. No mocks en rutas productivas, TODO como sustituto de comportamiento ni excepciones silenciosas. Una prueba no acredita conexión real ni despliegue.
10. Resolver decisiones rutinarias de implementación usando el stack y registrar dependencias exactas; no pedir aprobación para cada librería. Bloqueos de cuenta, normativa fiscal, integración externa o credenciales se registran en docs/development/status.md y no se inventan. Continuar trabajo independiente.
11. Antes de cada cambio de dominio, definir migración, DTO, permisos, reglas, estados, errores y aceptación del módulo. Actualizar contratos junto al código; no mantener especificaciones alternativas.
12. Ejecutar scripts/check_repository.py y pruebas pertinentes. Al existir código Go: gofmt, go vet, go test; Angular: npm ci, build y pruebas de comportamiento; MySQL 5.7: integridad/concurrencia reales. No sustituir estas pruebas por SQLite.
13. Desarrollo y pruebas locales/sintéticas están autorizados. Desplegar, activar correo, modificar infraestructura, migrar BD productiva o enviar mensajes requiere autorización correspondiente. El agente entrega primero una implementación revisable.

## Continuidad y autoridad

Usuario explícito vigente > maestro (alcance/hosting) > arquitectura/stack/datos/contratos (reglas compartidas) > especificaciones de módulos (función) > código para estado implementado. Un ejemplo de módulo no modifica el contrato compartido. Si cambia una decisión actualizar todos los afectados y documentar razón en docs/development/decisions.md. El historial Git es historia, no otra especificación vigente.

Registrar fase, archivos, pruebas y resultados en status.md al terminar cada unidad. No marcar integración externa validada sin evidencia. La política de rama también se aplica a herramientas automáticas; deshabilitar propuestas automáticas que creen ramas.
