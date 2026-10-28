# Variables globales del expediente

El usuario solicita crear las variables y asignar sus valores en la pestaña Variables, con alcance de expediente y no por archivo. Cada variable tiene identificador, etiqueta, tipo y valor opcional. Identificadores únicos en minúsculas, números y guion bajo; el primero es una letra. Tipos: texto, entero, decimal, fecha y sí/no. Los números se guardan como texto exacto; los valores no se interpretan como código.

Para la futura generación Typst se acuerda la sintaxis `#variables.nombre`, donde `nombre` es el identificador. La asociación de plantillas y el almacenamiento de variables no ejecutan Typst ni generan el PDF final: la inyección de datos y la compilación segura quedan pendientes hasta contar con el aislamiento documentado en la ficha del módulo.

La migración `5` añade un campo JSON nullable al expediente. Las variables globales empiezan vacías. Los valores históricos por formulario se conservan sin modificación y se muestran para consulta, incluso cuando hay distintos valores para el mismo identificador; el usuario decide qué valor crear en Variables. No se selecciona ni sobrescribe automáticamente ningún valor anterior.

Cada formulario existente evalúa sus campos declarados contra los valores globales con igual identificador y tipo. Los pendientes obligatorios se calculan usando esos valores; un tipo distinto no se aplica. El endpoint antiguo de valores por formulario mantiene compatibilidad mediante una escritura sobre las variables globales del expediente, con validación y protección de versión. La edición actual de interfaz se concentra en Variables.

`PUT /api/v1/licitaciones/{id}/variables` recibe exclusivamente `{ "variables": [{ "key", "label", "kind", "value" }] }`, con hasta 100 entradas, sesión, CSRF, permiso editar, expediente Borrador y `If-Match` fuerte. La actualización atómica reemplaza el conjunto completo, incrementa la versión y audita sólo el evento y la versión. Una versión obsoleta devuelve 412 y conserva los cambios en el navegador.
