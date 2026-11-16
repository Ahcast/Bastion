# Aplicación Android y pantallas pequeñas

## Decisión del 2026-11-16

Por solicitud de Marco, se añade un cliente Android ligero mediante Trusted Web
Activity (TWA) / Custom Tabs. Reutiliza el frontend React publicado en
`https://bastion.castilloit.net/`. No duplica reglas ni datos. Bluehost sigue
sirviendo el frontend y MySQL; AWS conserva backend/API/MCP. La autenticación,
cookies, CSRF, TOTP y autorización se mantienen en su origen HTTPS existente.

El navegador gestiona PDF, descargas y selección de archivos con su sesión.
El contenedor no incorpora JavaScript nativo, secretos, caché offline de datos
privados, permisos de almacenamiento ni acceso a SQL. La actividad ignora URLs
y extras de intents externos: siempre abre el sitio fijo.

La interfaz común incorpora ajustes para 320–430 px: encabezado accesible al
desplazarse, controles de al menos 44 px, campos de 16 px, safe areas, modales
con altura dinámica y paginación que puede ocupar varias líneas. Conserva
campos, botones, permisos y cálculos existentes. Esta mejora móvil es nueva,
autorizada por la solicitud, no un requisito inferido de paridad Legacy.

## Compilación y entrega

La primera sesión quedó limitada por permisos del entorno. Tras la autorización
del usuario se sincronizó `origin/main` en `0046bfb`, conservando las mejoras
de autenticación para dos sesiones simultáneas. Chromium local verificó 23
estados de acceso, menú, clientes y formulario, a 320, 360, 390, 430 y 1280 px,
incluyendo altura reducida de 400 px, sin desbordamiento horizontal. Las
respuestas fueron sintéticas. Las 188 pruebas frontend, TypeScript y Vite
pasaron en la preparación. Las comprobaciones Android corresponden al workflow
y la evidencia de entrega registrada en `09-despliegue.md`.

Proyecto `android/`, Java 17, Gradle 8.13, Android Gradle Plugin 8.13.2,
compile/target SDK 36, mínimo Android 7 (API 24), Browser Helper 2.7.4.
Con SDK y Java instalados: `gradle -p android :app:assembleDebug :app:lintDebug`.
La versión de Gradle debe ser 8.13, como fija el workflow `Android APK`.

CI genera un APK Preview instalable con ID `net.castilloit.bastion.preview`,
firma de depuración verificada, SHA-256 y procedencia. El APK release con ID
`net.castilloit.bastion` se entrega sin firma; **no es instalable**. Antes de
distribución estable, firmarlo con clave permanente privada y custodiada por
el propietario; no guardar esa clave en Git ni en artifacts. La clave efímera
de Preview cambia entre runners y no sirve como identidad de distribución.
Ver [instalación](../android/INSTALL.md).

La primera entrega se construyó desde `bb5d2f2d7d2e1b4393ee6b9742c7ccd4f1456c1c`
con [Android APK aprobado](https://github.com/Ahcast/Bastion/actions/runs/43933782305).
`bastion-0.1.0-preview.apk` contiene 4 848 801 bytes y SHA-256
`9f7a71c95b1c526cae0f35f8ec6a376435d1fc2512eb3cc6bcf4ee88c1c99c35`.
La firma APK v2 se verificó con `apksigner`; también pasaron lint Android y
las 23 comprobaciones de diseño Chromium en CI. Se cotejaron localmente el
digest oficial del ZIP, los APK y el commit. No se ha instalado en teléfono físico.

## Activación y comprobaciones pendientes

- Generar APK no instala la aplicación en un teléfono, no despliega el frontend
  ni publica en Google Play. La interfaz del APK corresponde al sitio activo.
- El frontend móvil de esta entrega ya se desplegó en Bluehost con respaldo,
  configuración preservada y 53 recursos HTTPS cotejados. Véase `09-despliegue.md`.
- Para ocultar la barra del navegador, publicar en Bluehost
  `/.well-known/assetlinks.json` la relación `delegate_permission/common.handle_all_urls`,
  namespace `android_app`, package `net.castilloit.bastion` y SHA-256 del certificado
  estable. No asociar una clave efímera de CI con el sitio productivo.
- Sin esa asociación el fallback Custom Tabs conserva una barra de dirección.
  No se desactiva ni simula la comprobación de dominio.
- Probar en teléfono real inicio de sesión/TOTP, teclado, navegación, PDF,
  subida/descarga de archivos, rotación, vuelta desde otra app y sesión expirada.
- Las pruebas Chromium usan respuestas sintéticas y no acreditan esos flujos
  de extremo a extremo contra producción ni equivalen a pruebas Android.

Referencia: [documentación oficial TWA](https://developer.chrome.com/docs/android/trusted-web-activity/).
