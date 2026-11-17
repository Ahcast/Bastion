# Bastion Android nativo

Cliente Java/Android Views de la API de Bastion. Sin navegador, WebView ni frontend web embebido. Android 7+ (API 24), target/compile 36, Java 17, Gradle 8.13.

```sh
gradle -p android --no-daemon :app:assembleDebug :app:lintDebug :app:assembleRelease :app:assembleDebugAndroidTest
```

El APK instalable es `app/build/outputs/apk/debug/app-debug.apk`. El release requiere firma duradera externa. La CI verifica hashes/firma y ejecuta el Preview real e instrumentación en API 24/35/36 con Chrome deshabilitado. Pruebas autenticadas usan datos ficticios; nunca escriben en producción.

Contratos: instalar `yaml@2.9.1` en un prefijo de herramientas, establecer `BASTION_YAML` a su directorio y ejecutar `node android/qa/generate-contract.cjs`. Se versiona el recurso resultante; no requiere Node en Android. La API fija conserva sesiones, CSRF, TOTP, permisos, If-Match e idempotencia.

Ver [arquitectura y alcance](../docs/13-android.md) e [instalación](INSTALL.md).
