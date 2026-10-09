# AI — Integración transversal de IA en Bastion

## 1. Propósito

El módulo `AI` es transversal. No reemplaza las reglas de negocio de Clientes, Procesos, Cotizaciones, Órdenes, Facturas, Gastos, Reportes o Colaboración y no agrega una entrada obligatoria al menú.

Su función es proporcionar una capa común para:

- análisis;
- clasificación;
- extracción estructurada;
- investigación;
- generación de borradores;
- asistencia al usuario;
- automatizaciones controladas;
- acceso de agentes mediante API/MCP;
- trazabilidad de ejecuciones, costos, herramientas y decisiones.

La aplicación debe seguir funcionando cuando cualquier proveedor de IA esté caído o deshabilitado.

## 2. Principio arquitectónico y ejecución
Ver [pipeline vigente](../architecture/ai-integration.md), [contrato](../contracts/ai-pipeline.md) y [resultados](ai-results.md).
Bastion guarda tareas -> ChatGPT Work con skills reclama pendientes mediante MCP -> investiga -> guarda resultados validados -> usuario revisa.
AI es transversal y administra estado; Core mantiene API/MCP, políticas de autorización, auditoría, archivos y correo. No hay acceso directo externo a MySQL, shell, cPanel, S3 completo ni SMTP genérico.

## 3. Ubicación
AI permanece en `backend/internal/modules/ai/`. Go ejecuta operaciones acotadas de administración de tareas y resultados. La investigación/modelo ocurre en Work, aprovechando prioritariamente el plan Plus del usuario, sujeto a prueba de capacidades reales.
Lightsail conserva crawler y backups. OCR pesado y otros ejecutores requieren validación separada. Grokbot atiende RFQ autorizadas; alojamiento/conexión por confirmar.
RAG inicial: documentos/texto en S3, índices/metadatos y recuperación por Go/MySQL. No requiere vector DB ni un servidor RAG dedicado.

## 4. Ejecutor y adaptadores
Work consulta la cola lógica mediante MCP; cron Go no llama a Plus como API ni despierta automáticamente el chat. Versionar skills y registrar la versión usada.
Adaptadores API de proveedores pueden añadirse en el futuro sin cambiar servicios de aplicación/políticas de autorización. No son requisito ni fallback automático de pago. La selección de modelo/uso depende del ejecutor; registrar solo datos observables, sin inventar tokens/costos.
Validar tareas desatendidas, plugin, permisos, búsqueda web, transferencia documental, lectura/escritura y capacidad diaria antes de activar.

## 5. Identidad delegada y autorización

Un agente actúa con una identidad explícita:

```text
human
ai-agent
system-automation
```

Cuando el agente asiste a un empleado, opera con identidad delegada y **no puede superar los permisos del empleado**.

```text
Empleado
   |
   v
AI Assistant
   |
   v
Tool Policy
   |
   v
misma autorización de dominio
```

Nunca:

```text
Empleado sin permiso
   -> IA
   -> credenciales admin
```

Cada tool declara:

- permiso requerido;
- tipos de entidad aceptados;
- riesgo;
- si admite ejecución automática;
- si requiere aprobación humana;
- esquema de entrada;
- esquema de salida.

## 6. Niveles de acción

### Nivel 1 — lectura/análisis

Puede ejecutarse automáticamente si el usuario tiene permiso de lectura:

```text
resumir
clasificar
extraer campos
explicar métricas
comparar
buscar información permitida
```

### Nivel 2 — borradores

Puede crear borradores auditables:

```text
cotización borrador
email borrador
supplier candidate
propuesta de items
comentarios sugeridos
```

### Nivel 3 — escritura interna reversible

Solo con permiso explícito y policy:

```text
crear tarea
registrar nota
actualizar un campo no crítico
```

### Nivel 4 — acciones sensibles o externas

Requieren aprobación humana por defecto:

```text
enviar email
emitir/enviar cotización
marcar orden Comprada/Entregada
crear factura definitiva
registrar pago
anular factura
fusionar clientes/proveedores
eliminar información
cambiar datos fiscales
```

Las excepciones futuras deben definirse mediante `automation_rules` y quedar auditadas.

## 7. Tools de Bastion

Tools transversales propuestas:

