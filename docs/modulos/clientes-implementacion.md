# Clientes: primera versión

## Alcance y fuente

Revisión estática del 2026-10-22 sobre la copia de CastilloIT en Downloads. El `index.html` activo carga `main-4ZA7RVJ2.js`; la ruta Clientes carga `chunk-L24TMX5W.js`. Se cotejaron formulario, `back/app/Http/Controllers/ClientesController.php` y `back/app/Models/Clientes.php`. No se ejecutó Laravel ni se leyeron credenciales o registros de clientes del Legacy.

La entrega incorpora `bastion-clientes`, migración MySQL 5.7 propia, servicio de dominio, API y pantalla React de carga diferida con los siete campos observados, Buscar, Nuevo, Editar, Guardar y paginación. Guardar limpia el formulario y recarga el listado. El catálogo para selectores es un endpoint paginado; la integración visual con editores de otros módulos corresponde a sus entregas. No añade borrado, archivo ni importación automática.

El usuario confirmó publicar sin ajustes editables: Ajustes > Clientes explica el estado pendiente. Registro `clientes`, versión 0.1.0, migración `_clientes_migrations/1`. Depende solo de Core y MySQL. Requiere inclusión, `BASTION_ALLOWED_MODULES=...clientes`, migración válida y activación administrativa desde Ajustes > Core > Módulos. La instalación lo deja desactivado por defecto y conserva los módulos existentes.

## Datos y paridad

- Nombre no vacío tras recorte; máximo 255. Opcionales null/omitidos se guardan vacíos y se recortan; límites: contacto 255, correo 50, RTN 30, teléfonos 20, dirección 1500. Se valida correo si se informa; no se añade un patrón tributario ni telefónico.
- Alta busca primero nombre canónico y después dígitos ASCII del RTN, con menor ID como desempate. Una coincidencia actualiza el nombre y solo completa opcionales vacíos; nunca sustituye RTN o correo existentes. No elimina ni une físicamente otro registro.
- Editar reemplaza los siete campos; permite vaciar opcionales y corregir RTN, como Legacy. No se impone unicidad nueva al editar. Nombres duplicados se representan en listado/catálogo por correo, RTN y menor ID. La búsqueda nombre/correo/RTN se aplica antes de elegir representante, igual al controlador. Un ID conocido sigue siendo consultable con permiso.
- Nombre canónico: minúsculas, descomposición Unicode, eliminación de diacríticos, equivalencias latinas y espacios colapsados. Probado para español; la equivalencia completa con `iconv ASCII//TRANSLIT` para otros alfabetos queda pendiente. Se conserva texto de presentación y RTN original.
- `clientes_records` es un catálogo nuevo de Bastion. No se migra `clients` del Legacy; una migración requiere esquema, reconciliación de identificadores y relaciones.

## Seguridad, consistencia y límites

El alcance es el catálogo compartido de la instalación, conforme al Core actual de una empresa. No se aceptan empresa/actor del cliente ni se afirma soporte multiempresa. `clientes/catalogo/ver` autoriza lista, detalle y catálogo; `editar` autoriza alta/edición. `eliminar` no habilita rutas inexistentes. Los roles nuevos empiezan sin acceso. El Administrador tiene acceso implícito solo si el módulo está habilitado.

El servicio verifica permisos y activación. Un bloqueo transaccional del registro de módulo serializa merges, edición y desactivación. Alta exige `Idempotency-Key` por actor, 16–128 caracteres ASCII alfanuméricos, guion o guion bajo; mismo contenido normalizado devuelve recibo, contenido diferente devuelve 409. Recibos persistentes, retiro pendiente de definición. Ediciones posteriores no cambian respuestas históricas de recibos. Edición exige `If-Match` fuerte; versión obsoleta devuelve 412 y la UI conserva el borrador, bloquea reenvío y ofrece recargar con confirmación de descarte si hay cambios. La navegación protege borradores y bloquea salida durante guardado.

SQL parametrizado, búsqueda literal de hasta 200 caracteres, páginas 1–10000, 1–100 filas (25 por defecto), orden determinista y `has_more`. Catálogo paginado con solo ID, nombre, RTN y correo. Cuerpo máximo 16 KiB, 120 solicitudes/minuto por actor y timeout HTTP de 30 segundos. Sesiones, CSRF, origen y cabeceras del Core; los errores de extractores Axum siguen pendientes de unificación transversal, los errores de dominio usan RFC 9457.

Eventos transaccionales `client.create`, `client.merge`, `client.update` con actor, ID y versión, sin copiar contacto/RTN a auditoría. Desactivar conserva datos y recibos. MCP queda cerrado hasta implementar OAuth/consentimiento del Core; el servicio queda preparado para reutilización, sin acceso directo de agentes a MySQL.

## Validación y publicación

Unitarias de normalización, límites y asignación masiva. CI con MySQL 5.7 aislado: migraciones repetibles, permisos, duplicados, prioridad nombre/RTN, concurrencia/idempotencia, versiones, paginación, búsqueda literal, catálogo, bitácora sin datos de contacto, cuota, desactivación/allowlist/checksum. Frontend: navegación/acceso, payload de siete campos, If-Match, conservación del formulario ante 412 y paginación.

Se publica primero en GitHub. Tras CI, se verifican artifacts/SHA-256 del mismo commit y respaldos privados antes de instalar frontend en Bluehost y backend en AWS. La evidencia final se registra después de verificar; preparar artifacts no confirma despliegue. Este corte no cierra paridad visual Legacy, migración de clientes, settings futuros ni pendientes generales del Core.

## Publicación verificada — 2026-10-22

Código publicado directamente en `main`: [`c86e8142763bea59adcad7dfb442daa4e96a3526`](https://github.com/Ahcast/Bastion/commit/c86e8142763bea59adcad7dfb442daa4e96a3526). [Core checks 39872647000](https://github.com/Ahcast/Bastion/actions/runs/39872647000) terminó correctamente: formato, compilación, 46 pruebas Rust (incluidas las integraciones reales con MySQL 5.7 y HTTP) y 42 pruebas frontend, tipos, build y auditoría de dependencias. El fallo inicial de compilación por firmas antiguas de QueryBuilder se corrigió antes de desplegar.

Backend instalado en AWS Lightsail y frontend en Bluehost desde los artifacts de ese mismo commit, con hashes ZIP/tar/archivos verificados. Se creó respaldo privado de la base y del frontend y se conservó el release anterior para reversión. La migración `clientes/1` quedó aplicada. Se comprobaron sin cambios los 835 procesos, las marcas/importaciones, cotizaciones, usuarios, personas, roles y permisos existentes. No se importaron clientes del Legacy ni se añadieron clientes sintéticos a la base desplegada.

Clientes queda incluido y permitido en el ambiente, **desactivado por defecto**. El Administrador puede activarlo desde Ajustes > Core > Módulos; Procesos y Cotizaciones conservaron su estado. Los ajustes propios siguen sin campos editables conforme a la confirmación del usuario.

HTTPS: `/health/live` y `/health/ready` devolvieron 200; Clientes y Procesos sin sesión devolvieron 401 con `application/problem+json`. Se verificaron los 31 archivos instalados y los hashes de seis recursos públicos críticos, incluido el chunk Clientes. La pantalla pública de acceso cargó sin errores de consola. En navegador local con datos exclusivamente sintéticos se comprobaron alta, búsqueda, edición, protección de borrador, Ajustes y ausencia de desbordamiento horizontal a 390 px. Las operaciones autenticadas del release fueron verificadas en CI con sesiones sintéticas aisladas; la activación y prueba operativa con el usuario real siguen a cargo del Administrador.
