# Licitaciones: infraestructura inicial

## Decisión de alcance del 2026-10-27

El usuario solicita desarrollar la infraestructura y rechaza campos iniciales de expediente, lotes e ítems. Incorporará formularios y archivos según se necesiten. No se crean formularios precargados, campos de negocio, reglas económicas, monedas ni transiciones de envío/adjudicación. El expediente inicial se identifica por su ID técnico y permanece en Borrador.

Se implementan expediente vacío, biblioteca de esquemas de formularios en Colaboración, publicación exclusiva del Administrador y copia independiente de una versión publicada al expediente. Publicar aquí aprueba **el esquema de captura**, no código Typst ni una plantilla PDF. No se admite código ejecutable. El catálogo nace vacío. Tipos técnicos iniciales: texto, entero, decimal exacto como cadena, fecha ISO y booleano; cada recurso define sus nombres, etiquetas y obligatoriedad al incorporarse. No hay fórmulas, impuestos ni cálculo de dinero implícitos.

Cada formulario declara su bloque legal, económico o técnico. Las copias conservan esquema, hash y versión de origen. Una revisión posterior de biblioteca no cambia copias previas. El usuario sólo modifica los valores declarados y puede guardar incompletos; la validación informa faltantes. El cambio del esquema dentro de una copia no está permitido.

## Contrato y seguridad

Contrato HTTP: [OpenAPI](licitaciones.openapi.yaml). ID del módulo: `licitaciones`; permisos comunes `ver`, `editar`, `eliminar` sobre `expedientes`. Esta primera infraestructura usa un ámbito compartido de empresa: los usuarios con ese permiso acceden a los expedientes de la empresa; no existe una ACL por persona inventada. Se comprueba la pertenencia de cada copia a su expediente. Publicación requiere Administrador del sistema, incluso si un rol tiene `editar` o `*` en ambos módulos. Biblioteca usa lectura de `colaboracion/empresa`; copiar requiere además editar el expediente y Colaboración activo. Las copias existentes se leen con Licitaciones aunque se desactive Colaboración.

Toda mutación usa sesión, origen exacto y CSRF comunes. Altas, publicaciones y copias requieren `Idempotency-Key` de 16–128 caracteres ASCII alfanuméricos, guion o subrayado, con recibos persistentes. Copiar, guardar valores y eliminar requieren `If-Match` fuerte; versión obsoleta: 412. El reintento idéntico de una copia recupera el recibo antes de comprobar la versión; no duplica efectos. Otro cuerpo para la misma clave: 409. Eliminación lógica, confirmada con ID, sólo en Borrador; no existe endpoint que fuerce estado. Los bloqueos serializan edición/eliminación/desactivación. Auditoría guarda identificadores y revisiones, sin valores privados.

Límites técnicos de metadatos: 256 KiB por solicitud, 100 variables por esquema, texto de hasta 4.000 caracteres por valor, 100 copias por expediente, páginas de 25 por defecto/100 máximo, 120 solicitudes por actor/minuto, timeout de 30 segundos. Son presupuestos de infraestructura; no límites funcionales de páginas PDF. Entradas binarias conservan el contrato acordado de 20.000.000 bytes; expediente y PDF final tienen presupuestos separados de 500.000.000 bytes. El validador puro comprueba estos límites, pero el transporte binario, las revisiones y S3 aún requieren implementación y mediciones; no se declara soporte operativo por haber definido constantes.

## Entrega y pendientes

Crate y migraciones propias MySQL 5.7, registro central, allowlist, API y frontend React bajo demanda. Inicialmente deshabilitado y sin cambios de configuración en los hosts. Las migraciones sólo se ejecutan mediante `migrate`; servir no ejecuta DDL. Colaboración incorpora migración adicional para esquemas publicados y recibos; revisar esa migración y respaldar antes de desplegar el artefacto común.

Quedan pendientes archivos originales y copias binarias recuperables MySQL/S3, lotes/ítems sin campos prefijados, paquetes Typst aprobados, fórmulas por formato real, número a letras, ensamblado/índice/foliado, historial PDF, envío/adjudicación y contrato con Facturas. Se muestran como pendientes, sin botones que aparenten estar operativos. Ajustes muestra alcance y límites; no tiene settings editables inventados. MCP sigue pospuesto. Esta entrega es infraestructura inicial, no paridad ni módulo completo ni despliegue.
