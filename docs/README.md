# Documentación

[Documento maestro v0.5](Proyecto_Bastion_Documento_Maestro.md) se conserva como referencia funcional e histórica de la estructura inicial.

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
- [RAG pendiente](knowledge-rag/README.md)
- [Despliegue](deployment/README.md)

[Política de idioma](idioma.md): interfaz y contenido visible en español; nombres técnicos en inglés permitidos.

## Precedencia de arquitectura

`architecture/oficina-ai-v2.md` recoge la arquitectura objetivo más reciente para el nuevo desarrollo y los módulos: Bluehost Choice Plus aloja Angular, Laravel, MySQL y email; Cloudflare protege el perímetro; S3 almacena documentos, snapshots y backups; y GitHub Actions se plantea como ejecutor del crawler. Cuando una referencia anterior conserve supuestos de infraestructura incompatibles —por ejemplo, Lightsail como ejecutor permanente— prevalece la arquitectura v2 hasta que esos documentos históricos sean consolidados.

Los documentos de módulos preservan las funcionalidades legacy verificadas, pero aplican seguridad server-side, persistencia transaccional en MySQL, archivos privados en S3, idempotencia, auditoría, eventos y conexión controlada con IA.
