use anyhow::{ anyhow, Result, Context };
use reqwest::blocking as req;
use serde_json::{ Value, json };
use std::io::{ stderr, stdin, Write };
use std::str::FromStr;
use crate::utils::make_api_path;

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
    Ldap { vault_url: url::Url, path: String, username: String, password: String },
    UserPass { vault_url: url::Url, path: String, username: String, password: String },
    Token { token: String }
}

/// Authenticate a user given the AuthDetails provided and return a token
pub fn get_auth_token(opts: AuthDetails) -> Result<String> {
    match opts {
        AuthDetails::Ldap { vault_url, mut path, mut username, mut password } => {
            if username.is_empty() {
                username = prompt_for_input("Please enter Vault LDAP username: ")?;
            }
            if password.is_empty() {
                password = prompt_for_hidden_input("Please enter Vault LDAP password: ")?;
            }
            if path.is_empty() {
                path = "/auth/ldap".to_owned();
            }
            ldap(vault_url, path, username, password)
        },
        AuthDetails::UserPass { vault_url, mut path, mut username, mut password } => {
            if username.is_empty() {
                username = prompt_for_input("Please enter Vault username: ")?;
            }
            if password.is_empty() {
                password = prompt_for_hidden_input("Please enter Vault password: ")?;
            }
            if path.is_empty() {
                path = "/auth/userpass".to_owned();
            }
            userpass(vault_url, path, username, password)
        },
        AuthDetails::Token { mut token } => {
            if token.is_empty() {
                token = prompt_for_hidden_input("Please enter Vault token: ")?;
            }
            Ok(token)
        }
    }
}

/// Prompt for input from stdin
fn prompt_for_input(msg: &str) -> Result<String> {
    stderr().write_all(msg.as_bytes())
        .with_context(|| format!("Could not write to stdout"))?;
    let mut username = String::new();
    stdin().read_line(&mut username)
        .with_context(|| format!("Failed to read username from stdin"))?;
    Ok(username)
}

/// Prompt for password-like input (input is hidden)
fn prompt_for_hidden_input(msg: &str) -> Result<String> {
    rpassword::prompt_password_stderr(msg)
        .with_context(|| format!("Failed to read password from stdin"))
}

/// Get an auth token via LDAP
fn ldap(vault_url: url::Url, auth_path: String, username: String, password: String) -> Result<String> {
    let url = make_api_path(vault_url, &auth_path);
    let client = req::Client::builder().build()
        .with_context(|| format!("Could not instantiate client to talk to vault API"))?;
    let res: Value = client.post(url)
        .json(&json!({ "username": username, "password": password }))
        .send()
        .with_context(|| format!("Could not complete LDAP login request to vault API"))?
        .json()
        .with_context(|| format!("Could not deserialize LDAP login response from vault API"))?;
    let token = res["auth"]["client_token"]
        .as_str()
        .ok_or_else(|| anyhow!("Could not find the client token in the LDAP login response"))?;
    Ok(token.to_string())
}

/// Get an auth token via username-password authentication
fn userpass(vault_url: url::Url, auth_path: String, username: String, password: String) -> Result<String> {
    let url = make_api_path(vault_url, &auth_path);
    let client = req::Client::builder().build()
        .with_context(|| format!("Could not instantiate client to talk to vault API"))?;
    let res: Value = client.post(url)
        .json(&json!({ "username": username, "password": password }))
        .send()
        .with_context(|| format!("Could not complete username-password login request to vault API"))?
        .json()
        .with_context(|| format!("Could not deserialize username-password login response from vault API"))?;
    let token = res["auth"]["client_token"]
        .as_str()
        .ok_or_else(|| anyhow!("Could not find the client token in the username-password login response"))?;
    Ok(token.to_string())
}

