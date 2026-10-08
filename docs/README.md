# Documentación

El [documento maestro vigente v0.7](Proyecto_Bastion_Documento_Maestro.md) consolida alcance y decisiones transversales. Leerlo antes del detalle del área. Las versiones anteriores se consultan en Git, no son instrucciones alternativas de implementación.

## Referencias vigentes

- [Arquitectura objetivo Oficina AI v2](architecture/oficina-ai-v2.md)
- [Lineamientos por módulo](modules/README.md)
- [Core](architecture/core.md)
- [Separación Core y módulos](architecture/core-modules.md)
- [Contrato modular](contracts/modules.md)
- [Seguridad](security/README.md)
- [Crawler](contracts/crawler-import.md)
- [API/MCP](contracts/api-mcp.md)
- [Observabilidad](observability/README.md)
- [Roadmap](roadmap/README.md)
- [RAG de precios: S3 y MySQL](knowledge-rag/README.md)
- [Integración y pipeline IA](architecture/ai-integration.md)
- [Contrato de tareas IA](contracts/ai-pipeline.md)
- [Presentación de resultados](modules/ai-results.md)
- [Despliegue](deployment/README.md)

[Política de idioma](idioma.md): interfaz y contenido visible en español; nombres técnicos en inglés permitidos.

## Precedencia de arquitectura

`architecture/oficina-ai-v2.md` recoge la arquitectura general. `architecture/ai-integration.md` desarrolla y actualiza la ejecución de IA: ChatGPT Work/Plus y skills mediante MCP, tareas/resultados en Bastion y RAG inicial S3/MySQL. Bluehost aloja Angular, MySQL y email. AWS Lightsail aloja backend Go, API, MCP, crawler y workers. Crawler y backup workers permanecen en AWS Lightsail; GitHub Actions se limita a CI/CD. Core mantiene identidad, API/MCP y seguridad. Los documentos describen objetivos, no conexiones o despliegues verificados.

Los documentos de módulos preservan las funcionalidades legacy verificadas, pero aplican seguridad server-side, persistencia transaccional en MySQL, archivos privados en S3, idempotencia, auditoría, eventos y conexión controlada con IA.

