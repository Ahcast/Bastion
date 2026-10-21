use std::{env, fmt, net::SocketAddr, path::PathBuf};
use ipnet::IpNet;

pub struct AppConfig {
    pub listen_addr: SocketAddr,
    pub cors_allowed_origin: String,
    pub database: DatabaseConfig,
    pub migrator_database: DatabaseConfig,
    pub bootstrap_secret: String,
    pub data_encryption_key: [u8; 32],
    pub trusted_proxy_cidrs: Vec<IpNet>,
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

#[derive(Debug)]
pub struct ConfigError {
    variable: &'static str,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "missing or invalid environment variable: {}", self.variable)
    }
}

impl std::error::Error for ConfigError {}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let listen_addr = match env::var("BASTION_LISTEN_ADDR") {
            Ok(value) => value.parse().map_err(|_| ConfigError { variable: "BASTION_LISTEN_ADDR" })?,
            Err(env::VarError::NotPresent) => SocketAddr::from(([127, 0, 0, 1], 8080)),
            Err(_) => return Err(ConfigError { variable: "BASTION_LISTEN_ADDR" }),
        };
        let cors_allowed_origin = required_trimmed("BASTION_CORS_ALLOWED_ORIGIN")?;
        let host = required_trimmed("MYSQL_HOST")?;
        let port = optional_port()?;
        let database = required_trimmed("MYSQL_DATABASE")?;
        let username = required_trimmed("MYSQL_USER")?;
        let password = required("MYSQL_PASSWORD")?;
        if password.trim().is_empty() {
            return Err(ConfigError { variable: "MYSQL_PASSWORD" });
        }
        let tls_ca = PathBuf::from(required_trimmed("MYSQL_TLS_CA")?);
        if !tls_ca.is_file() {
            return Err(ConfigError { variable: "MYSQL_TLS_CA" });
        }
        let bootstrap_secret = required("BASTION_BOOTSTRAP_SECRET")?;
        if bootstrap_secret.as_bytes().len() < 32 {
            return Err(ConfigError { variable: "BASTION_BOOTSTRAP_SECRET" });
        }
        let encoded_key = required_trimmed("BASTION_DATA_ENCRYPTION_KEY")?;
        let decoded_key = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded_key)
            .map_err(|_| ConfigError { variable: "BASTION_DATA_ENCRYPTION_KEY" })?;
        let data_encryption_key: [u8; 32] = decoded_key
            .try_into()
            .map_err(|_| ConfigError { variable: "BASTION_DATA_ENCRYPTION_KEY" })?;
        let trusted_proxy_cidrs=match env::var("BASTION_TRUSTED_PROXY_CIDRS"){
            Ok(raw)=>raw.split(',').map(str::trim).filter(|s|!s.is_empty()).map(|s|s.parse::<IpNet>().map_err(|_|ConfigError{variable:"BASTION_TRUSTED_PROXY_CIDRS"})).collect::<Result<Vec<_>,_>>()?,
            Err(env::VarError::NotPresent)=>Vec::new(),
            Err(_)=>return Err(ConfigError{variable:"BASTION_TRUSTED_PROXY_CIDRS"}),
        };

        let database_config=DatabaseConfig { host, port, username, password, database, tls_ca };
        let mut migrator_database=database_config.clone();
        migrator_database.username=required_trimmed("MYSQL_MIGRATOR_USER")?;
        migrator_database.password=required("MYSQL_MIGRATOR_PASSWORD")?;
        if migrator_database.password.trim().is_empty(){return Err(ConfigError{variable:"MYSQL_MIGRATOR_PASSWORD"});}

        Ok(Self {
            listen_addr,
            cors_allowed_origin,
            database: database_config,
            migrator_database,
            bootstrap_secret,
            data_encryption_key,
            trusted_proxy_cidrs,
        })
    }
}

fn required(name: &'static str) -> Result<String, ConfigError> {
    env::var(name).map_err(|_| ConfigError { variable: name })
}

fn required_trimmed(name: &'static str) -> Result<String, ConfigError> {
    let value = required(name)?;
    let trimmed = value.trim();
    if trimmed.is_empty() { return Err(ConfigError { variable: name }); }
    Ok(trimmed.to_owned())
}

fn optional_port() -> Result<u16, ConfigError> {
    match env::var("MYSQL_PORT") {
        Ok(value) => value.parse().ok().filter(|port| *port > 0).ok_or(ConfigError { variable: "MYSQL_PORT" }),
        Err(env::VarError::NotPresent) => Ok(3306),
        Err(_) => Err(ConfigError { variable: "MYSQL_PORT" }),
    }
}
