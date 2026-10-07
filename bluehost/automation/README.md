# Background jobs y dashboard

Fuente: [Documento maestro vigente](../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 32. Background jobs

Operaciones que no deben bloquear request HTTP:

- importaciones;
- generación PDF;
- emails;
- AI tasks;
- document processing;
- reportes pesados;
- notificaciones.

Inicial:

- Laravel Queue;
- driver database.

Si Bluehost no permite worker persistente:

- cron;
- `queue:work --stop-when-empty`;
- o worker separado.

Futuro:

- Redis;
- Amazon SQS.

---


# 33. Dashboard realtime

MVP:

- polling cada 15–30 s.

Fase 2:

- WebSocket administrado:
  - Ably;
  - Pusher.

Laravel Reverb solo si Bluehost soporta procesos persistentes.

---

## Pipeline IA y programación

Work ejecuta la puntuación diariamente (hora por definir en America/Tegucigalpa), recoge pendientes de cualquier fecha y persiste avances mediante MCP. Laravel crea tareas/outbox y mantiene reservas, vencimientos, alertas y resultados; su cron no invoca la suscripción Plus ni garantiza despertar Work. Investigaciones se procesan por lotes según límites medidos. Jobs Bluehost acotados por tiempo/memoria/cantidad y sin solapamientos; stop-when-empty solo no basta. Ver [pipeline](../../docs/architecture/ai-integration.md). No se crean tareas programadas reales con este documento.
