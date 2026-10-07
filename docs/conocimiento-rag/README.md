# RAG pendiente

Fuente: [Documento maestro v0.5](../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 39. Knowledge / RAG Engine

El RAG es importante para IA, pero su diseño funcional **queda pendiente**.

Aclaración importante:

> El RAG no debe asumirse como una búsqueda de items iguales o equivalentes técnicos.

La relación histórica que debe usar para apoyar pricing y decisiones aún debe definirse.

Por ahora la arquitectura reserva:

```text
Knowledge/RAG Service
  │
  ├── S3 documents
  ├── vector/knowledge index
  └── Bastion API/MCP
```

No se fija todavía:

- Qdrant;
- embeddings;
- reglas de similitud;
- infraestructura final;
- lógica de pricing basada en RAG.

El diseño RAG tendrá revisión independiente.

---

