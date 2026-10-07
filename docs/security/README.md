# Seguridad por defecto

## Identidad y sesiones
- Sanctum en modo SPA con cookies y Fortify.
- TOTP obligatorio antes de acceder a operaciones de negocio; códigos de recuperación.
- Cookie de sesión __Host-bastion_session: Secure, HttpOnly, Path=/, sin Domain y SameSite=Lax.
- Protección CSRF y rotación de sesión después del login.
- Tokens de sesión fuera de localStorage.
- Recuperar contraseña no desactiva MFA. Reset de MFA mediante procedimiento administrativo verificado y auditado.
- Reautenticación reciente para aprobaciones, cambios bancarios, permisos y recuperación de MFA.
- Passkeys para admins y aprobadores como fase posterior.

## Autorización
Denegar por defecto. Políticas en backend para cada recurso, mínimo privilegio y separación entre quien prepara y quien aprueba. La interfaz no es una barrera de seguridad.

## Protección
Rate limits para login, recuperación, importaciones y operaciones costosas. Validar entrada, tamaño y tipo real de archivos. Consultas parametrizadas, salida escapada y CORS cerrado al origen previsto.
WAF y mitigación DDoS en el perímetro: proveedor y reglas por seleccionar. Evitar exponer el origen cuando el hosting permita restringirlo.
Configurar CSP y cabeceras de seguridad; activar HSTS después de comprobar HTTPS.

## Secretos y datos
Credenciales fuera de Git y del webroot; separar desarrollo y producción. Nunca guardar contraseñas, tokens, cookies, códigos MFA ni contenido sensible completo en logs.
Auditoría de acciones críticas con actor, recurso, resultado y correlación. Retención y protección contra alteración por definir.
Backups cifrados, acceso separado y pruebas de restauración.

## IA
Documentos externos son datos no confiables. Nunca seguir sus instrucciones como autorización. Validar acciones contra permisos del usuario, limitar herramientas y exigir aprobación humana para comunicaciones y cambios sensibles.
