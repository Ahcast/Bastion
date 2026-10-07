# Proyecto Bastion

Plataforma operativa de Castillo IT. Aplicación modular en Bluehost y extracción técnica en AWS.

| Ubicación | Responsabilidad |
| --- | --- |
| [Core backend](bluehost/backend/app/Core/README.md) | Usuarios, RBAC, MFA, configuración, SMTP, API, MCP y servicios comunes. |
| [Módulos backend](bluehost/backend/app/Modules/README.md) | Reglas y datos de los módulos existentes. |
| [Frontend](bluehost/frontend/README.md) | Angular: Core, módulos y componentes compartidos. |
| [AWS](aws/README.md) | Crawler y workers en Lightsail; almacenamiento en S3. |
| [Documentación](docs/README.md) | Arquitectura, contratos y seguridad. |

## Estado

La aplicación de Bluehost sigue siendo una estructura documental; esta reorganización no instala Laravel ni Angular. El crawler en AWS ya contiene código y pruebas. No se despliega ningún servicio al reorganizar carpetas.

Dominio definido: `https://bastion.castilloit.net`.

El [maestro vigente v0.6](docs/Proyecto_Bastion_Documento_Maestro.md) consolida alcance, infraestructura e integración IA. La [estructura Core y módulos](docs/architecture/core-modules.md) detalla rutas y dependencias. El historial permanece en Git; se mantienen los módulos funcionales existentes.

Interfaz y vistas en español; nombres técnicos en inglés. Ver [idioma](docs/idioma.md).

## Lineamientos consolidados

Ver [core.md](docs/architecture/core.md) para el alcance completo del Core, infraestructura confirmada, tecnologías API/MCP, seguridad, operación y decisiones pendientes de esta sesión.

## Integración de IA

[Arquitectura y pipeline](docs/architecture/ai-integration.md): puntuación diaria de pendientes, investigación por ítem, RFQ con Grokbot, resultados dentro de Procesos, precio sugerido con [RAG S3/MySQL](docs/knowledge-rag/README.md) y fichas técnicas. ChatGPT Work/Plus es el ejecutor externo prioritario mediante MCP, sujeto a prueba de capacidades. El usuario revisa y crea la cotización. Esta actualización es documental.