```text
search_processes()
get_process()
get_process_versions()

search_customers()
get_client()
suggest_client_match()

search_suppliers()
get_supplier()
find_supplier_candidates()
get_supplier_price_history()

generate_process_analysis()
create_quote_draft_from_process()
suggest_quote_items()
compare_quote_with_process()

get_quote()
calculate_quote_margin()
draft_quote_email()

get_order()
get_order_items()
suggest_supplier()
suggest_purchase_cost()
calculate_order_margin()

get_invoice()
compare_invoice_sources()

get_expense()
classify_expense()
detect_duplicate_expense()

get_report_metrics()
explain_report()

list_authorized_documents()
get_document_metadata()
summarize_document()
extract_document_fields()

create_task()
request_approval()
```

No se ofrece una tool `run_sql()` ni `execute_shell()`.

Para correo se prefieren tools de propósito limitado, por ejemplo:

```text
draft_supplier_rfq()
send_supplier_rfq()
draft_customer_email()
send_customer_email()
```

en lugar de un `send_email()` sin restricciones.

## 8. Persistencia y trazabilidad

Tablas transversales recomendadas:

```text
ai_tasks
ai_runs
agent_actions
ai_prompt_versions
ai_feedback
```

`ai_runs` como mínimo:

```text
id
task_type
entity_type
entity_id
actor_type
actor_id
provider
model
prompt_version
input_hash
status
started_at
finished_at
input_tokens
output_tokens
cost
confidence nullable
result_json
error_code nullable
created_at
```

`agent_actions`:

```text
id
ai_run_id
tool_name
risk_level
input_hash
status
approval_id nullable
started_at
finished_at
result_summary
```

No guardar secretos, credenciales ni contenido sensible innecesario en logs.

## 9. Idempotencia y reanudación
Aplicar [contrato IA](../contracts/ai-pipeline.md): reserva atómica con vencimiento, lease token, versión/hash de entrada e idempotencia. Rechazar escrituras tardías o desactualizadas como resultado vigente.
Puntuar una vez cada proceso pendiente, incluidos rezagados; marcar completado solo al guardar correctamente. Versiones nuevas relevantes pueden crear revisiones separadas bajo política explícita. Cambiar prompt/skill/modelo no dispara por sí solo reprocesamiento masivo.
Guardar avances por ítem; fallos y límites de Work dejan pendientes recuperables. Datos insuficientes no equivalen a irrelevancia.

## 10. Datos externos y prompt injection

Todo contenido procedente de:

```text
HonduCompras
PDFs
Word/Excel
correo
web
proveedores
clientes
archivos subidos
```

se trata como:

```text
UNTRUSTED_EXTERNAL_CONTENT
```

Reglas:

- instrucciones encontradas dentro de documentos no cambian permisos;
- el modelo no decide qué tools puede usar;
- tool calls se validan nuevamente en Go;
- URLs, archivos y outputs deben pasar validación;
- las credenciales nunca se insertan en prompts;
- un documento no puede autorizar envíos, pagos o escrituras.

## 11. Pipeline asíncrono
1. Puntuación diaria con skill sobre pendientes.
2. Investigación de ítems relevantes: precios publicados/catálogos y comparación técnica.
3. Paso 2.5: RFQ de alto interés sin precio; ChatGPT prepara y Grokbot tramita envío autorizado por Core.
4. Presentación progresiva dentro de Procesos.
5. Skill + RAG para rango de venta; cálculos deterministas en Go.
6. Ficha oficial o ficha propia de Castillo IT sustentada.
7. Usuario selecciona y crea/completa cotización.

Resultados por ítem y evidencia en cada etapa. RFQ pendiente no bloquea otros ítems. Jobs cortos de Lightsail, nunca requests abiertos durante investigaciones ni workers durmiendo por respuestas. UI consulta estado.

## 12. Roles funcionales de IA

### Supervisor / interfaz gerencial

Puede responder, según permisos:

- qué procesos entraron;
- cuáles requieren atención;
- cotizaciones pendientes;
- órdenes atrasadas;
- facturas/saldos;
- gastos y anomalías;
- estado del crawler/backups;
- actividad de automatizaciones e IA.

### Analista de Procesos / Procurement

- clasificación;
- scoring/relevancia;
- extracción de requisitos;
- identificación de items;
- búsqueda histórica;
- investigación de precio/proveedor;
- propuesta de RFQ.

### Asistente de Oficina

