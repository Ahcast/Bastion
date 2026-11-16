# Bastion Android

Cliente TWA / Custom Tabs del sitio HTTPS vigente de Bastion. El workflow
`../.github/workflows/android.yml` compila, verifica y empaqueta el APK Preview
instalable. Cada artifact incluye su commit, SHA-256 y certificado.

Requisitos de compilación: Java 17, Gradle 8.13, Android SDK con plataforma 36
y build tools 36.0.0. Configurar `ANDROID_HOME` y `JAVA_HOME` en el entorno.
Desde la raíz del repositorio:

```sh
gradle -p android --no-daemon :app:assembleDebug :app:lintDebug
```

Si se descomprime este proyecto por separado, ejecutar desde su directorio:

```sh
gradle --no-daemon :app:assembleDebug :app:lintDebug
```

El resultado, cuando la compilación termine correctamente, estará en
`app/build/outputs/apk/debug/app-debug.apk`. La variante Preview usa una firma
de desarrollo y un ID distinto a producción. Para una distribución estable,
compilar `:app:assembleRelease`, firmar con la clave permanente del propietario
y configurar Digital Asset Links. Nunca publicar claves privadas.

Ver [instalación y limitaciones](INSTALL.md) y [arquitectura](../docs/13-android.md).
Los cambios de interfaz viven en `web/`; el APK usa lo publicado en Bluehost.
