# Ejecución del Core

Core de plataforma implementado localmente. Esto no acredita producción, acceso a Bluehost/S3/SMTP reales, capacidad de memoria de Lightsail ni OAuth Work. Credenciales y servicios reales requieren entorno autorizado. [OpenAPI](../contracts/core-openapi.json) describe endpoints implementados; [estado](status.md) mantiene límites.

## Dependencias y configuración

Go 1.27.2; Node 24 LTS compatible con Angular 22.2; npm ci utiliza lockfile. MySQL 5.7 con InnoDB/utf8mb4. AWS S3 y SMTP deshabilitados por defecto. Producción requiere BASTION_ENV=production, origen HTTPS y denylist curada; development se limita a loopback. Crear DB y cuentas fuera de la app con privilegios diferenciados: migración DDL y ejecución DML limitado; auditoría INSERT/SELECT, sin UPDATE/DELETE para runtime. El schema runner no se ejecuta al arrancar servidor.

Copiar backend/.env.example a un archivo privado fuera del document root y cargarlo en el entorno del proceso. No se carga automáticamente .env. Generar BASTION_ENCRYPTION_KEY con openssl rand -base64 32; custodiar junto con el plan de backup. No usar la clave sintética de tests. Sesiones/códigos se guardan hasheados; TOTP/SMTP con AES-GCM. Cambiar la clave requiere descifrar/recifrar datos, no una simple edición del entorno; conservar acceso a clave anterior durante migración autorizada.

MySQL DSN requiere parseTime=true y UTC. Producción rechaza conexiones sin TLS verificado, skip-verify/preferred, salvo túnel SSH local explícitamente validado con BASTION_DB_TUNNEL_VERIFIED=true. Esa bandera declara evidencia operativa; no abre ni verifica el túnel. No usarla para eludir TLS remoto. Pool máximo 8 conexiones; límites del hosting deben validarse.

```bash
cd backend
go mod download
go run ./cmd/admin migrate
go run ./cmd/admin bootstrap
go run ./cmd/server
```

Bootstrap interactivo: correo/nombre/contraseña 15+ caracteres. Solo funciona en DB sin usuarios; crea Administración del sistema con permisos técnicos Core, sin aprobación financiera. No argumentos/env con contraseña bootstrap. Login inicial exige enrolar TOTP y guardar códigos. La cuenta administrativa inicial/rol están protegidos contra desactivación/reasignación desde UI; no perder sus medios de recuperación. Roles comerciales no se inventan. Otros roles se configuran y asignan antes de acceso funcional.

Datos sintéticos locales: las pruebas pueden generar administradores ficticios en bases cuyo nombre es bastion_test o bastion_browser_test, nunca en la BD real. No desplegar tests/browserseed como utilidad productiva.

## Frontend y API

```bash
cd frontend
npm ci
npm start
```

Abrir http://localhost:4200; servidor http://localhost:8080. Son orígenes distintos. En desarrollo se usa cookie bastion_dev_session sin Secure para HTTP local; en producción __Host-bastion_session con Secure/HttpOnly/Lax, sin Domain, Path=/. No intentar reutilizar cookie de desarrollo en producción.

La SPA recupera CSRF en JSON y lo envía explícitamente en mutaciones. CORS exacto; Origin obligatorio en mutaciones. Campos desconocidos/cuerpos extra se rechazan en DTO. Sesión parcial solo identidad; permisos se comprueban nuevamente en backend. GET de polling no actualiza inactividad sin marca de actividad humana. Login/MFA limitados por ventanas MySQL; hasta 32 solicitudes simultáneas y 2 cálculos Argon2id por proceso. No equivalen a WAF/DDoS.

Configuración permite usuarios, roles/equipos, empresa/SMTP, auditoría, jobs, archivos y conexiones técnicas. Reautenticación válida 5 minutos; códigos TOTP no reutilizables dentro del mismo paso. Recuperación usa códigos de un uso. Reset de contraseña conserva MFA; reset MFA administrativo exige referencia de verificación y revoca sesiones/conexiones. Las invitaciones/códigos/token técnico se muestran una vez para entregar por canal autorizado; no hay envío automático de invitación.

Producción exige BASTION_PASSWORD_DENYLIST_FILE: corpus local curado de contraseñas comunes/filtradas como SHA256 (un hash por línea, máximo 100.000 únicos), fuera de Git. Corpus vacío/ilegible/malformado impide arrancar. En desarrollo se bloquea también una lista mínima incorporada. El corpus elegido tiene cobertura limitada; no se afirma consultar todas las filtraciones ni se contacta un proveedor externo. Preparar/verificar corpus antes de producción. Sesiones simultáneas permitidas dentro de vencimientos y revocación; no existe una decisión comercial de limitar a una sesión.

## Workers y SMTP

```bash
cd backend
go run ./cmd/worker
```

Runner de hasta 30 jobs/60 s, unidades con timeout 45 s y lease 90 s; reintentos hasta 5. Ejecutar con timer cada minuto, sin solapamiento del proceso systemd; MySQL soporta múltiples workers acotados. Outbox transaccional se publica de forma idempotente; no requiere Redis. Actor vigente se reevalúa. Trabajo desconocido falla explícitamente. Reintento administrativo solo failed; uncertain necesita conciliación.

