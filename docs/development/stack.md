# Stack y selección de dependencias

Core implementado con Go 1.27.2, Angular 22.2.2, TypeScript 6.0.3 y Node 24.19.0. Dependencias exactas en go.mod/go.sum y package-lock.json; la matriz vigente fue validada mediante instalación/build/pruebas.

| Área | Base |
| --- | --- |
| Backend | Go 1.27, parche estable soportado verificado al inicializar |
| HTTP | net/http estándar, rutas REST y middleware explícito; evitar framework innecesario |
| Persistencia | database/sql + go-sql-driver/mysql, consultas parametrizadas y transacciones |
| Migraciones | SQL numerado, ejecutor reproducible y registro de versión |
| Dinero | Biblioteca decimal mantenida; cadenas JSON y DECIMAL MySQL según modelo |
| Sesiones | Tokens opacos CSPRNG, hash en BD, expiración/revocación; cookie host-only |
| Password/TOTP | Argon2id de biblioteca mantenida; TOTP compatible y recuperación de un uso; sin criptografía propia |
| API | DTO explícitos, OpenAPI 3.1 como contrato versionado |
| MCP | SDK oficial github.com/modelcontextprotocol/go-sdk/mcp, Streamable HTTP; conformance y prueba cliente |
| OAuth delegado | Biblioteca/proveedor conforme mantenido; authorization code + PKCE; discovery, audiencia, revocación. No implementar servidor OAuth desde cero |
| S3 | AWS SDK for Go v2; objetos privados, links temporales autorizados |
| Jobs | Cola/outbox MySQL, workers Go + systemd/timers; leases/retries; no Redis/SQS inicial |
| Frontend | Angular estable soportado + TypeScript/RxJS compatibles; Node 24 LTS para build; npm lockfile |
| UI | Formularios accesibles, navegación modular; una biblioteca visual mantenida compatible, sin cambiar menú |
| Tests | testing/httptest de Go; Angular y navegador para flujos; unittest Python existente |
| PDF | Generación servidor/worker ligera evaluada; no hacer del PDF navegador/base64 la fuente persistida |

El agente fija versiones exactas en go.mod/go.sum y package-lock.json tras una instalación/build/prueba. No copiar el antiguo supuesto Angular 22 + TypeScript 6 sin validar la matriz vigente. No prereleases ni upgrade automático de majors. Registrar selección/razón en decisions.md. Si una biblioteca no cumple MySQL 5.7, seguridad o memoria, escoger alternativa compatible y documentar.

Go 1.27 y el SDK MCP oficial fueron contrastados con sus fuentes oficiales el 9 de octubre de 2026. Consultar [Go](https://go.dev/doc/devel/release), [matriz Angular](https://angular.dev/reference/versions), [SDK MCP](https://github.com/modelcontextprotocol/go-sdk), [driver MySQL](https://github.com/go-sql-driver/mysql) y docs del paquete seleccionado. Versiones y condiciones de cuenta no se deducen del chat.

OAuth/MCP externo bloqueado hasta comprobar proveedor y cliente; esto no bloquea REST, Core, módulos ni pruebas locales con identidades de servicio limitadas. No exponer /mcp sin validación de identidad, Origin, permisos, audiencia cuando use OAuth y límites.
