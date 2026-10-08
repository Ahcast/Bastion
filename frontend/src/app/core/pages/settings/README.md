# settings

Fuente: [Documento maestro vigente](../../../../../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 21. Módulo Configuración

Debe incluir:

```text
Empresa
Usuarios
Roles
Permisos
Equipos
Impuestos
Monedas
Series de documentos
Plantillas
Email
Integraciones
AI Agents
Workflows
Aprobaciones
Categorías
Auditoría
Seguridad
```

Integraciones futuras:

- AWS;
- Gmail/email;
- Google Drive;
- GitHub;
- Grok;
- ChatGPT;
- S3;
- HonduCompras;
- otras fuentes.

---


## Administración del Core

Incluye usuarios, equipos, RBAC, MFA, sesiones, SMTP, conexiones API/MCP y auditoría. Cada operación está protegida por permiso y, cuando corresponda, reautenticación. Los secretos no se devuelven al navegador.