Los flags de entorno son techo: settings/integrations permite desactivar inmediatamente MCP/files/email mediante administración con reautenticación y auditoría; no permite superar ese techo. BASTION_EMAIL_ENABLED=true y allowlist BASTION_SMTP_ALLOWED_HOSTS solo en entorno autorizado. Config SMTP: TLS implícito 465 o STARTTLS obligatorio 587, nunca plaintext. Credenciales cifradas no aparecen en API. Prueba encolada solo al correo propio, con reautenticación/Idempotency-Key. Worker registra sending antes de red; timeout/crash puede dejar sending/uncertain. No reenvía a ciegas. Conciliación requiere evidencia y no crea otro envío. accepted significa aceptación SMTP, no entrega al buzón. Mailer falso se inyecta solo en tests. Lectura IMAP/RFQ pertenece a la fase de integración AI, no está activada aquí.

## Archivos S3 y cuarentena

BASTION_FILES_ENABLED=true requiere bucket privado y región, credenciales AWS de mínimo privilegio fuera de Git. No se usa el bucket crawler como depósito implícito. Requiere bloqueo público, cifrado y CORS S3 limitado al frontend/método PUT/headers Content-Type y x-amz-checksum-sha256. Validar estos controles antes de activar.

Bytes en S3, metadatos/versiones en MySQL. PUT firmado 5 minutos, checksum SHA256 y tamaño/tipo esperados; completar verifica checksum real S3 y firma de tipo de los primeros bytes. Estado pending→quarantined; descarga solo available, attachment y URL temporal. El análisis de malware es externo, no se ejecuta OCR ni scanner pesado en Lightsail.

POST documents/{id}/scan requiere permiso técnico, reautenticación, número de versión y referencia real del análisis; registra clean/rejected. El operador solo lo invoca con evidencia del ejecutor de análisis. La carga/confirmación por sí sola nunca autoriza descarga. La UI no ofrece un botón para omitir el scanner. Artefactos rechazados/pending huérfanos requieren política de retención/limpieza autorizada; no se borran arbitrariamente.

Archivos personales Core admiten scopes all/self/team. Vínculos a entidades de módulos aún no implementados fallan cerrados: antes de enlazar documentos comerciales, el módulo incorpora su política de entidad compartida para REST/MCP. La lista solo contiene archivos Core sin vínculo comercial; no hay acceso por una key S3 enviada por el usuario.

## MCP

BASTION_MCP_ENABLED=false por defecto. Al habilitar en entorno de prueba, /mcp usa SDK oficial Streamable HTTP stateless y bearer técnico revocable/expirable ligado al creador con MFA; no acepta sesión SPA como OAuth. Herramientas iniciales core_modules, core_jobs y core_document_download, scopes por herramienta y permisos actuales. Sin SQL/shell ni correo arbitrario. No existen tools AI de fases futuras aún. Se verifica por SDK cliente, no por simular JSON-RPC manualmente.

OAuth/PKCE delegado a Work sigue pendiente de proveedor/compatibilidad real. No presentar bearer técnico como autorización OAuth ni conexión Work certificada. Hasta validar esos requisitos mantener MCP externo deshabilitado/restringido por proxy de pruebas.

## Pruebas y builds

```bash
cd backend
go vet ./...
go test -race ./...
# Configurar DSN de una base MySQL 5.7 vacía y desechable llamada bastion_test:
go test -race -count=1 ./tests/integration
make build
cd ../frontend
npm ci
npm run build
npm test -- --watch=false
cd ..
python scripts/check_repository.py
```

Integración limpia todas las tablas de bastion_test para cada caso: no usar otro esquema. Sin variable BASTION_TEST_MYSQL_DSN las pruebas MySQL se omiten explícitamente; CI siempre la configura. Incluye doce solicitudes simultáneas, rollback/outbox, lease vencido, revocación, códigos reutilizados, cuarentena/IDOR, SMTP incierto y SDK MCP.

## Prueba de navegador

Configurar base desechable bastion_browser_test, exportar BASTION_TEST_MYSQL_DSN para tests/browserseed y mismo DSN en backend, clave sintética solo de test. Sembrar go run ./tests/browserseed; ejecutar servidor y frontend. Entonces:

```bash
cd frontend
npx playwright install chromium
npx playwright test
```

Opcional BASTION_BROWSER_EXECUTABLE selecciona Chrome instalado. Test contra API/Angular reales en dos orígenes, login→TOTP→recovery→config persistida tras recarga→logout; sin mocks HTTP ni SMTP real. No usar cuentas ni documentos reales. Config CI reproduce este flujo.

## Despliegue autorizado posterior

Templates en backend/deploy: servicio server, timer worker y proxy nginx. Editar rutas/usuarios/certificados/env en entorno autorizado; no instalar automáticamente. Angular assets en Bluehost con fallback index.html y headers de seguridad/CSP adaptados. MySQL sigue Bluehost. TLS/DNS/túnel, restore y memoria total con crawler requieren prueba operativa previa. No publicar fuentes, claves ni ejemplos de prueba. WAF/protección perimetral pendiente de configuración real.
