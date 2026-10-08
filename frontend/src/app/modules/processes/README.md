# processes

Fuente: [Documento maestro vigente](../../../../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 14. Módulo Procesos

Fuente principal:

HonduCompras crawler.

Estados de negocio posibles:

```text
Nuevo
Revisión
Asignado
Analizando
Interesante
Cotizando
Oferta preparada
Presentado
Adjudicado
Perdido
Cancelado
Cerrado
```

Funciones:

- ver proceso;
- ver histórico;
- ver cambios;
- ver documentos;
- asignar responsable;
- asignar equipo;
- prioridad;
- comentarios;
- tareas;
- análisis AI;
- score;
- items;
- crear cotización;
- iniciar research;
- iniciar procurement.

---

## Pipeline IA y revisión del usuario — 2026-10-07

Ver [integración](../../../../../docs/architecture/ai-integration.md), [contrato](../../../../../docs/contracts/ai-pipeline.md) y [presentación](../../../../../docs/modules/ai-results.md). Puntuación diaria de pendientes, investigación por ítem, RFQ con Grokbot, precio sugerido con antecedentes S3/MySQL y fichas. Resultados progresivos dentro de Procesos, con fuentes, fecha y cumplimiento por requisito. Estado de investigación separado de revisión humana y presentación de oferta.

La creación básica de cotización sigue operativa sin IA. Acción adicional explícita del usuario incorpora solo alternativas, cantidades, precios y adjuntos seleccionados, mediante Quotes y sin sobrescribir ediciones humanas. No se exportan automáticamente costos, proveedores, márgenes ni notas internas al cliente.

