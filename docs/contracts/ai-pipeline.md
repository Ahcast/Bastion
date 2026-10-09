# Contrato propuesto del pipeline IA
Fecha: 2026-10-07. Contrato de diseño; nombres de herramientas y esquemas pendientes de implementación y pruebas. Ver [arquitectura IA](../architecture/ai-integration.md).

## Herramientas y límites
| Herramienta | Entrada principal | Salida/efecto |
| --- | --- | --- |
| claim_scoring_tasks | límite validado | tareas pendientes reservadas, lease y versión de entrada |
| get_process_context | proceso y versión | campos/documentos autorizados y completitud |
| save_process_score | tarea, lease, resultado | puntuación persistida y validada |
| claim_research_tasks | límite validado | investigación por ítem pendiente |
| save_item_research | tarea, lease, candidato/oferta y evidencia | avance persistido, sin sobrescribir selección humana |
| create_rfq_request | ítems, destinatarios verificados, contenido | RFQ pendiente de autorización/envío |
| send_supplier_rfq | RFQ y versión autorizada | envío acotado mediante Core |
| record_supplier_quote | RFQ, mensaje fuente, valores | respuesta vinculada; ambigüedades a revisión |
| search_pricing_history | modelo/categoría/fechas/filtros | antecedentes autorizados y fuentes |
| save_price_recommendation | ítem, costos versionados, comparables | recomendación provisional o revisable |
| register_artifact | carga autorizada, checksum, vínculos | documento verificado o pendiente |
| complete_task | tarea, lease y resumen | cierre solo si cumple validaciones de etapa |

Son herramientas propuestas, no endpoints ya disponibles. El catálogo no concede permisos. REST y MCP usan los mismos contratos, servicios de aplicación, políticas de autorización e idempotencia. Core mantiene OAuth/MCP según core.md y credenciales de servicio separadas para Grokbot/worker.

## Sobre común
Cada escritura incluye schema_version, task_id, run_id, input_version, input_hash, skill_version, idempotency_key y, cuando usa reserva, lease_token. Bastion deriva actor/requested_by de autenticación, nunca de un argumento libre.
Cada resultado incluye status, sources, warnings, missing_fields y contenido tipado por etapa. Un score no prueba conformidad técnica.
Cada fuente identifica URL o documento/version/página, fecha de consulta y alcance de lo que respalda. Costos/uso/modelo no observables se guardan null/desconocidos.

## Reserva, reintentos y consistencia
- Seleccionar/reservar atómicamente con transacciones y actualización condicional compatibles con MySQL 5.7; no depender de SKIP LOCKED.
- Registrar lease_expires_at y token de reserva; renovación acotada si procede.
- Rechazar escrituras de una reserva vencida/reemplazada y resultados de versiones antiguas como actualización de la versión vigente. Pueden archivarse como historial explícito.
- Deducción inicial por proceso y etapa; tareas de revisión se crean explícitamente por cambio relevante. Restricción única sobre la clave lógica y el input correspondiente.
- Misma idempotency_key y payload: devolver resultado previo autorizado. Payload distinto: conflicto.
- Guardar cada unidad terminada; no confirmar un lote completo porque se recibieron algunas respuestas.
- Estado + auditoría + outbox en una transacción. Consumidores deduplican eventos.
- Fallos transitorios: reintentos limitados y backoff; permisos/datos inválidos: revisión. No prometer exactamente una vez entre servicios.
- Una ejecución diaria toma todos los pendientes elegibles por lotes, incluidos rezagados, sin filtrar solo por fecha de hoy.
- Cancelación/revocación bloquea nuevas operaciones; efectos externos ya enviados no se pueden deshacer.

## Estados
| Técnico | Visible |
| --- | --- |
| pending | Pendiente |
| running | En ejecución |
| waiting_supplier | En espera de proveedor |
| needs_review | Requiere revisión |
| completed | Completada |
| failed | Fallida |
| cancelled | Cancelada |

pending -> running con reserva; running -> completed/failed/needs_review/waiting_supplier según etapa. Fallo reintentable o reserva expirada puede volver a pending bajo límites. Respuesta de proveedor activa la siguiente tarea; no ocupa un worker esperando.
Estos estados no reemplazan los estados comerciales de Processes/Quotes. Revisado por usuario y Oferta presentada son hechos separados.

## Contrato de datos por etapa
- Score: clasificación, puntuación según escala versionada, razones, confianza/limitaciones, completitud. Pesos/umbrales por definir.
- Research: requisito solicitado, candidato exacto, evidencia y cumple/no cumple/no verificable por requisito. Precio con moneda, unidad/paquete, condición, MOQ, stock y fecha. Valores ausentes no son cero.
- RFQ: referencia, ítems/cantidades, contacto y fuente, contenido/version, autorización, correlación de mensajes. Confirmar que la dirección pertenece al proveedor correcto.
- Pricing: costos confirmados/estimados separados, tipo de cambio/fecha, comparables y supuestos, margen calculado por Go, rango sugerido y vigencia.
- Artifact: autoría fabricante/Castillo IT, documento/version/checksum, producto/ítem, fuentes y estado de disponibilidad.

## Conversión a cotización
Conservar creación básica determinista (cliente/entidad, expediente y source_process_id), operativa sin IA.
Acción adicional explícita del usuario: crear borrador e incorporar solo ítems, alternativas, cantidades, precios y adjuntos seleccionados; o incorporarlos a un borrador existente con control de versión. Previsualizar selección y validar mediante Quotes.
No sobrescribir ítems/precios editados por humanos. Idempotencia contra doble clic y restricciones de negocio; múltiples cotizaciones legítimas por proceso siguen permitidas.
Costos, proveedores, márgenes y notas son internos; no se exportan al cliente automáticamente.

## Aceptación al implementar
Duplicados concurrentes; caída tras guardar; lease vencido; resultado tardío; revocación; entrada parcial; incompatibilidad técnica; RFQ duplicada/ambigua; timeout SMTP; documentos no autorizados; cotización existente editada; límite de Work alcanzado conservando pendientes.


