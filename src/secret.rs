use std::str::FromStr;
use std::collections::HashMap;
use anyhow::{ anyhow, Result, Context };
use serde_json::Value;
use serde::{ Deserialize };
use crate::client::Client;

pub struct SecretStore {
    // Client to make requests with:
    client: Client,
    // map of mount_point => storage_type for supported secret stores:
    mount_points: HashMap<StorageType,String>
}

impl SecretStore {

    /// Create a new SecretStore instance that knows about the
    /// available secret mount points
    pub async fn new(client: Client) -> Result<SecretStore> {
        #[derive(Deserialize)]
        struct SysMounts {
            data: HashMap<String,SysMountsData>
        }
        #[derive(Deserialize)]
        struct SysMountsData {
            r#type: String
        }

        let sys_auth: SysMounts = client.get("/sys/mounts")
            .await
            .with_context(|| anyhow!("Failed to get secret store information from Vault"))?;

        let mount_points = sys_auth.data
            .into_iter()
            .filter_map(|(mount,props)| {
                let ty = StorageType::from_str(&props.r#type).ok()?;
                let mount = mount.trim_matches('/').to_owned();
                Some((ty, mount))
            })
            .collect();

        Ok(SecretStore { client, mount_points })
    }

    /// Given some path, obtain the secret pointed to
    pub async fn get(&self, original_path: &str) -> Result<String> {
        let original_path = original_path.trim_start_matches('/');
        let (storage_type_and_path, key) = split_secret_path_and_key(original_path)
            .ok_or_else(|| anyhow!("The provided path does not appear to be valid (it should not end in '/')"))?;
        let (storage_type, path) = self.split_path(storage_type_and_path)
            .ok_or_else(|| anyhow!("The provided path is not supported (no known secret storage is mounted here)"))?;

        match storage_type {
            StorageType::KV => {
                let mount_point = self.mount_points
                    .get(&StorageType::KV)
                    .ok_or_else(|| anyhow!("Key-Value secret storage is not enabled in Vault"))?;

                let api_path = format!("{mount}/data/{path}"
                    , mount = mount_point
                    , path = path );

                let res: Value = self.client.get(&api_path)
                    .await
                    .with_context(|| format!("Could not get secrets at path '/{}' from KV2 store", &path))?;

                let secret = res["data"]["data"][&key]
                    .as_str()
                    .ok_or_else(|| anyhow!("Could not find the secret '{}' at path '/{}' in KV2 store", &key, &path))?
                    .to_owned();
                Ok(secret)
            },
            StorageType::Cubbyhole => {
                let mount_point = self.mount_points
                    .get(&StorageType::Cubbyhole)
                    .ok_or_else(|| anyhow!("Cubbyhole secret storage is not enabled in Vault"))?;

                let api_path = format!("{mount}/{path}"
                    , mount = mount_point
                    , path = path );

                let res: Value = self.client.get(&api_path)
                    .await
                    .with_context(|| format!("Could not get secrets at path '/{}' from Cubbyhole store", &path))?;

                let secret = res["data"][&key]
                    .as_str()
                    .ok_or_else(|| anyhow!("Could not find the secret '{}' at path '/{}' in Cubbyhole store", &key, &path))?
                    .to_owned();
                Ok(secret)
            },
        }
    }

    /// Resolve a path into the storage type used for it and the remaining
    /// path to the secret. The remaining path has no leading '/'.
    fn split_path<'a>(&self, path: &'a str) -> Option<(StorageType,&'a str)> {
        let path = path.trim_start_matches('/');
        for (&ty,mount_path) in &self.mount_points {
            if path.starts_with(mount_path) {
                let path = path[mount_path.len()..].trim_start_matches('/');
                return Some((ty,path));
            }
        }
        None
    }
}

fn split_secret_path_and_key(s: &str) -> Option<(&str, &str)> {
    let idx = s.rfind('/')?;
    Some((&s[0..idx], &s[idx+1..]))
}


/// The supported secret storage types
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum StorageType {
    KV,
    Cubbyhole
}

impl FromStr for StorageType {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "kv" => Ok(StorageType::KV),
            "cubbyhole" => Ok(StorageType::Cubbyhole),
            _ => Err(anyhow!("'{}' is not a supported storage type", s))
        }
    }
}

/// A mapping from secret to environment variable
#[derive(Clone,PartialEq,Debug)]
pub struct SecretMapping {
    pub path: String,
    pub processors: Vec<String>,
    pub env_var: String,
}

impl FromStr for SecretMapping {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<SecretMapping> {
        let idx = s.find('=')
            .ok_or_else(|| anyhow!("Expected secrets of the form 'ENV_VAR=path/to/secret/key' but got '{}'", s))?;

        let env_var_str = s[0..idx].trim();
        let secret_str = &s[idx+1..];

        let secret_str_bits = secret_str
            .split('|')
            .map(|s| s.trim())
            .collect::<Vec<_>>();

        let (&path_str, processor_strs) = secret_str_bits
            .split_first()
            .ok_or_else(|| anyhow!("Expected secret values of the form 'path/to/secret/key [| command ...]' but got '{}'", secret_str))?;

        let path = path_str.to_owned();
        let env_var = env_var_str.to_owned();
        let processors = processor_strs
            .iter()
            .map(|&s| s.to_owned())
            .collect();

        Ok(SecretMapping { path, env_var, processors })
    }
}
