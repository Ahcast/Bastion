# Collaboration

Fuente: [Documento maestro v0.5](../../../../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 20. Módulo Colaboración

No debe intentar recrear Google Drive completo.

Debe permitir:

- archivos;
- carpetas;
- documentos;
- comentarios;
- menciones;
- tareas;
- notas;
- versiones;
- relación con entidades.

Ejemplo:

```text
Proceso ENEE-123/
├── TDR.pdf
├── oferta_tecnica.docx
├── datasheets/
├── cotizacion_proveedor.pdf
├── comentarios
└── tareas
```

Los archivos pueden vivir en S3.

MySQL guarda:

- metadata;
- entity link;
- owner;
- timestamps;
- version;
- S3 key.

---


## Integración con Core

Este módulo conserva su alcance funcional. Consume [Core](../../Core/README.md) y cumple el [contrato modular](../../../../../docs/contracts/modules.md). Sus endpoints REST y herramientas MCP reutilizan las mismas Actions/Services y Policies.
