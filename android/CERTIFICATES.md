# Raíz pública de compatibilidad TLS

`app/src/main/res/raw/isrg_root_x1.pem` es el certificado público ISRG Root X1 obtenido de [Let's Encrypt](https://letsencrypt.org/certs/isrgrootx1.pem), descrito en sus [cadenas de confianza oficiales](https://letsencrypt.org/certificates/). No contiene clave privada.

Huella SHA-256 DER: `96bcec06264976f37460779acf28c5a7cfe8a3c0aae11a8ffcee05c0bddf08c6`.

La prueba HTTPS nativa encontró `Trust anchor for certification path not found` en Android 7/API 24. La cadena pública observada en la API el 2026-11-17 es YE2 → Root YE → ISRG Root X2 (cruzada) → ISRG Root X1. Android 16 validó la conexión con sus raíces del sistema.

Se añade X1 exclusivamente al dominio exacto `api.bastion.castilloit.net` mediante [Network Security Configuration de Android](https://developer.android.com/privacy-and-security/security-config). Se conservan raíces del sistema, validación de nombre, cadena y vigencia; HTTP claro sigue prohibido. No hay TrustManager permisivo, aceptación de certificados arbitrarios, anclaje al certificado final renovable ni modificación del almacén del teléfono. Revisar este recurso cuando cambien las cadenas o políticas de confianza del proveedor.
