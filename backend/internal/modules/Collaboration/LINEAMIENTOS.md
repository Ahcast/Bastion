# Módulo Colaboración

## Objetivo

Preservar la biblioteca por empresa/persona, con subida, preview, descarga, filtros y borrado lógico, migrando bytes a S3, metadata a MySQL y agregando autorización real.

## Funcionalidad legacy a conservar

- carpeta de empresa;
- carpetas por persona;
- conteo de archivos;
- filtros/búsqueda;
- subida;
- título/notas/metadata;
- preview;
- descarga;
- tamaño visible;
- borrado lógico;
- identificación visual del scope.

## Debilidades legacy a eliminar

- “carpeta persona” solo visual;
- acceso por ID sin ownership/policy suficiente;
- manifest JSON;
- binarios en filesystem backend;
- MIME/extensiones demasiado permisivos;
- preview inline de contenido activo;
- read-modify-write sin transacción.

## Modelo objetivo

```text
documents
document_links
collaboration_entries
```

### collaboration_entries

```text
id
document_id
scope        // company | person
person_id nullable
title
notes nullable
uploaded_by
created_at
updated_at
deleted_at
```

Bytes en S3.

## Upload seguro

```text
Angular
 -> Go validation
 -> S3 privado
 -> checksum
 -> metadata MySQL
 -> opcional malware scan
 -> available
```

Allowlist inicial configurable:

```text
pdf
doc/docx
xls/xlsx
csv
txt
jpg/jpeg/png
```

Contenido activo riesgoso no se sirve inline.

## Preview y descarga

- endpoint autorizado o URL firmada corta;
- `Content-Disposition` seguro;
- HTML/SVG y otros activos se aíslan o descargan;
- nunca exponer `storage_key` directamente.

## Autorización

Scope empresa:

```text
colaboracion.view
```

Scope persona:

- admin autorizado; o
- persona propietaria; o
- permiso explícito.

Conocer el ID no otorga acceso.

## API objetivo

```http
GET    /api/v1/collaboration
POST   /api/v1/collaboration
GET    /api/v1/collaboration/{id}
DELETE /api/v1/collaboration/{id}

GET    /api/v1/collaboration/{id}/preview
GET    /api/v1/collaboration/{id}/download
```

Filtros:

```text
scope
person_id
search
page
per_page
```

## Conexión documental

```mermaid
flowchart LR
    C[Colaboración] -.-> P[Procesos]
    C -.-> Q[Cotizaciones]
    C -.-> O[Órdenes]
    C -.-> F[Facturas]
    C -.-> CL[Clientes]
    C -.-> S[Proveedores]
    AI[AI Gateway] -. resumen / extracción .-> C
```

La infraestructura documental es común, pero los permisos de cada módulo se mantienen separados.

## Integración de IA

Tools:

```text
list_authorized_documents()
get_document_metadata()
summarize_document()
extract_document_fields()
classify_document()
find_related_documents()
```

### Autorización antes del modelo

La ACL se evalúa **antes** de cargar contenido para IA:

```text
actor
  -> Policy / document ACL
  -> Document Service
  -> AI Gateway
```

Un documento al que el usuario no tiene acceso tampoco puede ser leído mediante un asistente.

### Contenido no confiable

Todo documento se trata como:

```text
UNTRUSTED_EXTERNAL_CONTENT
```

Por tanto:

- instrucciones dentro del PDF/Word/Excel no cambian permisos;
- un documento no puede ordenar que la IA envíe correo, elimine datos o ejecute tools;
- tools solicitadas por el modelo se revalidan en Go;
- credenciales, secretos y paths internos no se incluyen en prompts.

### Casos de uso

- resumir documento;
- extraer campos estructurados;
- clasificar documento;
- preparar metadata sugerida;
- localizar documentos relacionados por entidad/metadatos;
- producir insumos para Procesos, Cotizaciones, Órdenes, Facturas y Gastos.

### OCR / parsing

La capa de documentos puede generar una representación textual/estructurada para IA. El binario original permanece en S3 y su hash no cambia.

Los resultados derivados se pueden guardar como artifact o metadata con:

```text
document_id
sha256
ai_run_id
extractor_version
prompt_version
result_json
created_at
```

### RAG futuro

Colaboración y `documents` aportan fuentes autorizadas al RAG inicial S3/MySQL para pricing; índices y fragmentos respetan las ACL del documento fuente. La búsqueda vectorial es opcional posterior.

Una búsqueda vectorial no otorga acceso adicional.

### Acciones sensibles

La IA puede proponer título, categoría, tags o vínculos. Borrar, mover entre scopes sensibles o compartir un documento requiere Domain Action y Policy.

## Seguridad

```text
colaboracion.view
colaboracion.upload
colaboracion.download
colaboracion.delete
```

Descargas sensibles pueden auditarse.

## Criterios de aceptación

- Carpetas empresa/persona siguen en UX.
- Upload/preview/download continúan.
- Borrado sigue siendo lógico.
- Metadata deja JSON.
- Bytes dejan webroot.
- Scope persona pasa de visual a autorización real.
- IA nunca puede leer un documento sin autorización del actor.
- Contenido documental no puede modificar políticas de autorización mediante prompt injection.
- Resultados IA quedan trazables contra documento/hash analizado.


Las capacidades OCR/extracción descritas requieren ejecutor externo por definir; no se ejecutan en Lightsail.
