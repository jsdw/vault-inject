use std::str::FromStr;
use anyhow::{ anyhow, Result, Context };
use serde_json::Value;
use crate::client::Client;

/// A mapping from secret to environment variable
#[derive(Clone,PartialEq,Debug)]
pub struct SecretMapping {
    pub secret: Secret,
    pub secret_processors: Vec<String>,
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

        let (secret_str, secret_processor_strs) = secret_str_bits
            .split_first()
            .ok_or_else(|| anyhow!("Expected secret values of the form 'path/to/secret/key [| command ...]' but got '{}'", secret_str))?;

        let secret = Secret::from_str(secret_str)
            .with_context(|| format!("Could not parse '{}' into a valid secret path", secret_str))?;
        let env_var = env_var_str.to_owned();
        let secret_processors = secret_processor_strs
            .iter()
            .map(|&s| s.to_owned())
            .collect();

        Ok(SecretMapping { secret, env_var, secret_processors })
    }
}

#[derive(Clone,PartialEq,Debug)]

pub enum Secret {
    KV1(KV1),
    KV2(KV2),
    Cubbyhole(Cubbyhole)
}

impl FromStr for Secret {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Secret> {
        static KV1_PREFIX: &str = "kv1://";
        static KV2_PREFIX: &str = "kv2://";
        static CUBBYHOLE_PREFIX: &str = "cubbyhole://";

        // normalise beginning:
        let mut s = s.trim_start_matches('/');

        // complain if path ends in '/' (for now; so that we can use it to return all secrets later):
        if s.ends_with('/') {
            return Err(anyhow!("Secret paths should not end in '/' but '{}' does", s));
        }

        // base secret type on path prefix. Assume we are looking for a single key.
        if s.starts_with(KV2_PREFIX) {
            s = &s[KV2_PREFIX.len()..];
            let (path, key) = split_secret_path_and_key(s)?;
            Ok(Secret::KV2(KV2{
                path: path.to_owned(),
                key: key.to_owned()
            }))
        } else if s.starts_with(KV1_PREFIX) {
            s = &s[KV1_PREFIX.len()..];
            let (path, key) = split_secret_path_and_key(s)?;
            Ok(Secret::KV1(KV1{
                path: path.to_owned(),
                key: key.to_owned()
            }))
        } else if s.starts_with(CUBBYHOLE_PREFIX) {
            s = &s[CUBBYHOLE_PREFIX.len()..];
            let (path, key) = split_secret_path_and_key(s)?;
            Ok(Secret::Cubbyhole(Cubbyhole{
                path: path.to_owned(),
                key: key.to_owned()
            }))
        } else {
            Err(anyhow!("'{}' does not start with one of '{}', '{}' or '{}", s, KV1_PREFIX, KV2_PREFIX, CUBBYHOLE_PREFIX))
        }
    }
}

#[derive(Clone,PartialEq,Debug)]
pub struct KV1 {
    path: String,
    key: String
}

#[derive(Clone,PartialEq,Debug)]
pub struct KV2 {
    path: String,
    key: String
}

#[derive(Clone,PartialEq,Debug)]
pub struct Cubbyhole {
    path: String,
    key: String
}

/// Acquire a secret:
pub async fn fetch_secret(client: &Client, secret: &Secret) -> Result<String> {
    match secret {
        Secret::KV1(props) => {
            let res = request_secret_at_path(client, "/secret", &props.path).await?;
            let secret = res["data"][&props.key]
                .as_str()
                .ok_or_else(|| anyhow!("Could not find the secret '{}' at path '/{}' in KV1 store", &props.key, &props.path))?
                .to_owned();
            Ok(secret)
        },
        Secret::KV2(props) => {
            let res = request_secret_at_path(client, "/secret/data", &props.path).await?;
            let secret = res["data"]["data"][&props.key]
                .as_str()
                .ok_or_else(|| anyhow!("Could not find the secret '{}' at path '/{}' in KV2 store", &props.key, &props.path))?
                .to_owned();
            Ok(secret)

        },
        Secret::Cubbyhole(props) => {
            let res = request_secret_at_path(client, "/cubbyhole", &props.path).await?;
            let secret = res["data"][&props.key]
                .as_str()
                .ok_or_else(|| anyhow!("Could not find the secret '{}' at path '/{}' in cubbyhole store", &props.key, &props.path))?
                .to_owned();
            Ok(secret)
        },
    }
}

fn join_paths(path1: &str, path2: &str) -> String {
    format!("{}/{}",
        path1.trim_end_matches('/'),
        path2.trim_start_matches('/')
    )
}

async fn request_secret_at_path(client: &Client, prefix: &str, path: &str) -> Result<Value> {
    let res: Value = client.get(join_paths(prefix, path))
        .await
        .with_context(|| format!("Could not deserialize secrets at path '/{}' to JSON", &path))?;
    Ok(res)
}

fn split_secret_path_and_key(s: &str) -> Result<(&str, &str)> {
    let idx = s.rfind('/')
        .ok_or_else(|| anyhow!("Secret path should point to a single secret key, not just a path to a set of keys"))?;
    Ok((&s[0..idx], &s[idx+1..]))
}