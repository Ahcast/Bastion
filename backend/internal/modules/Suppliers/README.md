# Suppliers

Fuente: [Documento maestro vigente](../../../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 13. Módulo Proveedores

Debe contener:

- empresa;
- país;
- contactos;
- emails;
- categorías;
- marcas;
- productos;
- RFQ;
- cotizaciones recibidas;
- precios;
- lead times;
- MOQ;
- términos de pago;
- documentos;
- historial;
- evaluación;
- confiabilidad;
- tiempos de respuesta.

---


## Integración con Core

Este módulo conserva su alcance funcional. Consume [Core](../../core/README.md) y cumple el [contrato modular](../../../../docs/contracts/modules.md). Sus endpoints REST y herramientas MCP reutilizan los mismos servicios de aplicación/Services y políticas de autorización.

