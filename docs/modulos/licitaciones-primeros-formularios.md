# Primer grupo de formularios de Licitaciones

Decisiones confirmadas por el usuario a partir de los archivos de `bastion formularios`, el 2026-10-28. Este documento define alcance; no acredita implementación ni despliegue.

## Variables del expediente

Los datos se capturan en Variables y se validan antes de entregarlos a Typst. `lpn`, `inst` y `descripcion` son texto; `dia`, `year` y `lotes` son enteros; `mes` se selecciona entre los meses del año; `stylo` se selecciona entre `bid` y `std`. `total_paginas` se calcula a partir de todas las páginas del PDF final, incluidos los PDF e imágenes adjuntos. No es editable como valor manual.

El archivo `config.typ` de origen define `datos_base`; los documentos recibidos consumen `#datos_base.lpn` y campos análogos. `variables.typ` incorpora los totales de lotes y `lpn_total`. La sintaxis de estos formularios debe mantenerse compatible con sus imports y no confundirse con la sintaxis general `#variables.nombre` acordada para plantillas futuras. Los textos se entregan como datos y no como código.

## Lotes y oferta económica

`lotes` es el número total de lotes de la licitación. El usuario selecciona el lote que desea preparar; los lotes sin ítems no se incorporan al PDF final. Cada lote comienza vacío. Las listas de productos de los archivos entregados son datos de prueba, no se precargan.

Para cada lote se generan `items{n}_{stylo}.typ` y `lote{n}-{stylo}.typ`. El número de ítem se genera por orden y se vuelve a enumerar al eliminar. Ambos estilos contienen descripción, fecha de entrega, cantidad decimal, unidad y precio unitario con dos decimales. BID añade país de origen y transporte con dos decimales. Se requiere validación exacta de montos y cantidades sin usar números binarios flotantes.

Según las plantillas y confirmación del usuario, el subtotal es cantidad por precio unitario, el ISV es 15 % del subtotal y en BID el transporte de la línea se suma después del impuesto. Se conserva el redondeo de las plantillas: el total de lote es la suma de los totales calculados, redondeada a dos decimales. `variables.typ` suma los totales de los lotes. Se requiere corroborar que los totales mostrados en pantalla coincidan con los documentos generados.

Al cambiar de BID a STD o reducir los lotes se identifica la información que se descartaría y se exige confirmación antes de descartar esos datos. En otro caso se preservan los cambios. El control de versión del expediente debe proteger contra ediciones concurrentes.

## Selección, orden y composición

Para estilo BID se usa `carta-de-oferta.typ`; para STD, `presentacion-de-oferta.typ`. `dj-integridad.typ` e `informacion_sobre_el_oferente.typ` se seleccionan por expediente. Los datos fijos de empresa y representante se conservan tal como figuran en las plantillas aportadas. Los documentos de este grupo y los anexos PDF o imágenes seleccionados se pueden ordenar en el módulo mediante arrastrar y soltar; el orden elegido gobierna la composición final. `main.typ` está vacío y no es necesario como recurso del usuario.

Las fuentes utilizan `@preview/oxifmt:1.0.0` y `@preview/num2words:0.2.0`; las dependencias deben estar disponibles de forma controlada en el entorno de generación. Debe preservarse el límite acordado del PDF final, la autorización de fuentes referenciadas, el conteo real de páginas y el foliado. No ejecutar código de plantillas aportadas sin el aislamiento y límites de recursos previstos en `licitaciones.md`.

## Verificación local de compatibilidad

El 2026-10-28 se probaron los formularios aportados con datos ficticios y Typst 0.15.1: carta BID, presentación STD, lote STD y los dos formularios legales compilaron. El adaptador prepara `config.json` y los archivos de ítems como datos, convierte cantidades y montos a `decimal` y construye los imports solo para lotes ofertados. Esta prueba aislada no acredita todavía la integración de los formularios ni la composición del PDF final.

Se registraron las siguientes huellas SHA-256 de los paquetes descargados desde `packages.typst.org`: `num2words-0.2.0.tar.gz` `101c973cc233a3bcd9d97f8351b1f4900f33b930efb7e4a2375b8839b8c7018a`, `oxifmt-1.0.0.tar.gz` `7d17a1fc8ad01740ec3cb2b03c7360a4225ff9318e5710765fa98ea6fd59594f`. Antes de su uso en producción deben almacenarse y verificarse como dependencias fijadas, sin permitir descargas del compilador. En la prueba local faltaron las fuentes Carlito y Liberation Serif; se deben fijar y verificar también en el entorno final para mantener el aspecto de los originales.

## Ubicación del folio

El 2026-10-28 el usuario confirmó que el folio se coloca sobre el margen existente, sin añadir una franja externa. Si el contenido ocupa ese margen, se coloca el número de todos modos, incluso si puede superponerse al contenido. Esta decisión sustituye la recomendación anterior de garantizar que el folio nunca cubra contenido.

## Firmas y prueba de composición

El usuario autorizó incluir PDF con firma digital aunque la firma no se conserve al unirlos. El archivo original de Colaboración sigue siendo una referencia; las copias previamente existentes se mantienen. El 2026-10-28 se comprobó visualmente una unión y foliado de prueba con QPDF 12.4.2 en páginas verticales, horizontales y rotadas. Esta prueba es exploratoria: aún no integra ni despliega un servicio de composición. El archivo oficial Linux `qpdf-12.4.2-bin-linux-x86_64.zip` registra SHA-256 `db367d897829f22c4198ce1094143c9d467bd6ee7dfabc44ba6f02056b24f8b1`; la selección final del compositor requiere prueba de aislamiento y límites.

## Proceso aislado de composición

La rama de desarrollo incluye un ejecutable de composición para un único trabajo; recibe únicamente la cantidad de piezas y toma las rutas de una configuración del servicio. Rechaza ejecutarse como root o compartiendo el espacio de red de PID 1. Estas comprobaciones no bastan por sí solas: antes de conectarlo a la API o desplegarlo deben configurarse una identidad sin credenciales, un directorio privado por trabajo accesible únicamente a ese trabajo, aislamiento de red y límites de CPU, memoria, disco y duración. El servicio también debe verificar versiones y permisos de los originales antes de entregar el resultado y limpiar archivos temporales. La creación del servicio, el transporte desde la API y la activación en AWS siguen pendientes.
