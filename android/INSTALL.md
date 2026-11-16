# Bastion para Android · 0.1.1 Preview

Instala **bastion-0.1.1-preview.apk** en Android 7.0 o posterior. Descarga y
descomprime el artifact `bastion-android-apk` de GitHub Actions, abre ese APK
y permite la instalación desde la aplicación usada para abrirlo si Android lo pide.
Abre «Bastion Preview» e inicia sesión con tu cuenta y autenticador habituales.
Necesita Internet y un navegador actualizado compatible con Custom Tabs/TWA.

Esta entrega es una versión de prueba firmada con una clave de depuración,
no una publicación en Google Play. Una nueva compilación puede necesitar
desinstalar la anterior porque la clave temporal del runner cambia.
Si instalaste 0.1.0, desinstala primero esa Preview: esta compilación tiene
una firma de desarrollo diferente. Los datos de negocio permanecen en el servidor.
`bastion-0.1.1-unsigned.apk` NO es instalable: es el paquete para firmar con
la clave privada permanente del propietario antes de distribuir una versión estable.
No se ha instalado ni probado esta entrega en un teléfono físico.

El APK abre `https://bastion.castilloit.net/` y usa la versión del frontend
publicada allí. No incluye una copia de las pantallas ni funciona sin conexión.
Los ajustes nuevos de pantalla requieren desplegar el artifact web del mismo
commit en Bluehost; generar el APK no realiza ese despliegue.

Mientras no se publique la asociación Digital Asset Links con el certificado
de la versión estable, el navegador muestra su barra de dirección. Es el
comportamiento seguro esperado y permite usar el sistema desde la instalación.
La API, sesiones, permisos, TOTP, PDF, archivos y auditoría siguen a cargo del
Core y del navegador; el APK no guarda claves ni recibe acceso directo a datos.

Comprueba el SHA-256 contra `SHA256SUMS`. `BUILD.txt`, `APK-INFO.txt` y
`SIGNATURE.txt` identifican el código, los requisitos y el certificado comprobado.