Puente entre empleado y Bastion:

- buscar cliente/proveedor/proceso;
- preparar cotización;
- generar documento;
- preparar correo;
- consultar orden/factura;
- producir reporte;
- solicitar aprobación.

### Ingeniería / Codex

GitHub, desarrollo, tests, PRs, refactor y migraciones. No se considera agente operativo de procurement ni obtiene acceso automático a datos de producción.

## 13. Integración por módulo

| Módulo | Capacidades IA principales | Límite crítico |
|---|---|---|
| Inicio | resumen ejecutivo, prioridades, alertas explicadas | no recalcula KPIs |
| Clientes | matching, duplicados sugeridos, resumen de relación | no fusiona automáticamente |
| Proveedores | investigación, candidatos, historial, RFQ borrador | fuentes/evidencia obligatorias |
| Procesos | clasificación, resumen, requisitos, relevancia, items sugeridos | raw del crawler es inmutable |
| Cotizaciones | items sugeridos, comparación con proceso, margen, email | crear desde proceso no depende de IA; envío requiere aprobación |
| Órdenes | proveedor/costo sugerido, riesgo/margen, seguimiento | no marca comprado/entregado automáticamente |
| Facturas | validación cruzada, borrador, inconsistencias | no altera correlativo, pagos o SAR |
| Gastos | OCR/extracción, clasificación, duplicados | usuario confirma antes de registrar |
| Reportes | narrativa, anomalías, explicación | usa métricas canónicas del backend |
| Colaboración | resumen/extracción de documentos autorizados | respeta ACL y prompt-injection controls |

## 14. Proceso -> Cotización y IA

La conversión básica es determinista:

```text
Proceso
  -> cliente / entidad
  -> expediente
  -> Cotización Borrador
```

No requiere modelo de IA.

Después puede ejecutarse de forma separada:

```text
Cotización borrador
  -> suggest_quote_items(process_id)
  -> propuesta estructurada de items
  -> revisión humana
  -> agregar items aprobados
```

Esto mantiene la creación operativa aun cuando la IA no esté disponible.

## 15. API y MCP

REST sigue siendo la API principal de la SPA y de integraciones de servicio.

MCP expone tools de Bastion a agentes externos reutilizando los mismos servicios de aplicación y políticas de autorización.

```text
REST Controller ----\
                    -> Domain Action -> Policy -> DB/S3/Email
MCP Tool -----------/
AI internal Tool ---/
```

No crear tres implementaciones de la misma regla.

## 16. Eventos IA

Eventos recomendados:

```text
ai.task_created
ai.run_started
ai.run_completed
ai.run_failed
ai.approval_required
ai.tool_called
```

Eventos de negocio pueden disparar IA sin convertir la IA en parte de la transacción principal:

```text
process.created -> enqueue_initial_scoring_if_pending
process.updated -> request_revision_if_relevant_under_policy
document.uploaded -> extract_document_fields
```

## 17. Criterios de aceptación

- Ningún módulo llama directamente a un proveedor IA.
- Ninguna API key de IA llega a Angular.
- IA no tiene acceso directo a MySQL, shell ni credenciales SMTP.
- Cada tool aplica permisos/políticas de autorización.
- Cada ejecución relevante queda en `ai_runs`.
- Outputs estructurados se validan con schema antes de usarse.
- Acciones sensibles requieren aprobación por defecto.
- El sistema funciona sin IA.
- Procesos y documentos externos se tratan como contenido no confiable.
- El proveedor/modelo puede cambiar sin reescribir los módulos.

## 18. Integración con Core

AI consume [Core](../architecture/core.md), cumple el [contrato modular](../contracts/modules.md) y sigue la [arquitectura](../architecture/README.md).

Workflow conserva aprobaciones y transiciones; AI solicita aprobaciones, no las implementa por su cuenta.

## Desarrollo operativo del pipeline

La skill define criterios de negocio aún por concretar. Fuentes y variantes exactas por oferta; datos desconocidos no son cero. RFQ registra contactos verificados, autorización y respuesta correlacionada; SMTP incierto no se reenvía a ciegas. Precio sugerido utiliza antecedentes con condiciones comparables, y fichas propias declaran autoría sin inventar certificaciones. Usuario puede incorporar únicamente selección revisada a Cotizaciones; envío de oferta sigue sujeto a autorización.


