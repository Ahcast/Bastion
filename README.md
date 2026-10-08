# Proyecto Bastion

Plataforma modular de Castillo IT. Frontend en español; nombres técnicos en inglés.

| Ruta | Responsabilidad | Alojamiento |
| --- | --- | --- |
| [frontend](frontend/README.md) | Angular, shell, Core visual y módulos | Bluehost |
| [backend](backend/README.md) | Go: Core, módulos, API, MCP, crawler Python y workers | AWS Lightsail |
| [BD](backend/database/README.md) | Esquema, migraciones y propiedad de datos | Percona/MySQL en Bluehost |
| [Archivos](backend/storage/s3/README.md) | Binarios, documentos y respaldos | Amazon S3 |
| [docs](docs/README.md) | Maestro, arquitectura, contratos y seguridad | GitHub |

La ubicación del código no determina dónde se aloja la BD: su esquema pertenece al backend y el servidor permanece en Bluehost. El correo también permanece en Bluehost.

Frontend: https://bastion.castilloit.net. API/MCP: https://api.bastion.castilloit.net (subdominio propuesto, DNS por validar).

## Estado

Core Go y Angular siguen siendo estructura documental; el crawler Python contiene implementación y pruebas. Reorganizar el repositorio no despliega ni migra servicios. No se agregan módulos de negocio.

Leer el [maestro](docs/Proyecto_Bastion_Documento_Maestro.md), [Core](docs/architecture/core.md), [estructura](docs/architecture/core-modules.md) e [integración IA](docs/architecture/ai-integration.md). La IA se ejecuta externamente; Lightsail no ejecuta OCR ni modelos locales.
