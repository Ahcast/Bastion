# Background jobs y dashboard

Fuente: [Documento maestro v0.5](../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

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

