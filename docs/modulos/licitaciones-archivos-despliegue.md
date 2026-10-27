# Licitaciones: nombre y archivos publicados el 2026-10-27

Backend AWS y frontend Bluehost ejecutan `f41b7a6d6c6b8ac9db8b56302d4b14e45dca8b07`, posterior a `e5c8d9f3734b85be136e5804dfca6101ea3a5990`. [Core checks aprobado](https://github.com/Ahcast/Bastion/actions/runs/40594463205): formato, compilación, pruebas del workspace, pruebas de archivos con MySQL 5.7 y almacenamiento privado sintético, 132 pruebas del frontend y build de producción. La prueba dedicada verifica el límite real de 20.000.000 bytes, copias independientes, reintentos, versiones, autorización HTTP y limpieza de claves exactas.

Se publican el [nombre editable](licitaciones-nombre.md) y los [archivos legales y técnicos](licitaciones-archivos.md), sin precargar formularios. El nombre se edita en Resumen. Documentación legal y Especificaciones técnicas permiten subir PDF/imágenes desde el PC o incorporar una copia de Colaboración; conservan vista previa, descarga y retirada confirmada con sus permisos respectivos. El diseño mantiene los componentes y estilos compartidos de Cotizaciones.

## AWS y datos

Se validaron el digest del ZIP de GitHub, el paquete, BUILD.txt y sus hashes internos. SHA-256 del paquete backend: `16a3ff55c8c56ebcd80df704108af2952e6e54fb5f5dce834db95a31b2126bb0`.

Antes de migrar se detuvo el servicio, se guardó un respaldo privado de MySQL y configuración y se verificó la restauración de las 81 tablas en tablas temporales, comparando todas sus filas. Se aplicaron únicamente cambios nuevos de Licitaciones: migración 2 (nombre) y 3 (tres tablas de archivos/recibos/limpieza), pasando de 81 a 84 tablas. La proyección de los campos anteriores de los expedientes permaneció idéntica; los nombres existentes comenzaron vacíos. Los esquemas y datos de las demás tablas quedaron intactos, salvo las entradas esperadas del ledger de Licitaciones. No se crearon registros de negocio sintéticos en producción.

El ejecutable activo corresponde al commit indicado, readiness respondió 200 y se preservaron ambiente y activación de los módulos, incluido Licitaciones activo. No se persistieron credenciales de migración. Se mantiene el pendiente general de separar la identidad de migración del acceso operativo existente.

El usuario aplicó manualmente la [política IAM acotada](licitaciones-s3-policy.json) al usuario Bastion, después de que el acceso disponible devolviera AccessDenied para administrarla. Se verificó con la identidad del servicio: escritura AES256, lectura con hash, acceso anónimo 403 y retirada completa de dos versiones sintéticas y su marcador. No quedaron objetos sintéticos de esa comprobación. No se ampliaron otros prefijos.

## Bluehost y comprobación pública

Se verificó el paquete estático `c3a70056527a3fb66a363983409f0aae187fec164dd4ccfe037e01bc8138e9b3`, se respaldó y cotejó el sitio anterior, se publicaron assets antes del índice y se conservaron config.js, ambas reglas Apache y los assets anteriores para recuperación. El paquete publicado contiene 47 archivos; 45 hashes se verificaron públicamente por HTTPS, excluyendo las dos reglas Apache privadas.

BUILD.txt identifica el commit indicado. Las rutas SPA de Licitaciones, Cotizaciones y Colaboración respondieron 200; readiness respondió 200 y las rutas de listado, archivos, preview y biblioteca sin sesión respondieron 401. TLS validado. La publicación terminó a las 22:35 UTC. Respaldos y registros operativos permanecen privados en los hosts.

No se completó una revisión visual autenticada en navegador; la evidencia funcional proviene de pruebas automatizadas y las comprobaciones operativas descritas. PDF final, índice, foliado, lotes/ítems y adjudicación conservan sus pendientes de definición e implementación.
