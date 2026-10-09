# Presentación de resultados IA
Especificación de UX objetivo, 2026-10-07. Vive en Procesos y Cotizaciones; no agrega un módulo al menú. [Arquitectura](../architecture/ai-integration.md).

## Bandeja de oportunidades
Columnas: proceso/entidad, interés y puntuación, cierre, avance de ítems, precios disponibles/RFQ, fichas disponibles y responsable.
Filtros: recomendados, próximos a vencer, sin responsable, listos para revisión, esperando proveedores e información insuficiente.
El usuario puede revisar cualquier proceso, cambiar prioridad y solicitar investigación sin depender de un score alto. Información insuficiente no equivale a descartado. Mostrar última actualización y cierre en America/Tegucigalpa.

## Detalle del proceso
Encabezado: entidad, expediente, cierre, responsable y estado comercial.
Resumen IA: razones de interés, progreso y obstáculos, con fuentes y limitaciones; estado de investigación independiente del estado comercial.
Pestañas:
- Resumen: avances, bloqueos y siguientes acciones.
- Ítems y alternativas: productos y comparación técnica.
- Solicitudes a proveedores: preparadas, autorizadas, enviadas, respondidas, inciertas.
- Precio sugerido: compra, logística, costo puesto, margen, rango, comparables.
- Documentos: originales, fichas oficiales y propias, evidencia.
- Historial: ejecuciones, versiones y revisiones humanas.

Publicar progresivamente, sin esperar todas las RFQ. Polling inicial 15–30 s solo donde haga falta, pausado en pestañas ocultas; backoff ante error. Mostrar estados vacío, pendiente, parcial, fallido y sin permiso en español.

## Mesa por ítem
Mostrar solicitado/cantidad, alternativas, cumplimiento, compra/moneda, venta sugerida y pendientes.
Detalle por requisito: exigido, encontrado, fuente/fecha y cumple/no cumple/no verificable. Separar cumplimiento de stock, entrega y documentación.
Acciones: seleccionar alternativa, agregar producto/precio propio, solicitar otra búsqueda/RFQ, marcar revisado o descartar con motivo.
No confundir unidad con paquete ni precio publicado con costo puesto en Honduras. Identificar estimaciones y fechas de consulta. Precio recomendado provisional cuando faltan costos.

## Cotización
Crear cotización con los ítems seleccionados: previsualización y confirmación del usuario, referencia al proceso y transferencia controlada mediante Quotes.
Mantener también creación básica sin IA. Si existe cotización, ofrecer abrirla/incorporar selección, sin impedir múltiples borradores legítimos ni sobrescribir edición humana.
Adjuntos seleccionados; costos, márgenes, proveedores y notas permanecen internos salvo decisión explícita autorizada.

## Notificaciones y métricas
Alertas accionables: resultados para revisión, respuesta de proveedor, cierre próximo y bloqueo. Agrupar avances menores en resumen diario; destinatarios/canal según configuración autorizada.
Investigación completa, Revisado por usuario y Oferta presentada son estados independientes.
Medir conversión a cotizaciones/ofertas, tiempo de preparación, pendientes sin responsable y procesos vencidos sin ofertar con motivo. No contar una tarea IA completada como oferta presentada.
Configuración: skills/versiones activas, criterios pendientes de definir, conexiones, límites y salud de ejecuciones; nunca secretos visibles.


