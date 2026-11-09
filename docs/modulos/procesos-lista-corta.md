# Licitación en Procesos

Procesos ofrece dos pestañas, Compra menor y Licitación. Licitación reutiliza la
activación de Procesos y el permiso `procesos/consulta/ver`.
Frontend React en Bluehost, API en AWS y entrega local de Scout.

El escáner publica `licitaciones/lista_corta.json` junto al `maestro.csv` configurado.
La instantánea atómica, en versión 2, incluye todas las licitaciones vigentes del
escáner, versiones estables, primera detección, última
modificación, fecha de extracción de cada ficha, metadatos y evidencia de interés.
Una relectura idéntica conserva la versión. Un cambio del contenido vuelve a generar
una alerta. Durante un recorrido parcial se conservan las oportunidades anteriores
vigentes que aún no se han vuelto a observar. Un recorrido completo retira de la
vista las oportunidades que dejaron de aparecer. El historial de identidad permanece
en SQLite. El API también excluye plazos vencidos según la hora del servidor.

La pantalla muestra expediente, entidad, objeto, unidad de compra, lugar de recepción,
plazo, fuente y evidencia. Permite buscar, filtrar BID/JICA/UCP y mostrar solo fichas
pendientes de revisar. El orden prioriza JICA, BID y UCP, seguidos de posibles JICA
y del resto, con el próximo cierre primero dentro de cada grupo.
La fuente Japón produce un posible JICA pendiente de verificar.
UCP se interpreta como Unidad Coordinadora de Proyectos o Programas.

La pestaña Licitación muestra el número de coincidencias nuevas o actualizadas por
revisar. Después de abrirla, Procesos consulta cada minuto mientras está visible. Las
marcas de revisión se almacenan por usuario en el navegador y se sincronizan entre
pestañas del mismo origen. Los cambios de versión reabren la marca visual.

`GET /api/v1/procesos/licitaciones` aplica sesión, autorización, cuota y el semáforo
compartido de Procesos antes de abrir archivos. Solo lee una ruta relativa fija,
rechaza archivos mayores a 8 MiB y más de 5000 filas, valida enlaces de HonduCompras y
proyecta campos conocidos. Archivos ausentes o inválidos se muestran como tales. La
pantalla informa cobertura parcial y extracciones con más de dos horas de antigüedad.
El contrato HTTP está en `procesos.openapi.yaml`.

El trabajador del servidor consulta la entrega cada minuto, incluso con Bastion
cerrado. Solo encola coincidencias explícitas JICA, BID o UCP, vigentes y detectadas
desde la fecha de activación. Los posibles JICA quedan visibles para revisar. Se
registra un correo por identidad de licitación y dirección destinataria en la misma
transacción que la bandeja de salida cifrada. Las relecturas, cambios de versión y
reinicios reutilizan ese registro. El remitente debe ser un administrador activo con
SMTP y `procesos/consulta/ver`. Los fallos de entrega se gestionan desde la bandeja de
correo de Bastion.

Para la instalación, publicar Scout y Bastion, verificar CI, respaldar las versiones
activas y aplicar la migración 5 de Procesos con el migrador autorizado. La versión
anterior verifica el inventario exacto de migraciones. Planificar la reversión del
inventario antes de activar la nueva versión. Ejecutar
`scout_licitaciones.py --output-dir /var/lib/bastion-scout/licitaciones`
con el usuario operativo de Scout y dar al grupo de Bastion lectura de ese directorio
y de su JSON. El servicio de extracción solo necesita escribir en su directorio.
Los ejemplos systemd del repositorio Scout proponen un recorrido por hora con un
presupuesto de 45 minutos. El temporizador se instala explícitamente en AWS.

Tras publicar la primera extracción, configurar en el backend
`BASTION_TENDER_ALERT_SENDER_USER_ID` con el ID del administrador remitente,
`BASTION_TENDER_ALERT_RECIPIENT_EMAIL` con la dirección confirmada y
`BASTION_TENDER_ALERT_SINCE` con la fecha y hora de activación RFC3339. Así la lista
inicial queda visible y los correos empiezan con nuevas detecciones. El trabajador usa
la cuenta SMTP del administrador remitente y la configuración SMTP global existente.
Dejar las tres variables vacías mantiene los correos desactivados.

La comprobación local usa las fichas reales obtenidas en la prueba anterior y cubre
versiones estables, cambios, conservación parcial, enlaces, vencimiento, permisos,
revisión y consulta periódica. Este documento describe la implementación preparada.
El despliegue, la entrega SMTP y la activación del temporizador se verifican por separado.
