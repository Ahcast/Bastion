# Transporte previsto del PDF final

El usuario confirmó un límite independiente de 1.000.000.000 bytes. La generación y la descarga de ese tamaño siguen pendientes: el almacenamiento privado actual limita las operaciones en memoria a 100 MiB y no debe ampliarse globalmente.

La carga preparada usa el servicio de almacenamiento privado existente de Bastion, leyendo el PDF desde un archivo con bloques acotados y SHA-256. Se limita al prefijo de Licitaciones y aún no está conectada a una ruta pública. La cobertura automatizada verifica el transporte con un sustituto local; el usuario confirmó que Bastion ya dispone de lectura y escritura en S3. Quedan pendientes la descarga por flujo, la integración del trabajador, el aislamiento del directorio de trabajo, la limpieza y la concurrencia.
