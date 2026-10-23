use ipnet::IpNet;
use std::{env, fmt, net::SocketAddr, path::PathBuf};

pub struct AppConfig {
    pub listen_addr: SocketAddr,
    pub cors_allowed_origin: String,
    pub database: DatabaseConfig,
    pub bootstrap_secret: String,
    pub data_encryption_key: [u8; 32],
    pub trusted_proxy_cidrs: Vec<IpNet>,
    pub procesos_allowed: bool,
    pub cotizaciones_allowed: bool,
    pub clientes_allowed: bool,
    pub facturas_allowed: bool,
    pub ordenes_compra_allowed: bool,
}

#[derive(Clone)]
pub struct DatabaseConfig {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) database: String,
    pub(crate) tls_ca: PathBuf,
}

impl DatabaseConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::with_credentials_from_env("MYSQL_USER", "MYSQL_PASSWORD")
    }

    pub fn migrator_from_env() -> Result<Self, ConfigError> {
        Self::with_credentials_from_env("MYSQL_MIGRATOR_USER", "MYSQL_MIGRATOR_PASSWORD")
    }

    fn with_credentials_from_env(
        user_variable: &'static str,
        password_variable: &'static str,
    ) -> Result<Self, ConfigError> {
        let host = required_trimmed("MYSQL_HOST")?;
        let port = optional_port()?;
        let database = required_trimmed("MYSQL_DATABASE")?;
        let username = required_trimmed(user_variable)?;
        let password = required(password_variable)?;
        if password.trim().is_empty() {
            return Err(ConfigError {
                variable: password_variable,
            });
        }
        let tls_ca = PathBuf::from(required_trimmed("MYSQL_TLS_CA")?);
        if !tls_ca.is_file() {
            return Err(ConfigError {
                variable: "MYSQL_TLS_CA",
            });
        }
        Ok(Self {
            host,
            port,
            username,
            password,
            database,
            tls_ca,
        })
    }
}

#[derive(Debug)]
pub struct ConfigError {
    variable: &'static str,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "missing or invalid environment variable: {}",
            self.variable
        )
    }
}

impl std::error::Error for ConfigError {}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let listen_addr = match env::var("BASTION_LISTEN_ADDR") {
            Ok(value) => value.parse().map_err(|_| ConfigError {
                variable: "BASTION_LISTEN_ADDR",
            })?,
            Err(env::VarError::NotPresent) => SocketAddr::from(([127, 0, 0, 1], 8080)),
            Err(_) => {
                return Err(ConfigError {
                    variable: "BASTION_LISTEN_ADDR",
                });
            }
        };
        let cors_allowed_origin = required_trimmed("BASTION_CORS_ALLOWED_ORIGIN")?;
        let database_config = DatabaseConfig::from_env()?;
        let bootstrap_secret = required("BASTION_BOOTSTRAP_SECRET")?;
        if bootstrap_secret.as_bytes().len() < 32 {
            return Err(ConfigError {
                variable: "BASTION_BOOTSTRAP_SECRET",
            });
        }
        let encoded_key = required_trimmed("BASTION_DATA_ENCRYPTION_KEY")?;
        let decoded_key =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded_key)
                .map_err(|_| ConfigError {
                    variable: "BASTION_DATA_ENCRYPTION_KEY",
                })?;
        let data_encryption_key: [u8; 32] = decoded_key.try_into().map_err(|_| ConfigError {
            variable: "BASTION_DATA_ENCRYPTION_KEY",
        })?;
        let trusted_proxy_cidrs = match env::var("BASTION_TRUSTED_PROXY_CIDRS") {
            Ok(raw) => raw
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| {
                    s.parse::<IpNet>().map_err(|_| ConfigError {
                        variable: "BASTION_TRUSTED_PROXY_CIDRS",
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
            Err(env::VarError::NotPresent) => Vec::new(),
            Err(_) => {
                return Err(ConfigError {
                    variable: "BASTION_TRUSTED_PROXY_CIDRS",
                });
            }
        };

        let allowed_modules = match env::var("BASTION_ALLOWED_MODULES") {
            Ok(value) => parse_allowed_modules(&value)?,
            Err(env::VarError::NotPresent) => Vec::new(),
            Err(_) => {
                return Err(ConfigError {
                    variable: "BASTION_ALLOWED_MODULES",
                });
            }
        };
        Ok(Self {
            listen_addr,
            cors_allowed_origin,
            database: database_config,
            bootstrap_secret,
            data_encryption_key,
            trusted_proxy_cidrs,
            procesos_allowed: allowed_modules.contains(&"procesos"),
            cotizaciones_allowed: allowed_modules.contains(&"cotizaciones"),
            clientes_allowed: allowed_modules.contains(&"clientes"),
            facturas_allowed: allowed_modules.contains(&"facturas"),
            ordenes_compra_allowed: allowed_modules.contains(&"ordenes_compra"),
        })
    }
}

fn parse_allowed_modules(value: &str) -> Result<Vec<&'static str>, ConfigError> {
    if value.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut modules = Vec::new();
    for module in value.split(',').map(str::trim) {
        let module = match module {
            "procesos" => "procesos",
            "cotizaciones" => "cotizaciones",
            "clientes" => "clientes",
            "facturas" => "facturas",
            "ordenes_compra" => "ordenes_compra",
            _ => {
                return Err(ConfigError {
                    variable: "BASTION_ALLOWED_MODULES",
                });
            }
        };
        if modules.contains(&module) {
            return Err(ConfigError {
                variable: "BASTION_ALLOWED_MODULES",
            });
        }
        modules.push(module);
    }
    Ok(modules)
}

#[cfg(test)]
mod module_tests {
    use super::parse_allowed_modules;

    #[test]
    fn allowlist_is_explicit_and_preserves_core_only() {
        assert!(parse_allowed_modules("").unwrap().is_empty());
        assert_eq!(
            parse_allowed_modules(" procesos , cotizaciones ").unwrap(),
            vec!["procesos", "cotizaciones"]
        );
        assert_eq!(
            parse_allowed_modules("cotizaciones").unwrap(),
            vec!["cotizaciones"]
        );
        assert_eq!(parse_allowed_modules("clientes").unwrap(), vec!["clientes"]);
        assert_eq!(parse_allowed_modules("facturas").unwrap(), vec!["facturas"]);
        assert_eq!(
            parse_allowed_modules("ordenes_compra").unwrap(),
            vec!["ordenes_compra"]
        );
        for value in ["*", "unknown", "procesos,", "procesos,procesos"] {
            assert!(parse_allowed_modules(value).is_err());
        }
    }
}

fn required(name: &'static str) -> Result<String, ConfigError> {
    env::var(name).map_err(|_| ConfigError { variable: name })
}

fn required_trimmed(name: &'static str) -> Result<String, ConfigError> {
    let value = required(name)?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ConfigError { variable: name });
    }
    Ok(trimmed.to_owned())
}

fn optional_port() -> Result<u16, ConfigError> {
    match env::var("MYSQL_PORT") {
        Ok(value) => value
            .parse()
            .ok()
            .filter(|port| *port > 0)
            .ok_or(ConfigError {
                variable: "MYSQL_PORT",
            }),
        Err(env::VarError::NotPresent) => Ok(3306),
        Err(_) => Err(ConfigError {
            variable: "MYSQL_PORT",
        }),
    }
}
