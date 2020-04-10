use std::str::FromStr;
use std::collections::HashMap;
use anyhow::{ anyhow, Result, Context };
use serde_json::Value;
use serde::{ Deserialize };
use crate::client::Client;

pub struct SecretStore {
    // Client to make requests with:
    client: Client,
    // list of mount points and storage types for each:
    mount_points: Vec<(StorageType,String)>
}

impl SecretStore {

    /// Create a new SecretStore instance that knows about the
    /// available secret mount points
    pub async fn new(client: Client) -> Result<SecretStore> {
        // This API route is "internal", but the Vault CLI tool uses it
        // to find mount points, and we do too, because /sys/mounts requires
        // more permissions:
        let mut sys_auth: Value = client.get("/sys/internal/ui/mounts")
            .await
            .with_context(|| format!("Failed to get secret store information from Vault"))?;

        #[derive(Deserialize)]
        struct SysMountsData {
            r#type: String
        }
        let secret_mounts: HashMap<String,SysMountsData> = serde_json::from_value(sys_auth["data"]["secret"].take())
            .with_context(|| format!("Failed to get secret store information from Vault (unexpected response)"))?;

        let mount_points = secret_mounts
            .into_iter()
            .filter_map(|(mount,props)| {
                let ty = StorageType::from_str(&props.r#type).ok()?;
                let mount = mount.trim_matches('/').to_owned();
                Some((ty, mount))
            })
            .collect();

        Ok(SecretStore { client, mount_points })
    }

    /// Given some path, obtain the secrets pointed to
    pub async fn get(&self, original_path: &str) -> Result<Vec<(String,String)>> {
        let storage_type_and_path = original_path.trim_start_matches('/');
        let (storage_type, mount_point, path) = self.split_path(storage_type_and_path)
            .ok_or_else(|| anyhow!(
                "The path '/{}' is not supported (no known secret storage is mounted here)"
                , original_path))?;

        match storage_type {
            StorageType::KV => {
                let api_path = format!("{mount}/data/{path}"
                    , mount = mount_point
                    , path = path );

                let res: Value = self.client.get(&api_path)
                    .await
                    .with_context(|| format!(
                        "Could not find any secrets at path '/{}' from KV2 store mounted at '/{}'"
                        , &path, &mount_point))?;

                let secret = to_keyvalues(&res["data"]["data"])?;
                Ok(secret)
            },
            StorageType::Cubbyhole => {
                let api_path = format!("{mount}/{path}"
                    , mount = mount_point
                    , path = path );

                let res: Value = self.client.get(&api_path)
                    .await
                    .with_context(|| format!(
                        "Could not find any secrets at path '/{}' from Cubbyhole store mounted at '/{}'"
                        , &path, &mount_point))?;

                let secret = to_keyvalues(&res["data"])?;
                Ok(secret)
            },
        }
    }

    /// Resolve a path into the storage type used for it and the remaining
    /// path to the secret. The remaining path has no leading '/'.
    fn split_path<'s,'a>(&'s self, path: &'a str) -> Option<(StorageType,&'s str,&'a str)> {
        let path = path.trim_start_matches('/');
        for (ty,mount_path) in &self.mount_points {
            if path.starts_with(mount_path) {
                let path = path[mount_path.len()..].trim_start_matches('/');
                return Some((*ty,&**mount_path,path));
            }
        }
        None
    }
}

fn to_keyvalues(value: &Value) -> Result<Vec<(String,String)>> {
    let obj = value.as_object()
        .ok_or_else(|| anyhow!("Expected to find an object containing key/value pairs but got '{}'", value))?;
    let mut out = Vec::new();
    for (key, val) in obj {
        let val_str = val.as_str()
            .ok_or_else(|| anyhow!("The value for '{}' is not a string; is '{}'", key, val))?;
        out.push((key.to_owned(), val_str.to_owned()));
    }
    Ok(out)
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
