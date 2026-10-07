# RAG de precios — S3 y MySQL
Diseño objetivo actualizado el 7 de octubre de 2026. [Pipeline IA](../architecture/ai-integration.md).
El objetivo funcional es recuperar antecedentes para una skill de recomendación de precio competitivo. El usuario decide el precio final. No se presume equivalencia técnica por similitud.

## Primera etapa
- S3 privado: cotizaciones, adjudicaciones, catálogos, fichas, originales y texto extraído por versión.
- MySQL: metadatos, referencias a fragmentos, productos/partes, categorías, fechas, monedas, unidades, precios y condiciones verificadas.
- Laravel: filtros y búsqueda autorizada; devuelve datos/fragmentos mediante MCP.
- ChatGPT Work: consume solo antecedentes pertinentes, aplica skill y registra recomendación.
No descargar todo el histórico diariamente. Un corpus pequeño puede consultarse sin base vectorial ni servidor RAG dedicado. Las capacidades de recuperación exacta/textual se medirán con casos reales.

## Preparación y consulta
1. Registrar documento, permisos, checksum, versión y procedencia.
2. Extraer texto una vez por versión; OCR si es escaneado. Ejecutor por capacidad; no presumir OCR pesado en Bluehost/Lightsail 1 GB.
3. Validar extracción, normalizar moneda/unidad/fecha/condiciones y conservar vínculo al original.
4. Indexar en MySQL con campos e índices compatibles con 5.7.
5. Buscar desde search_pricing_history, aplicar ACL antes de entregar fragmentos y citar documento/version/página.
6. Registrar comparables usados y limitaciones; actualizar/eliminar proyecciones obsoletas cuando cambie fuente o permiso.

La skill compara modelo, cantidad, antigüedad, condición, impuestos y entrega. Laravel calcula costos y márgenes con decimales. Sin comparables suficientes, indicar recomendación basada en costo/margen; una adjudicación pasada no garantiza competitividad actual.
No usar recomendaciones generadas por IA como precios de mercado confirmados; separar antecedentes verificados, propuestas y ventas/adjudicaciones efectivas.

## Ampliación opcional
S3 Vectors puede almacenar índices vectoriales y consultar similitud; no es lo mismo que guardar PDFs en un bucket normal.
Requiere generar embeddings de fragmentos y consultas con un modelo/versión compatible, mantener procedencia, filtros de acceso y reindexar si cambia el modelo. No asumir que embeddings o consumo AWS estén incluidos en Plus.
No se fija proveedor/modelo, presupuesto ni se crea infraestructura en este cambio. Decidir cuando la búsqueda inicial sea insuficiente.

## Pendientes
Corpus disponible, calidad de extracción, tamaño, responsables de validación, retención, OCR, límites de resultados y evaluación de recuperación. Sin RAG disponible, Bastion conserva investigación/cotización manual y muestra la limitación de pricing.

Fuentes: [S3 Vectors](https://docs.aws.amazon.com/AmazonS3/latest/userguide/s3-vectors.html), [consultas](https://docs.aws.amazon.com/AmazonS3/latest/userguide/s3-vectors-query.html).
