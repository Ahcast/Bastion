# Variables globales en Typst

Las variables se introducen una sola vez en la pestaña Variables del expediente. En un formulario aprobado se usan como `#variables.nombre`, reemplazando `nombre` por el identificador de la variable. La preparación incorpora el import necesario automáticamente y entrega los valores por JSON, sin convertir el texto introducido en código Typst.

El número de páginas se expone como `#variables.total_paginas` con el valor calculado para el PDF final, incluso si existe un valor histórico con ese identificador. Se conservan los tipos booleano, entero y decimal; los números fuera del rango exacto de Typst se rechazan antes de generar el documento. Los formularios del primer grupo conservan su interfaz `datos_base` y reciben los mismos datos del expediente.

Esta preparación no ejecuta por sí misma los formularios: todavía falta conectarla al servicio aislado y al armado del PDF final.

El trabajador acepta una lista de fuentes aprobadas que requieren variables globales. Inserta el import fijo en una copia temporal por bloques y conserva intacta la fuente original; la autorizacion de la fuente y el aislamiento del trabajo deben verificarse antes de invocarlo.
