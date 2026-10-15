# Verificación OTP recordada y enrolamiento QR

Cambio solicitado el 15 de octubre de 2026. Core conserva TOTP obligatorio para la primera vinculación y cuando no existe una verificación recordada válida. No agrega permisos ni registro público.

## Datos y estados

Migración 0040: `mfa_browser_verifications` guarda hash de un token aleatorio, usuario, hash de IP normalizada, creación y vencimiento; índices por usuario y vencimiento. `sessions` agrega `ip_hash`, `browser_hash` y `family_hash` anulables. El último vincula rotaciones de autenticación para que una salida concurrente también invalide una confirmación que acaba de rotar la sesión. Las sesiones anteriores sin IP deben verificar OTP nuevamente. La IP sola nunca permite omitir el factor: hacen falta contraseña correcta y cookie del navegador vinculada al mismo usuario/IP. No se guarda la IP literal.

Cookie productiva `__Host-bastion_mfa`, Secure, HttpOnly, SameSite=Lax, Path=/, sin Domain; variante de desarrollo `bastion_dev_mfa`. Vigencia absoluta de 30 días desde la última confirmación OTP, sin renovación por contraseña. Es una caducidad de seguridad adicional a logout/IP solicitados, incorporada ante la consulta sobre secuestro de sesión. No va a JSON, almacenamiento web, auditoría ni logs. Sesiones mantienen inactividad de 30 minutos y absoluto de 12 horas configurables. Su caducidad y la limpieza periódica no borran la verificación del navegador.

Login rota sesión/CSRF y devuelve `mfa_required=false` únicamente con prueba válida y MFA ya vinculado. Primer navegador, cookie ausente/alterada/vencida, otra cuenta o IP diferente producen sesión parcial. OTP confirmado crea una prueba nueva y una sesión completa ligada a esa IP. Una sesión completa también requiere presentar la cookie recordada correspondiente: robar solo la cookie de sesión no basta. Cambiar IP durante una sesión invalida sesión y prueba asociada antes de acceder a datos. No se confía en cabeceras reenviadas salvo X-Real-IP del proxy local existente.

Logout manual revoca la prueba del navegador y todas las sesiones que la reutilicen, además de la sesión actual, incluso si esta ya venció o fue limpiada. Cambio/recuperación de contraseña, reset MFA, desactivación/cambio de roles y revocación administrativa eliminan todas las pruebas del usuario dentro de la misma transacción que sus sesiones. No se permite que login/factor concurrente restaure una prueba revocada: bloqueos de usuario antes de prueba/sesión.

Operaciones sensibles siguen requiriendo confirmación reciente con contraseña y sesión MFA vigente. No solicitan otro OTP en la misma IP; `code` permanece opcional para clientes anteriores y, si se envía, conserva validación y consumo de un uso.

## DTO y errores

- `POST /auth/login`: entrada igual; respuesta `csrf_token`, `mfa_required` booleano real.
- `GET /auth/status`: mantiene `mfa_complete`, `mfa_enabled`, `email`.
- `POST /auth/mfa/enroll`: respuesta `otpauth_uri` y `qr_data_url` PNG generado en el backend con pquerna/otp v1.5.0 ya fijado; nunca se consulta un servicio QR externo. Mostrar QR automáticamente al entrar al enrolamiento, con texto alternativo e instrucciones y URI manual. Reintentar no rota un secreto pendiente existente.
- Confirmación/desafío: mantienen DTO; emiten además cookie recordada tras commit correcto. Códigos de recuperación siguen siendo de un uso.
- `POST /auth/reauth`: contraseña obligatoria, código opcional por compatibilidad.
- Errores existentes: 401 prueba/contraseña/factor inválidos o sesión movida de IP; 403 CSRF/permisos/enrolamiento incompatible; 429 límites existentes. Una prueba inválida en login no revela motivo ni autoriza acceso: exige OTP.

## Aceptación

MySQL 5.7/HTTP: primer acceso requiere OTP; éxito emite cookies protegidas; reingreso tras caducidad omite OTP en mismo navegador/IP; prueba sin contraseña no autentica; IP nueva (incluida sesión abierta), cookie alterada/ausente, cuenta diferente, logout y revocaciones exigen OTP; logout con sesión vencida también revoca; reauth sin OTP acepta contraseña correcta y rechaza incorrecta; replay TOTP/recovery rechazado; limpieza conserva pruebas vigentes; enrolamiento repetido conserva secreto y devuelve PNG válido.

Angular/navegador: QR visible automáticamente y cargado, URI manual disponible, error/reintento explícito, login recordado carga perfil y navega a Inicio, salida borra estado; confirmaciones sensibles solo solicitan contraseña. Datos sintéticos y regresión completa; no equivale a despliegue productivo.

## Equipos reconocidos (ampliación solicitada)

Migración 0041 agrega device_id opaco independiente de la credencial, nombre derivado de navegador/sistema, fingerprint_hash auxiliar del User-Agent y last_seen_at a la prueba. La cookie persistente identifica la instalación del navegador; la huella no autoriza acceso ni identifica hardware de forma infalible y no se toman canvas, fuentes ni identificadores físicos. Navegadores con igual huella pero sin cookie requieren OTP. Actualización de navegador no revoca por sí sola confianza; login correcto actualiza huella/nombre/último uso. Cerrar pestaña/reiniciar equipo conserva la cookie, con los límites de IP/logout/30 días existentes.

GET `/auth/devices?page=1&per_page=25`: permiso propio `core.identity.self`, sesión completa; respuesta `data` (id, name, created_at, last_seen_at, expires_at, current), total; únicamente propios/vigentes y sin cookies, hashes ni IP literal. DELETE `/auth/devices/{id}`: mismo permiso, contraseña reciente/CSRF, 404 para ID inexistente/ajeno; revoca prueba y sesiones asociadas en una TX con audit `auth.device_revoke`. Respuesta current; revocar actual limpia ambas cookies y redirige a login. En Mi cuenta se consultan equipos y última actividad, se confirma contraseña y se revoca un equipo. No nuevo menú superior ni permisos de administración para ver equipos ajenos. Aceptación: fingerprint igual sin credencial no autoriza; reinicio/reingreso conserva equipo; lista propia/paginada, reauth/CSRF/ajeno y revocación aislada/actual; regresión OTP/IP/QR intacta.
