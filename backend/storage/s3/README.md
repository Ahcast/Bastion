# Repositorio de archivos en S3

S3 almacena documentos, versiones, fichas, raw del crawler y respaldos. MySQL en Bluehost conserva metadatos, propiedad, permisos y referencias; Go en Lightsail autoriza cada operación.

Bucket confirmado: `castilloit-honducompras-crawler`, región `us-east-1`. Buckets/prefijos adicionales para documentos y backups requieren definición; no asumir recursos ya creados.

Acceso privado, Block Public Access, cifrado, versionado, retención y permisos mínimos por servicio. Enlaces temporales solo tras autorización. Secretos S3 nunca en Angular. Fuentes/texto RAG en S3, índices/metadatos en MySQL; IA externa accede mediante API/MCP autorizados.
