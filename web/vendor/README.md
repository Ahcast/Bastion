# Generador QR local

`qrcode-2.0.4.js` es la copia sin modificaciones de `dist/qrcode.js` de `qrcode-generator` 2.0.4, por Kazuhiko Arase, bajo licencia MIT. La licencia se conserva en este directorio y el encabezado original del código permanece intacto.

- Fuente: https://github.com/kazuhikoarase/qrcode-generator
- Paquete: https://registry.npmjs.org/qrcode-generator/-/qrcode-generator-2.0.4.tgz
- SHA-256 del archivo JavaScript: `79ec86f82856005b1c887905cfccfcfbec3821ca61c7fd5a952faa5f778f791c`.
- Integridad SHA-512 del paquete npm: `mZSiP6RnbHl4xL2Ap5HfkjLnmxfKcPWpWe/c+5XxCuetEenqmNFf1FH/ftXPCtFG5/TDobjsjz6sSNL0Sr8Z9g==`.

Se carga desde el mismo origen antes de `app.js`. El enrolamiento codifica la URI `otpauth://` en SVG dentro del navegador: no usa servicios QR, CDN, solicitudes de red adicionales ni almacenamiento persistente de la clave. El backend entrega la etiqueta escapada como URI, por lo que el contenido se codifica en modo Byte sin transformar la URI.
