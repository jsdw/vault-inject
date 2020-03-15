use anyhow::{ anyhow, Result, Context };
use serde_json::{ Value, json };
use serde::{ Serialize, Deserialize };
use std::str::FromStr;
use tokio::io::{ self, AsyncWriteExt, AsyncBufReadExt };
use tokio::task;
use crate::client::Client;


pub struct Auth {
    // Client to make requests with:
    client: Client
}

impl Auth {

    /// Create a new Auth instance that knows about the
    /// available auth capabilities
    pub fn new(client: Client) -> Auth {
        Auth { client }
    }

    /// Authenticate a user given the AuthDetails provided and return a token
    pub async fn login(&self, opts: AuthDetails) -> Result<String> {
        match opts {
            AuthDetails::Ldap { path, mut username, mut password } => {
                if username.is_empty() {
                    username = prompt_for_input("Please enter Vault LDAP username: ").await?;
                }
                if password.is_empty() {
                    password = prompt_for_hidden_input("Please enter Vault LDAP password: ").await?;
                }
                let path = path.unwrap_or_else(|| "ldap".to_owned());
                self.login_ldap(&path, &username, &password).await
            },
            AuthDetails::UserPass { path, mut username, mut password } => {
                if username.is_empty() {
                    username = prompt_for_input("Please enter Vault username: ").await?;
                }
                if password.is_empty() {
                    password = prompt_for_hidden_input("Please enter Vault password: ").await?;
                }
                let path = path.unwrap_or_else(|| "userpass".to_owned());
                self.login_userpass(&path, &username, &password).await
            },
            AuthDetails::Token { mut token } => {
                if token.is_empty() {
                    token = prompt_for_hidden_input("Please enter Vault token: ").await?;
                }
                Ok(token)
            }
        }
    }

    /// Login via LDAP (if configured in Vault)
    async fn login_ldap(&self, mount_path: &str, username: &str, password: &str)  -> Result<String> {
        let auth_path = format!("auth/{mount}/login/{username}"
            , mount = mount_path.trim_matches('/')
            , username = username );

        let res: Value = self.client.post(auth_path, &json!({ "password": password }))
            .await
            .with_context(|| format!("Could not complete LDAP login request to vault API"))?;

        let token = res["auth"]["client_token"]
            .as_str()
            .ok_or_else(|| anyhow!("Could not find the client token in the LDAP login response"))?;
        Ok(token.to_string())
    }

    /// Login via Username-Password (if configured in Vault)
    async fn login_userpass(&self, mount_path: &str, username: &str, password: &str)  -> Result<String> {
        let auth_path = format!("auth/{mount}/login/{username}"
            , mount = mount_path.trim_matches('/')
            , username = username );

        let res: Value = self.client.post(auth_path, &json!({ "password": password }))
            .await
            .with_context(|| format!("Could not complete Username-Password login request to vault API"))?;

        let token = res["auth"]["client_token"]
            .as_str()
            .ok_or_else(|| anyhow!("Could not find the client token in the Username-Password login response"))?;
        Ok(token.to_string())
    }

}

/// The details we need for each auth type in order to get a token
#[derive(PartialEq,Eq,Clone)]
pub enum AuthDetails {
    Ldap { path: Option<String>, username: String, password: String },
    UserPass { path: Option<String>, username: String, password: String },
    Token { token: String }
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

/// Currently available authentication methods
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
            _ => Err(anyhow!("'{}' is not a valid authentication type (try 'ldap', 'token' or 'userpass').", s))
        }
    }
}