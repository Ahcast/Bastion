# Decisiones de consolidación — 9 de octubre de 2026

| Contradicción o redundancia | Resolución vigente |
| --- | --- |
| Backend Bluehost/PHP frente a Go AWS | Go en Lightsail; Angular/BD/correo Bluehost |
| Misma origin frente a frontend/API separados | CORS exacto; cookie API host-only; CSRF entregado en JSON y enviado por Angular explícitamente |
| clients vs customers | customers, customer_id y customer_*_snapshot; REST /customers |
| orders vs OC de clientes | customer_purchase_orders/items; REST /purchase-orders; futuras OC proveedores separadas |
| invoice_payments vs payments | payments vinculados a invoice; no tabla paralela |
| quote_counter específico vs correlativos comunes | document_counters con tipo/serie y bloqueo transaccional |
| ai_tool_calls vs agent_actions | agent_actions para auditoría de llamadas; ai_runs para ejecuciones |
| /ingestion vs API del crawler existente | /api/internal/v1/crawler y ACK existente; sin alias ni 202 IMPORTED |
| README repetidos, maestro extenso y arquitectura v2 alternativa | Una spec por tema; maestro breve con enlaces; eliminados duplicados, carpeta vacía no es requisito |
| OCR/IA local implícita | OCR externo futuro; sin inferencia/OCR en Lightsail |
| CI protected-main/PR frente a rama única | Commits en main, sin ramas/PR; CI y revisión de commits; control detectivo no sustituye restricción preventiva GitHub |
| Aprobación senior global antes de todo código | Implementar por fases con aceptación; bloqueos externos no paralizan módulos independientes |
| Angular 22/TS6 asumidos | Elegir matriz soportada al inicializar y fijar lockfiles; no supuestos incompatibles |
| Roles comerciales/umbrales fiscales/IA no definidos | Configurables; sin automatización ni privilegios implícitos; registrar validación pendiente |

Las reglas de precisión/redondeo/IDs JSON son convenciones de implementación, no cambios a impuestos o precios comerciales. El cálculo conserva descuento absoluto por unidad y distribución proporcional; validar con ejemplos legacy antes de migrar datos.

Las ramas antiguas se conservan como padres históricos del commit de consolidación para no perder sus commits divergentes; sus tecnologías obsoletas no se reintroducen en la spec vigente. La limpieza remota elimina solo refs que ya son ancestros de main, con verificación SHA antes de borrar.

No hubo despliegue productivo, migración real ni envío. Las verificaciones locales se reportan separadas de las de GitHub.

## Implementación Core

Go 1.27.2 y dependencias fijadas; Angular 22.2.2/TypeScript compatible fijados tras build. Argon2id 32 MiB/3 iteraciones/paralelismo 1, máximo dos cálculos por proceso; calibrar en Lightsail. Sesiones simultáneas permitidas dentro de vencimientos, sin imponer una sesión única. Bootstrap técnico protegido, sin conceder permisos financieros. Cookies de desarrollo separadas de producción. Scope all/self/team, global administrativo all. Archivos comerciales sin política de módulo fallan cerrados. Entrega SMTP incierta exige conciliación. MCP inicial bearer técnico limitado, separado de OAuth delegado pendiente. No se amplió el menú ni se implementaron módulos comerciales ficticios.

## 2026-10-09 — Clientes y desarrollo secuencial

Clientes primero, luego Proveedores y demás propietarios según plan. Cada unidad cierra sus pruebas y CI antes de comenzar la siguiente, según instrucción explícita del usuario. Normalización canónica Unicode NFKD/diacríticos/espacios, RTN opcional con 14 dígitos; UNIQUE conserva deduplicación incluso en inactivos. Ficha con versión, contactos/direcciones relacionales y notas append-only; no merge/borrado automático. Scopes all/self/team con propietario inmutable; cambios de equipo exigen all y reauth. RegisterPermissions añade capacidades asignables, no permisos automáticos a administradores.

Hechos comerciales en domain_events, sin transformarlos en trabajos desconocidos; outbox queda para jobs registrados. DocumentPolicies permite a Core combinar permisos del archivo y la entidad sin importar módulos. MCP usa contexto autenticado y schemas explícitos para mantener IDs/versiones como strings. OAuth/Work y servicios externos siguen pendientes. Vista 360° se integra por propietario disponible, sin fabricar actividad futura.

CI usa la imagen oficial MySQL 5.7.44 desde el mirror Docker Official Images de ECR Public, fijada por digest sha256:4bc6bc963e6d8443453676cae56536f4b8156d78bae03c0145cbe47c2aad73bb. Se verificaron manifiesto amd64 y MYSQL_VERSION=5.7.44-1.el7. Docker Hub bloqueó el primer run de Clientes por límite anónimo antes de ejecutar backend/browser. El cambio de registro afecta solo pruebas; no crea MySQL productivo en AWS. Fuente: https://gallery.ecr.aws/docker/library/mysql y manifiestos públicos del registro.
