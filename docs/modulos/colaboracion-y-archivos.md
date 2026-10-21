# Módulo: Colaboración y archivos

## Propósito

Compartir archivos comunes de empresa y organizar documentos ligados a personas.

## Campos y filtros

| Campo | Nombre | Requerido | Función |
|---|---|---:|---|
| Tipo/alcance | `scope` | Sí | Empresa o persona. |
| Persona | `id_persona` | Si scope persona | Asigna propietario/carpeta. |
| Título visible | `title` | Sí | Etiqueta legible. |
| Archivo | `file` | Sí | Documento cargado. |
| Notas | `notes` | No | Contexto. |
| Buscar archivo | query | No | Busca título, nombre original o nota. |
| Tipo/persona | filtros | No | Filtra lista. |

## Botones y acciones

Actualizar; Empresa/Personas (abrir carpetas); Filtrar; Limpiar; seleccionar archivo; Guardar/subir (requiere archivo y persona para scope personal); Ver (preview); Descargar; Papelera/eliminar; paginación.

## Settings bajo Ajustes

No hay settings del módulo confirmados. Sección en Ajustes: sí. Retención, tamaños/tipos y categorías requieren decisión explícita; no presumirlos.

## API, datos y permisos

API actual: listar, subir, previsualizar, descargar y eliminar. Metadatos del archivo en manifiesto privado y archivo físico. Aplicar `ver` para listar/previsualizar/descargar, `editar` para subir o reemplazar y `eliminar` para eliminar/restaurar según la política de retención. Comprobar el alcance carpeta/empresa/persona en cada consulta, mutación y descarga.

## Pendientes

Ubicación física definitiva (Bluehost/AWS), límites de tamaño/tipo, antivirus, retención, versionado, papelera/restauración y permisos heredados de persona.
