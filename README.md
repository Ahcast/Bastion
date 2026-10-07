# Proyecto Bastion

Ecosistema operativo de Castillo IT. Estructura basada en el [Documento maestro v0.5](docs/Proyecto_Bastion_Documento_Maestro.md), 6 de octubre de 2026.

| Carpeta | Responsabilidad |
| --- | --- |
| [bluehost/](bluehost/README.md) | Angular, Laravel, MySQL, correo y núcleo de negocio |
| [aws/](aws/README.md) | Lightsail: crawler y workers técnicos. S3: raw, documentos y backups |
| [docs/](docs/README.md) | Documento maestro y arquitectura compartida |

Solo carpetas y Markdown. La arquitectura conceptual es la base aprobada; las versiones, contratos y detalles técnicos siguen sujetos a la revisión senior indicada en el documento maestro.
Se conserva la separación por entorno solicitada, dentro de este repositorio. No se crean otros repositorios ni recursos externos.

## Idioma obligatorio
Bastion se desarrolla y se presenta en español. Ver [requisito de idioma](docs/idioma.md). Las carpetas y módulos funcionales usan nombres españoles.
