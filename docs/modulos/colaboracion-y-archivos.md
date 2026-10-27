# Módulo: Colaboración y archivos

## Propósito

Funcionar como el Drive corporativo: compartir archivos comunes de la empresa y organizar documentos ligados a personas.

Decisión del usuario del 2026-10-24: Colaboración tiene su propia carpeta lógica `colaboracion/` dentro del bucket privado S3 compartido del Core. Los archivos de empresa se guardan bajo `colaboracion/empresa/` y los de cada persona bajo `colaboracion/personas/{id}/`. No se crea un bucket separado; las carpetas de la interfaz son ámbitos de autorización de Colaboración y no rutas públicas ni permisos heredados de Personas.

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

Decisión del usuario del 2026-10-23: 50 MB por archivo, ofimática + imágenes + ZIP, bloqueo de ejecutables/scripts y validación del MIME real. Papelera con retención de 30 días, restauración, purga automática y borrado definitivo confirmado/auditado. Configuración en Ajustes > Colaboración, almacenada por el Core con alcance empresa. Límites y listas editables, conservando el bloqueo base. Ver [implementación](colaboracion-implementacion.md).

## API, datos y permisos

API actual: listar, subir, previsualizar, descargar y eliminar. Metadatos del archivo en manifiesto privado y archivo físico. Aplicar `ver` para listar/previsualizar/descargar, `editar` para subir o reemplazar y `eliminar` para eliminar/restaurar según la política de retención. Comprobar el alcance carpeta/empresa/persona en cada consulta, mutación y descarga.

## Pendientes

Antivirus y análisis profundo de contenido, cuotas corporativas totales, migración Legacy y comprobación visual autenticada. S3 privado está acordado globalmente: se reutiliza el bucket del Core, organizado por empresa/personas. No se heredan permisos de administración de Personas; cada carpeta comprueba el permiso propio de Colaboración.

## Implementación local

Crate `bastion-colaboracion`, API versionada, pantalla React y sección editable de Ajustes. Permisos `ver`, `editar` y `eliminar`, con alcances `empresa`, `persona:{id}`, `settings` o `*`. Restaurar usa `eliminar`, conforme a esta ficha. Disponible sin activar después de aplicar su migración y allowlist. MCP continúa sin exponerse hasta completar el contrato de seguridad común. Ver [OpenAPI](colaboracion.openapi.yaml) y [estado verificable](colaboracion-implementacion.md).

## Extensión futura para Licitaciones

Actualización de implementación: Colaboración incorpora biblioteca vacía de **esquemas de captura**, versiones inmutables, hashes y publicación exclusiva del Administrador, mediante migración `202610270001`. El puerto propietario entrega una versión autorizada a Licitaciones. El usuario añade formularios cuando los necesite. No equivale a cargar/ejecutar plantillas Typst ni a disponer de copias binarias o ensamblado PDF; esas partes del diseño siguiente siguen pendientes. Ver [infraestructura](licitaciones-infraestructura.md).

La [ficha de Licitaciones](licitaciones.md) propone usar Colaboración como biblioteca de originales y futuras plantillas/definiciones reutilizables. La implementación actual no ofrece versiones funcionales, copias por expediente, aprobación de plantillas ni catálogo de variables, y su validador no admite `.typ`. Esas capacidades requieren contratos y migraciones propios antes de implementarse. No cambia la política vigente de archivos/papelera ni sus permisos. Las copias independientes deben tener propietario/retención explícitos y sobrevivir al borrado del original. El usuario confirma el 2026-10-27 que las copias solo son visibles dentro del expediente, no como archivos de Colaboración. Los formularios/formatos se incorporarán individualmente; el Administrador del sistema aprueba/publica las plantillas y el usuario solo cambia valores de variables de su expediente. El permiso genérico de edición de archivos no concede aprobación/publicación de plantillas.
