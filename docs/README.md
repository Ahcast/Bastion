# Mapa de documentación

Cada tema tiene una fuente única. No se reproducen capítulos del maestro en cada carpeta.

| Tema | Documento |
| --- | --- |
| Punto exacto para reanudar desarrollo | [Checkpoint operativo](ESTADO_DESARROLLO.md) |
| Alcance y hosting aprobados | [Maestro](Proyecto_Bastion_Documento_Maestro.md) |
| Instrucciones del agente | [AGENTS](../AGENTS.md) |
| Topología y estructura | [Arquitectura](architecture/README.md) |
| Tecnologías | [Stack](development/stack.md) |
| Secuencia y aceptación | [Plan](development/plan.md) |
| Historial amplio de trabajo y bloqueos | [Estado histórico](development/status.md) |
| Decisiones y contradicciones resueltas | [Decisiones](development/decisions.md) |
| Una sola rama | [Política Git](development/git-policy.md) |
| Identidad, permisos, seguridad y servicios compartidos | [Core](architecture/core.md) |
| Datos y nomenclatura | [Modelo](data-model.md) |
| API/MCP | [Contrato](contracts/api-mcp.md) |
| Registro modular | [Módulos](contracts/modules.md) |
| Funcionalidades por módulo | [Índice](modules/README.md) |
| Interfaz e Inicio/Configuración | [Frontend](frontend.md) |
| Pipeline IA | [Integración](architecture/ai-integration.md), [contrato](contracts/ai-pipeline.md) |
| RAG y resultados | [RAG](knowledge-rag/README.md), [UX IA](modules/ai-results.md) |
| Crawler existente y ACK | [Operación](../backend/crawler/OPERATIONS.md), [contrato](../backend/crawler/docs/IMPORT_CONTRACT.md) |
| Deploy, backup, monitorización | [Operaciones](operations.md) |
| Idioma | [Español](idioma.md) |

Documentos eliminados por duplicación siguen en el historial Git; no son instrucciones vigentes. No se pierde el alcance comercial por consolidar archivos.

Core implementado: [ejecución y pruebas](development/core-runbook.md), [OpenAPI](contracts/core-openapi.json). Catálogos implementados: [Clientes](development/customers-runbook.md) y [Proveedores](development/suppliers-runbook.md).

Cotizaciones es el módulo activo. Su contrato vigente está en [quotes.md](contracts/quotes.md), el contrato REST maduro en [quotes-openapi.json](contracts/quotes-openapi.json) y la operación/pruebas en [quotes-runbook.md](development/quotes-runbook.md). Borradores, cálculo, archivos/PDF, estados, aprobación, emisión, correo comercial y bulk están implementados.

FX se desarrolla por subunidades para no inventar una política comercial. El **registro histórico de observaciones USD/HNL** tiene contrato propio en [quotes-fx.md](contracts/quotes-fx.md), REST en [quotes-fx-openapi.json](contracts/quotes-fx-openapi.json) y ejecución/pruebas en [quotes-fx-runbook.md](development/quotes-fx-runbook.md). Esa subunidad no convierte cotizaciones ni elige TCR/compra/venta automáticamente. Las decisiones todavía no normativas de conversión siguen separadas en [Propuesta FX](development/quotes-fx-proposal.md). El checkpoint indica qué hito está certificado y las validaciones externas todavía pendientes.

Clonación OC: [contrato propietario](contracts/purchase-order-cloning.md), origen/versión durable y reinicio operacional sin copia de adjuntos.
