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

## 2. Principio arquitectónico

```text
Usuario / evento / job
        |
        v
Laravel Domain Action
        |
        v
AI Gateway
        |
        +--> Prompt Registry
        +--> Tool Registry
        +--> Policy / Approval Engine
        +--> Provider Router
        |
        +--> OpenAIProvider
        +--> FutureProvider
        |
        v
Structured Result
        |
        v
Domain Action / Draft / Review
```

La IA **nunca** recibe acceso directo a MySQL, credenciales de administración, shell del servidor, cPanel, S3 completo ni SMTP genérico.

Toda interacción con Bastion ocurre mediante tools de aplicación que reutilizan las mismas Actions/Services y Policies usadas por REST y MCP.

## 3. Ubicación

El AI Gateway comienza como parte del monolito Laravel en Bluehost:

```text
bluehost/backend/app/Modules/AI/
  Providers/
  Tools/
  Prompts/
  Policies/
  DTO/
  Jobs/
  Actions/
```

No se crea un microservicio de IA en la primera etapa.

Los modelos se consumen como servicios externos. Los jobs cortos pueden ejecutarse mediante Laravel database queue y Scheduler. Si en el futuro aparecen cargas largas o intensivas, podrán delegarse a un worker dedicado sin cambiar el contrato de tools ni los módulos de negocio.

AWS Lightsail mantiene su responsabilidad aprobada: crawler HonduCompras y workers técnicos de backup hacia S3. No se asume como runtime de IA salvo decisión futura explícita.

## 4. Abstracción de proveedor

Los módulos no deben llamar directamente a un SDK de proveedor.

Contrato conceptual:

```text
AIProvider
  -> generateStructured()
  -> generateText()
  -> useTools()
```

Adaptadores:

```text
OpenAIProvider
FutureProvider
LocalProvider opcional futuro
```

La aplicación solicita capacidades, no modelos concretos:

```text
analyze_process()
extract_requirements()
compare_products()
classify_expense()
summarize_document()
draft_email()
```

El proveedor/modelo real se decide en el AI Gateway según configuración, costo, disponibilidad y política.

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

search_clients()
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
ai_tool_calls
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

`ai_tool_calls`:

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

## 9. Idempotencia y cache semántica de tareas

Una misma tarea sobre la misma versión de entidad puede reutilizar resultado cuando:

```text
task_type + entity_version + prompt_version + input_hash
```

no han cambiado.

Ejemplo: si `process_versions.content_hash` es igual, no se vuelve a pagar automáticamente por el mismo resumen salvo solicitud explícita o cambio de prompt/modelo.

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
- tool calls se validan nuevamente en Laravel;
- URLs, archivos y outputs deben pasar validación;
- las credenciales nunca se insertan en prompts;
- un documento no puede autorizar envíos, pagos o escrituras.

## 11. Procesamiento síncrono y asíncrono

### Síncrono

Solo operaciones rápidas de UX:

```text
resumen corto
clasificación simple
redacción asistida
```

### Asíncrono

```text
análisis de procesos
extracción de documentos grandes
investigación de proveedores/precios
comparaciones extensas
generación de paquetes documentales
```

Flujo:

```text
Domain Event
  -> ai_task
  -> Laravel queue
  -> AI Gateway
  -> provider
  -> structured result
  -> ai_run
  -> Domain Event
```

La UI consulta estado o recibe actualización mediante el mecanismo que se defina; no mantiene una petición HTTP abierta durante tareas largas.

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

MCP expone tools de Bastion a agentes externos reutilizando las mismas Actions y Policies.

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
process.created -> analyze_process
process.updated -> reanalyze_if_content_changed
document.uploaded -> extract_document_fields
```

## 17. Criterios de aceptación

- Ningún módulo llama directamente a un proveedor IA.
- Ninguna API key de IA llega a Angular.
- IA no tiene acceso directo a MySQL, shell ni credenciales SMTP.
- Cada tool aplica permisos/Policies.
- Cada ejecución relevante queda en `ai_runs`.
- Outputs estructurados se validan con schema antes de usarse.
- Acciones sensibles requieren aprobación por defecto.
- El sistema funciona sin IA.
- Procesos y documentos externos se tratan como contenido no confiable.
- El proveedor/modelo puede cambiar sin reescribir los módulos.

## 18. Integración con Core

AI consume [Core](../../Core/README.md), cumple el [contrato modular](../../../../../docs/contracts/modules.md) y sigue la [arquitectura v2](../../../../../docs/architecture/oficina-ai-v2.md).

Workflow conserva aprobaciones y transiciones; AI solicita aprobaciones, no las implementa por su cuenta.
