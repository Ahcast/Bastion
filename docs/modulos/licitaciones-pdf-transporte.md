# Transporte previsto del PDF final

El usuario confirmó un límite independiente de 1.000.000.000 bytes. La generación y la descarga de ese tamaño siguen pendientes: el almacenamiento privado actual limita las operaciones en memoria a 100 MiB y no debe ampliarse globalmente.

La rama de desarrollo prepara una carga de PDF desde un archivo ya abierto, con bloques acotados y SHA-256 para detectar cambios durante la transferencia. Se limita al prefijo de Licitaciones y no queda conectada a una ruta pública. La prueba automatizada usa un transporte simulado sin credenciales ni red; no acredita que AWS CLI acepte el descriptor de archivo ni que S3 complete la operación. Antes de conectar o desplegar esta capacidad deben verificarse el transporte real usando un principal de prueba autorizado, la descarga por flujo, el aislamiento del directorio de trabajo, la limpieza de objetos y la concurrencia. La verificación real sigue pendiente de un acceso de prueba autorizado.
