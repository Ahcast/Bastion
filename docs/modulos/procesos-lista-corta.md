# Lista corta de licitaciones BID, JICA y UCP

Ampliación solicitada por el usuario el 6 de noviembre de 2026. La entrada Lista corta
pertenece a Procesos y reutiliza su activación y permiso `procesos/consulta/ver`.
Frontend React en Bluehost, API en AWS y entrega local de Scout.

El escáner publica `licitaciones/lista_corta.json` junto al `maestro.csv` configurado.
La instantánea atómica incluye versiones estables, primera detección, última
modificación, fecha de extracción de cada ficha, metadatos y evidencia de interés.
Una relectura idéntica conserva la versión. Un cambio del contenido vuelve a generar
una alerta. Durante un recorrido parcial se conservan las oportunidades anteriores
vigentes que aún no se han vuelto a observar. Un recorrido completo retira de la
vista las oportunidades que dejaron de aparecer. El historial de identidad permanece
en SQLite. El API también excluye plazos vencidos según la hora del servidor.

La pantalla muestra expediente, entidad, objeto, unidad de compra, lugar de recepción,
plazo, fuente y evidencia. Permite buscar, filtrar BID/JICA/UCP y mostrar solo fichas
pendientes de revisar. La fuente Japón produce un posible JICA pendiente de verificar.
UCP se interpreta como Unidad Coordinadora de Proyectos o Programas.

El encabezado de Bastion muestra el número de fichas nuevas o actualizadas. La consulta
se repite cada 60 segundos mientras la página es visible y al regresar a ella. Las
marcas de revisión se almacenan por usuario en el navegador y se sincronizan entre
pestañas del mismo origen. Los cambios de versión reabren la alerta. Esta entrega
implementa alertas dentro de Bastion con sesión abierta. Los avisos por correo y la
sincronización de revisiones entre dispositivos quedan pendientes de definir.

`GET /api/v1/procesos/lista-corta` aplica sesión, autorización, cuota y el semáforo
compartido de Procesos antes de abrir archivos. Solo lee una ruta relativa fija,
rechaza archivos mayores a 8 MiB y más de 5000 filas, valida enlaces de HonduCompras y
proyecta campos conocidos. Archivos ausentes o inválidos se muestran como tales. La
pantalla informa cobertura parcial y extracciones con más de dos horas de antigüedad.
El contrato HTTP está en `procesos.openapi.yaml`.

Para la instalación, publicar Scout y Bastion, verificar CI y respaldar las versiones
activas. Ejecutar `scout_licitaciones.py --output-dir /var/lib/bastion-scout/licitaciones`
con el usuario operativo de Scout y dar al grupo de Bastion lectura de ese directorio
y de su JSON. El servicio de extracción solo necesita escribir en su directorio.
Los ejemplos systemd del repositorio Scout proponen un recorrido por hora con un
presupuesto de 45 minutos. El temporizador se instala explícitamente en AWS.

La comprobación local usa las fichas reales obtenidas en la prueba anterior y cubre
versiones estables, cambios, conservación parcial, enlaces, vencimiento, permisos,
revisión y consulta periódica. Este documento describe la implementación preparada.
El despliegue y la activación del temporizador se verifican por separado.
