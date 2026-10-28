# Variables globales del expediente

El usuario solicita crear las variables y asignar sus valores en la pestaña Variables, con alcance de expediente y no por archivo. Cada variable tiene identificador, etiqueta, tipo y valor opcional. Identificadores únicos en minúsculas, números y guion bajo; el primero es una letra. Tipos: texto, entero, decimal, fecha y sí/no. Los números se guardan como texto exacto; los valores no se interpretan como código.

Para la futura generación Typst se acuerda la sintaxis `#variables.nombre`, donde `nombre` es el identificador. La asociación de plantillas y el almacenamiento de variables no ejecutan Typst ni generan el PDF final: la inyección de datos y la compilación segura quedan pendientes hasta contar con el aislamiento documentado en la ficha del módulo.

La migración `5` añade un campo JSON nullable al expediente. Las variables globales empiezan vacías. Los valores históricos por formulario se conservan sin modificación y se muestran para consulta, incluso cuando hay distintos valores para el mismo identificador; el usuario decide qué valor crear en Variables. No se selecciona ni sobrescribe automáticamente ningún valor anterior.

Cada formulario existente evalúa sus campos declarados contra los valores globales con igual identificador y tipo. Los pendientes obligatorios se calculan usando esos valores; un tipo distinto no se aplica. El endpoint antiguo de valores por formulario mantiene compatibilidad mediante una escritura sobre las variables globales del expediente, con validación y protección de versión. La edición actual de interfaz se concentra en Variables.

`PUT /api/v1/licitaciones/{id}/variables` recibe exclusivamente `{ "variables": [{ "key", "label", "kind", "value" }] }`, con hasta 100 entradas, sesión, CSRF, permiso editar, expediente Borrador y `If-Match` fuerte. La actualización atómica reemplaza el conjunto completo, incrementa la versión y audita sólo el evento y la versión. Una versión obsoleta devuelve 412 y conserva los cambios en el navegador.

## Publicación verificada del 2026-10-28

Backend AWS y frontend Bluehost ejecutan `8e5de99882db82b4852014f165c7096067bf4f20`, commit de integración cuya versión y árbol coinciden con los artifacts de [CI aprobado](https://github.com/Ahcast/Bastion/actions/runs/40689251518). El CI verificó compilación, formato, 137 pruebas de frontend y pruebas MySQL 5.7 de variables compartidas, aislamiento, permisos, versión obsoleta y conservación de valores históricos.

Antes de la migración 5 se guardó un respaldo privado de la base y se ensayó su restauración en 84 tablas temporales, comparando sus filas. Se conservaron todas las tablas y los valores por formulario; la columna nueva comenzó en NULL para expedientes existentes. Se verificaron configuración, estados de módulos y readiness 200. Las credenciales de migración no quedaron en el proceso del servicio.

En Bluehost se preservaron la configuración y los assets anteriores; se verificaron 47 archivos locales y 45 hashes públicos. Las rutas SPA respondieron 200 y la nueva escritura de variables sin sesión fue rechazada con 403. La inspección visual y la carga de valores con sesión autenticada en producción siguen pendientes; las pruebas de interfaz y API se realizaron en CI con datos sintéticos. La conexión de estas variables a la generación Typst y al PDF final continúa pendiente.
