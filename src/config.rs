use failure;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;

#[derive(Debug)]
pub struct ServerAddr(pub String);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoteConfig {
    host: SocketAddr,
    password: String,
    encrypt_method: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    host: SocketAddr,
    server_list: Vec<RemoteConfig>,
    udp: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerConfig {
    host: SocketAddr,
    udp: bool,
}

// fn read_json_cfg<'de, T>(file: &mut File) -> Result<(), failure::Error>
// where
//     T: de::Deserialize<'de>,
// {
//     let mut buf = String::new();
//     file.read_to_string(&mut buf)?;
//     let cfg: T = serde_json::from_str(&buf.clone())?;
//     // Ok(cfg)
//     Ok(())
// }

// trait ReadConfig {
//     fn from_file(file: &mut File) -> Result<Box<dyn Self>, failure::Error>;
// }

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
    pub fn server_list(&self) -> &Vec<RemoteConfig> {
        &self.server_list
    }
    pub fn udp(&self) -> bool {
        self.udp
    }
}

impl ServerConfig {
    pub fn from_file(file: &mut File) -> Result<ServerConfig, failure::Error> {
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        let cfg = serde_json::from_str(&buf)?;
        Ok(cfg)
    }
    pub fn host(&self) -> &SocketAddr {
        &self.host
    }
    pub fn udp(&self) -> bool {
        self.udp
    }
}

impl RemoteConfig {
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
