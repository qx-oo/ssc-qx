use failure;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerConfig {
    host: SocketAddr,
    password: String,
    encrypt_method: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    host: SocketAddr,
    server_list: Vec<ServerConfig>,
}

impl Config {
    pub fn from_file(file: &mut File) -> Result<Config, failure::Error> {
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        let cfg = serde_json::from_str(&buf)?;
        Ok(cfg)
    }
    pub fn host(&self) -> &SocketAddr {
        &self.host
    }
    pub fn server_list(&self) -> &Vec<ServerConfig> {
        &self.server_list
    }
}

impl ServerConfig {
    pub fn host(&self) -> &SocketAddr {
        &self.host
    }
    pub fn password(&self) -> &str {
        &self.password
    }
    pub fn encrypt_method(&self) -> &str {
        &self.encrypt_method
    }
}
