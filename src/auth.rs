use anyhow::{ anyhow, Result, Context };
use serde_json::{ Value, json };
use std::str::FromStr;
use tokio::io::{ self, AsyncWriteExt, AsyncBufReadExt };
use tokio::task;
use crate::client::Client;

/// Available authentication methods
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum AuthType {
    Ldap,
    UserPass,
    Token
}

// How to convert a string into the desired auth type
impl FromStr for AuthType {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "ldap" => Ok(AuthType::Ldap),
            "token" => Ok(AuthType::Token),
            "userpass" |
            "user-pass" |
            "username-password" |
            "username" |
            "user" => Ok(AuthType::UserPass),
            _ => Err(anyhow!("'{}' is not a valid authentication type (try 'ldap' or 'userpass').", s))
        }
    }
}

/// The details we need for each auth type in order to get a token
pub enum AuthDetails {
    Ldap { path: String, username: String, password: String },
    UserPass { path: String, username: String, password: String },
    Token { token: String }
}

/// Authenticate a user given the AuthDetails provided and return a token
pub async fn get_auth_token(client: &Client, opts: AuthDetails) -> Result<String> {
    match opts {
        AuthDetails::Ldap { mut path, mut username, mut password } => {
            if username.is_empty() {
                username = prompt_for_input("Please enter Vault LDAP username: ").await?;
            }
            if password.is_empty() {
                password = prompt_for_hidden_input("Please enter Vault LDAP password: ").await?;
            }
            if path.is_empty() {
                path = "/auth/ldap".to_owned();
            }
            ldap(client, path, username, password).await
        },
        AuthDetails::UserPass { mut path, mut username, mut password } => {
            if username.is_empty() {
                username = prompt_for_input("Please enter Vault username: ").await?;
            }
            if password.is_empty() {
                password = prompt_for_hidden_input("Please enter Vault password: ").await?;
            }
            if path.is_empty() {
                path = "/auth/userpass".to_owned();
            }
            userpass(client, path, username, password).await
        },
        AuthDetails::Token { mut token } => {
            if token.is_empty() {
                token = prompt_for_hidden_input("Please enter Vault token: ").await?;
            }
            Ok(token)
        }
    }
}

/// Prompt for input from stdin
async fn prompt_for_input(msg: &str) -> Result<String> {
    io::stderr().write_all(msg.as_bytes())
        .await
        .with_context(|| format!("Could not write to stdout"))?;
    let mut username = String::new();
    io::BufReader::new(io::stdin()).read_line(&mut username)
        .await
        .with_context(|| format!("Failed to read username from stdin"))?;
    Ok(username)
}

/// Prompt for password-like input (input is hidden)
async fn prompt_for_hidden_input(msg: &str) -> Result<String> {
    let msg = msg.to_owned();
    task::spawn_blocking(move || {
        rpassword::prompt_password_stderr(&msg)
            .with_context(|| format!("Failed to read password from stdin"))
    }).await?
}

/// Get an auth token via LDAP
async fn ldap(client: &Client, auth_path: String, username: String, password: String) -> Result<String> {
    let res: Value = client.post(auth_path, &json!({ "username": username, "password": password }))
        .await
        .with_context(|| format!("Could not complete LDAP login request to vault API"))?;
    let token = res["auth"]["client_token"]
        .as_str()
        .ok_or_else(|| anyhow!("Could not find the client token in the LDAP login response"))?;
    Ok(token.to_string())
}

/// Get an auth token via username-password authentication
async fn userpass(client: &Client, auth_path: String, username: String, password: String) -> Result<String> {
    let res: Value = client.post(auth_path, &json!({ "username": username, "password": password }))
        .await
        .with_context(|| format!("Could not deserialize username-password login response from vault API"))?;
    let token = res["auth"]["client_token"]
        .as_str()
        .ok_or_else(|| anyhow!("Could not find the client token in the username-password login response"))?;
    Ok(token.to_string())
}

