# Bastion Core

## Estado de esta entrega

Copia inicial del código existente y de los lineamientos de `docs/`, publicada directamente en `main`, sin ramas de desarrollo. El workspace Rust está en la raíz; el frontend estático está en `web/`.

Los dominios confirmados para las pruebas son `bastion.castilloit.net` para el frontend en Bluehost (`162.241.224.215`) y `api.bastion.castilloit.net` para la API en AWS Lightsail (IP estática `34.236.78.141`, confirmada por el usuario y DNS verificado desde Lightsail); MySQL permanece en Bluehost. Configurar `BASTION_CORS_ALLOWED_ORIGIN` con el origen exacto del frontend y `web/config.js` con el origen de la API.

La implementación es parcial y no se declara lista para producción. GitHub Actions comprobó la compilación y las pruebas del código inicial; la conexión real con MySQL y la validación integral del despliegue siguen pendientes. Se comprobó la sintaxis de `web/app.js` con Node. Los flujos de cambio/restablecimiento administrativo de TOTP, cambio confirmado de correo, Empresa/SAR y OAuth/MCP siguen pendientes; hay controles y pantallas existentes que deben validarse antes de considerar completo el Core.

El workflow `Rust checks` verifica formato, compilación y pruebas, y después genera un ejecutable de release para Lightsail: Linux x86_64 con Ubuntu 24.04. Usa `Cargo.lock` y `--locked` para conservar las versiones de dependencias. El paquete descargable `bastion-server-linux-x86_64` queda como artifact de la ejecución durante 30 días; incluye el ejecutable, una plantilla `.env.example`, el lockfile, el commit de origen y sumas SHA-256. Generar el paquete no instala ni inicia la API en el servidor.

Rust backend for the new CastilloIT application. The runtime is a modular monolith: Core owns authentication, authorization, settings, the module registry, audit history, and shared API controls. Business modules are added separately and remain disabled until they are included in a release, allowed by the environment, and enabled by an administrator.

## Current Core API

- One-time first administrator setup with a temporary secret and required TOTP enrollment.
- Argon2id password hashes, one-use recovery codes, opaque hashed session/device/challenge tokens, and AES-256-GCM encryption for TOTP seeds.
- TOTP challenges expire after five minutes; trusted browser tokens expire after 15 days and are bound to the observed IP. The Core allows one active session per user, enforces 24 hours of inactivity for ordinary sessions, and requires TOTP again after 15 days for persistent sessions.
- Exact CORS origin, exact `Origin` checks, CSRF double-submit token, host-only `Secure` cookies, bounded request bodies, safe response headers, request IDs, and RFC 9457 error responses.
- Core Person, User, Role, Module, Settings, and read-only Audit APIs. User creation requires a free Person and a role. New roles start without permissions. User accounts are disabled rather than deleted; removing the last active System Administrator is rejected.
- Permission capabilities are `ver`, `editar` (includes create and update), and `eliminar`. Core settings and user/role administration are System Administrator-only.
- MySQL 5.7-compatible schema and parameterized SQL. The runtime pool is bounded and MySQL TLS identity is verified.

## Build and run

Install the Rust toolchain from `rust-toolchain.toml`. Copy `.env.example` to `.env`, then set real database connection values, a TLS CA, a random bootstrap secret (at least 32 bytes), and a random 32-byte base64 data key. The normal server uses `MYSQL_USER`, which should have only the DML permissions needed by the app. Schema changes use the separate migrator credential, loaded only by `migrate`; omit it from the running server environment. `check-db` needs only the runtime MySQL configuration and CA, verifies TLS identity, and executes only `SELECT 1`, so it can test an empty database without creating tables or starting the server.

```powershell
cargo test --workspace
cargo run -p bastion-server -- check-db
cargo run -p bastion-server -- migrate
cargo run -p bastion-server -- serve
```

The API listens on `BASTION_LISTEN_ADDR`. Health checks are `/health/live` and `/health/ready`. The frontend must first call `GET /api/v1/auth/csrf`; state-changing requests send the returned token as `X-CSRF-Token`, preserve cookies, and send the configured exact `Origin`.

The first administrator flow is `GET /api/v1/bootstrap/status`, `POST /api/v1/bootstrap`, then `POST /api/v1/bootstrap/complete`. The bootstrap response contains the TOTP seed and one-time setup token. Completion requires a valid TOTP and returns recovery codes once. Protect those responses and save the codes securely.

## Deployment boundary

The frontend remains on Bluehost, the backend/API/MCP target remains AWS, and MySQL remains on Bluehost. Keep the MySQL runtime user limited to required data operations and use a separate migration credential. Configure the AWS egress address and Bluehost allowlist, verify the MySQL certificate and hostname, and inject all secrets from the deployment secret store.

This API foundation is not a production approval. SMTP delivery, invitation and password-recovery mail, trusted-proxy handling, and the static frontend have partial implementations that still require compilation and integration tests. OAuth client consent, MCP Streamable HTTP/tools, and AWS KMS envelope-key rotation are not complete. These components must be implemented and tested before the Core is considered complete.
