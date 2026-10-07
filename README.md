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

La [estructura Core y módulos](docs/architecture/core-modules.md) actualiza la organización inicial del [documento maestro v0.5](docs/Proyecto_Bastion_Documento_Maestro.md), que se conserva como referencia histórica. Se mantienen los módulos funcionales existentes.

Interfaz y vistas en español; nombres técnicos en inglés. Ver [idioma](docs/idioma.md).
