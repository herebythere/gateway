use http::Uri;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use tokio::fs;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub key_filepath: PathBuf,
    pub cert_filepath: PathBuf,
    pub addresses: HashMap<String, String>,
}

pub enum ConfigError<'a> {
    IoError(std::io::Error),
    JsonError(serde_json::Error),
    UriError(<http::Uri as TryFrom<String>>::Error),
    Error(&'a str),
}

impl fmt::Display for ConfigError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::IoError(io_error) => write!(f, "{}", io_error),
            ConfigError::UriError(json_error) => write!(f, "{}", json_error),
            ConfigError::JsonError(json_error) => write!(f, "{}", json_error),
            ConfigError::Error(error) => write!(f, "{}", error),
        }
    }
}

pub async fn from_filepath(filepath: &PathBuf) -> Result<Config, ConfigError> {
    // get position relative to working directory
    let config_pathbuff = match filepath.canonicalize() {
        Ok(pb) => pb,
        Err(e) => return Err(ConfigError::IoError(e)),
    };

    let parent_dir = match config_pathbuff.parent() {
        Some(p) => p.to_path_buf(),
        _ => return Err(ConfigError::Error("parent directory of config not found")),
    };

    // build json conifg
    let json_as_str = match fs::read_to_string(&config_pathbuff).await {
        Ok(r) => r,
        Err(e) => return Err(ConfigError::IoError(e)),
    };
    let config: Config = match serde_json::from_str(&json_as_str) {
        Ok(j) => j,
        Err(e) => return Err(ConfigError::JsonError(e)),
    };

    // create absolute filepaths for key and cert
    let key = match parent_dir.join(&config.key_filepath).canonicalize() {
        Ok(j) => j,
        Err(e) => return Err(ConfigError::IoError(e)),
    };
    if key.is_dir() {
        return Err(ConfigError::Error(
            "config did not include an existing key file",
        ));
    }

    let cert = match parent_dir.join(&config.cert_filepath).canonicalize() {
        Ok(j) => j,
        Err(e) => return Err(ConfigError::IoError(e)),
    };
    if cert.is_dir() {
        return Err(ConfigError::Error(
            "config did not include an existing cert file",
        ));
    }

    Ok(Config {
        host: config.host,
        port: config.port,
        key_filepath: key,
        cert_filepath: cert,
        addresses: config.addresses,
    })
}

// Map<URI host, destination URI>.
// ie: Map<example.com, http://some_address:6789>
pub fn create_address_map(config: &Config) -> Result<HashMap<String, Uri>, ConfigError> {
    let mut hashmap = HashMap::<String, Uri>::new();
    for (arrival_str, dest_str) in &config.addresses {
        let arrival_uri = match Uri::try_from(arrival_str) {
            Ok(uri) => uri,
            Err(e) => return Err(ConfigError::UriError(e)),
        };

        let host = match arrival_uri.host() {
            Some(uri) => uri,
            _ => return Err(ConfigError::Error("could not parse hosts from addresses")),
        };

        // no need to remove path and query, it is replaced later
        let dest_uri = match Uri::try_from(dest_str) {
            Ok(uri) => uri,
            Err(e) => return Err(ConfigError::UriError(e)),
        };

        hashmap.insert(host.to_string(), dest_uri);
    }

    Ok(hashmap)
}
