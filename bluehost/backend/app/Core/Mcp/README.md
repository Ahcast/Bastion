# Mcp

Servidor laravel/mcp bajo /mcp. Registro de herramientas por módulo, autenticación, autorización por conexión, límites y auditoría. Passport para delegación OAuth y Sanctum para clientes técnicos compatibles. Las herramientas llaman las mismas Actions que REST. La versión del protocolo y compatibilidad del cliente requieren prueba.

Área del [Core](../README.md), no un módulo de negocio. Estructura documental pendiente de implementación.

## Herramientas de IA

El [contrato IA](../../../../../docs/contracts/ai-pipeline.md) aporta claim/save/complete, consulta documental y RFQ acotada. El Core autentica y autoriza cada llamada, incluidos filtros y descargas; los módulos registran herramientas hacia sus Actions. Work/Plus y Grokbot requieren pruebas de interoperabilidad antes de habilitar escrituras.
