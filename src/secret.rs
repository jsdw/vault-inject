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

    /// Given some path, obtain the secret pointed to
    pub async fn get(&self, original_path: &str) -> Result<String> {
        let original_path = original_path.trim_start_matches('/');
        let (storage_type_and_path, key) = split_secret_path_and_key(original_path)
            .ok_or_else(|| anyhow!(
                "The path '/{}' does not appear to be valid (it should not end in '/')"
                , original_path))?;
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

                let secret = res["data"]["data"][&key]
                    .as_str()
                    .ok_or_else(|| anyhow!(
                        "Could not find the field '{}' in the secrets '/{}' in KV2 store mounted at '/{}'"
                        , &key, &path, &mount_point))?
                    .to_owned();
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

                let secret = res["data"][&key]
                    .as_str()
                    .ok_or_else(|| anyhow!(
                        "Could not find the field '{}' in the secrets '/{}' in Cubbyhole store mounted at '/{}'"
                        , &key, &path, &mount_point))?
                    .to_owned();
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

        for (idx,cmd) in secret_str_bits.iter().enumerate() {
            if cmd.is_empty() {
                let n = idx+1;
                return Err(anyhow!("Every '|' must forward to a command, but command {} of '{}' is missing", n, s))
            }
        }

        let (&path_str, processor_strs) = secret_str_bits
            .split_first()
            .ok_or_else(|| anyhow!("Expected secret values of the form 'path/to/secret/key [| command ...]' but got '{}'", secret_str))?;

        let path = path_str.trim_start_matches('/').to_owned();
        let env_var = env_var_str.to_owned();
        let processors = processor_strs
            .iter()
            .map(|&s| s.to_owned())
            .collect();

        Ok(SecretMapping { path, env_var, processors })
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_string_to_secretmapping() {

        let cases = vec![
            // ###############
            // ### Allowed ###
            // ###############

            // Basic paths (we strip leading but not trailing '/'):
            ("FOO = hello", Some(("FOO", "hello", vec![]))),
            ("FOO = /hello", Some(("FOO", "hello", vec![]))),
            ("FOO = /hello/", Some(("FOO", "hello/", vec![]))),
            ("FOO = /hello/foo/bar", Some(("FOO", "hello/foo/bar", vec![]))),
            ("FOO = /hello/foo/bar/", Some(("FOO", "hello/foo/bar/", vec![]))),
            ("FOO = /hello/foo/bar/", Some(("FOO", "hello/foo/bar/", vec![]))),
            // We ignore various whitespace:
            ("FOO= /hello/foo/bar/ ", Some(("FOO", "hello/foo/bar/", vec![]))),
            ("FOO=/hello/foo/bar/ ", Some(("FOO", "hello/foo/bar/", vec![]))),
            (" FOO=/hello/foo/bar/ ", Some(("FOO", "hello/foo/bar/", vec![]))),
            // We allow secrets to be piped through commands:
            ("FOO= /hello/foo/bar/ | base64", Some(("FOO", "hello/foo/bar/", vec!["base64"]))),
            ("FOO= /hello/foo/bar/ | base64 | rev", Some(("FOO", "hello/foo/bar/", vec!["base64", "rev"]))),
            ("FOO=/hello/foo/bar|base64|rev", Some(("FOO", "hello/foo/bar", vec!["base64", "rev"]))),
            ("FOO=/hello/foo/bar|base64| rev ", Some(("FOO", "hello/foo/bar", vec!["base64", "rev"]))),

            // ###################
            // ### NOT Allowed ###
            // ###################

            // You must have a path:
            ("FOO", None),
            // You must use '='
            ("FOO /hello/lark", None),
            // You can't have empty commands:
            ("FOO = /hello/lark |", None),
            ("FOO = /hello/lark ||", None),
            ("FOO = /hello/lark ||rev", None),
        ];

        for (s, res) in cases {
            match res {
                Some((env, path, processors)) => {
                    let processor_strings: Vec<String> = processors
                        .into_iter()
                        .map(|s: &str| s.to_owned())
                        .collect();
                    let mapping = match SecretMapping::from_str(s) {
                        Ok(mapping) => mapping,
                        Err(err) => panic!("String '{}' is not a valid SecretMapping: {:?}", s, err)
                    };
                    assert_eq!(env.to_owned(), mapping.env_var, "Environment variable doesn't match expected");
                    assert_eq!(path.to_owned(), mapping.path, "Path doesn't match expected");
                    assert_eq!(processor_strings, mapping.processors, "Piped commands don't match expected");
                },
                None => {
                    if let Ok(mapping) = SecretMapping::from_str(s) {
                        panic!("Did not expect '{}' to be valid, but it was: {:?}", s, mapping);   
                    }
                }
            }
        }

    }

}