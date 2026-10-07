# Quotes

Fuente: [Documento maestro vigente](../../../../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 15. Módulo Cotizaciones

Origen:

```text
Proceso -> Cotización
```

o:

```text
Cliente -> Cotización directa
```

Datos:

- cliente;
- proceso;
- items;
- cantidades;
- costo;
- precio;
- impuestos;
- margen;
- flete;
- proveedores;
- documentos;
- datasheets;
- versiones;
- approvals.

Estados:

```text
Borrador
Costeo
Revisión
Aprobada internamente
Enviada
Aceptada
Rechazada
Vencida
```

---


# 46. Versionado y consistencia

## Procesos

Cada cambio significativo:

- nueva version;
- diff;
- hash;
- timestamp;
- trigger AI si corresponde.

## Cotizaciones

Deben soportar versionado cuando se modifica una cotización enviada o aprobada.

## Documentos

Document hash para detectar reemplazos.

---


## Integración con Core

Este módulo conserva su alcance funcional. Consume [Core](../../Core/README.md) y cumple el [contrato modular](../../../../../docs/contracts/modules.md). Sus endpoints REST y herramientas MCP reutilizan las mismas Actions/Services y Policies.

## Pipeline IA y revisión del usuario — 2026-10-07

Ver [integración](../../../../../docs/architecture/ai-integration.md), [contrato](../../../../../docs/contracts/ai-pipeline.md) y [presentación](../../../../../docs/modules/ai-results.md). Puntuación diaria de pendientes, investigación por ítem, RFQ con Grokbot, precio sugerido con antecedentes S3/MySQL y fichas. Resultados progresivos dentro de Procesos, con fuentes, fecha y cumplimiento por requisito. Estado de investigación separado de revisión humana y presentación de oferta.

La creación básica de cotización sigue operativa sin IA. Acción adicional explícita del usuario incorpora solo alternativas, cantidades, precios y adjuntos seleccionados, mediante Quotes y sin sobrescribir ediciones humanas. No se exportan automáticamente costos, proveedores, márgenes ni notas internas al cliente.
