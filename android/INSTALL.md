# Instalar Bastion nativo 0.2.0 Preview

1. Desinstala la Preview anterior si Android indica conflicto de firma. Los registros de negocio permanecen en Bastion.
2. Abre `bastion-0.2.0-preview.apk` en el teléfono y permite instalar desde la aplicación que lo abrió.
3. Abre **Bastion Preview**, introduce tu correo, contraseña y código TOTP de Bastion.

Requiere Android 7 o posterior e Internet. Funciona sin Chrome ni otro navegador. Los módulos visibles dependen de tu cuenta y de su activación en Bastion. No introduzcas una dirección web: la API está configurada en la aplicación.

Los documentos PDF se consultan en el visor nativo; guardar archivos utiliza el selector Android. La carga móvil admite hasta 20 MiB, sujeta a límites menores del módulo. No hay modo offline ni guardado de borradores si Android termina el proceso.

`native-tests.apk` es únicamente para CI. El APK `unsigned` no es instalable sin firma. Esta Preview está firmada para pruebas; una versión futura con otra firma requerirá desinstalarla. La firma estable para distribución final se administra por separado.
