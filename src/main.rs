mod auth;
mod secret;
mod client;
mod cache;

use crate::auth::{ Auth, AuthDetails, AuthType };
use crate::secret::{ SecretStore, SecretMapping };
use anyhow::{ anyhow, Result, Context };
use structopt::StructOpt;
use std::process::Stdio;
use tokio::process::Command;
use tokio::prelude::*;
use futures::stream::{ StreamExt, FuturesUnordered };
use colored::*;

#[derive(Debug,Clone,StructOpt)]
#[structopt(name="vault-inject", about = "Inject vault secrets into commands")]
struct Opts {
    /// The command you'd like to run, having secrets exposed to it via environment variables
    #[structopt(long="command", short="c")]
    command: String,

    /// Username to login with (for the 'ldap'/'userpass' auth-type)
    #[structopt(long="username", env="VAULT_INJECT_USERNAME")]
    username: Option<String>,

    /// Password to login with (for the 'ldap'/'userpass' auth-type)
    #[structopt(long="password", env="VAULT_INJECT_PASSWORD", hide_env_values=true)]
    password: Option<String>,

    /// Token to login with (for the 'token' auth-type)
    #[structopt(long="token", env="VAULT_INJECT_TOKEN", hide_env_values=true)]
    token: Option<String>,

    /// URL of your vault instance (eg https://vault.yourdomain)
    #[structopt(long="vault-url", default_value="http://localhost:8200", env="VAULT_ADDR")]
    vault_url: url::Url,

    /// Which type of authentication would you like to use with vault?
    #[structopt(long="auth-type", env="VAULT_INJECT_AUTH_TYPE")]
    auth_type: Option<AuthType>,

    /// If the authentication path is not the default, you'll need to provide it here
    #[structopt(long="auth-path", env="VAULT_INJECT_AUTH_PATH")]
    auth_path: Option<String>,

    /// Map secrets to environment variables. Call this once for each secret you'd like to inject
    #[structopt(short="s", long="secret")]
    secrets: Vec<SecretMapping>,

    /// Don't read from the cache
    #[structopt(long="no-cache-read")]
    no_cache_read: bool,

    /// Don't cache the auth token
    #[structopt(long="no-cache-write")]
    no_cache_write: bool,

    /// Don't cache the auth token, or try to load one from the cache
    #[structopt(long="no-cache")]
    no_cache: bool
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{}", format!("{:?}",e).yellow());
    }
}

async fn run() -> Result<()> {
    let opts = Opts::from_args();

    if opts.secrets.is_empty() {
        return Err(anyhow!("One or more secret mappings should be provided using '--secret'"));
    }

    let mut cache = cache::Cache::load().await?;
    let client = client::Client::new(opts.vault_url.clone());
    let auth = Auth::new(client.clone());
    let auth_details = to_auth_details(&opts);

    // Check and return the cached token if we didn't provide a token
    // and we didn't ask to not use the cache at all:
    let cached_token = if opts.no_cache || opts.no_cache_read || opts.token.is_some() {
        None
    } else if let Some(token) = cache.get_token() {
        let is_valid = auth.is_token_valid(&token).await;
        if is_valid { Some(token) } else { None }
    } else {
        None
    };

    // If no cached token, authenticate with Vault to get one:
    let auth_token = if let Some(token) = cached_token {
        token
    } else {
        let token = auth.login(auth_details.clone()).await?;
        if !opts.no_cache && !opts.no_cache_write {
            cache.set_token(token.clone());
            cache.save().await?;
        }
        token
    };

    // Make a new secret store to obtain secrets from:
    let store = SecretStore::new(client.with_token(auth_token)).await?;

    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(&opts.command);

    // Fetch all of our secrets and process env var commands:
    let mut mappings = FuturesUnordered::new();
    for secret_mapping in &opts.secrets {
        let store = &store;
        mappings.push(async move {
            let secret_value = store.get(&secret_mapping.path).await?;
            let secret_value = process_commands(secret_value, &secret_mapping.processors).await?;
            Ok::<_,anyhow::Error>((&secret_mapping.env_var, secret_value))
        })
    }

    // When the above finishes, we set the env var => value mnappings for the command:
    while let Some(res) = mappings.next().await {
        let (env_var, value) = res?;
        cmd.env(env_var, value);
    }

    // Run the command we've been given:
    cmd.spawn()
       .with_context(|| format!("Failed to run the command '{}'", &opts.command))?
       .await?;

    Ok(())
}

fn to_auth_details(opts: &Opts) -> AuthDetails {
    // If a token is provided, auth-type defaults to token,
    // else it defaults to username-password:
    let auth_type = match opts.auth_type {
        Some(auth_type) => auth_type,
        None => if opts.token.is_some() {
            AuthType::Token
        } else {
            AuthType::UserPass
        }
    };

    // Extract the details we need from opts based on the auth type:
    match auth_type {
        AuthType::Ldap => AuthDetails::Ldap {
            path:      opts.auth_path.clone(),
            username:  opts.username.clone().unwrap_or(String::new()),
            password:  opts.password.clone().unwrap_or(String::new())
        },
        AuthType::UserPass => AuthDetails::UserPass {
            path:      opts.auth_path.clone(),
            username:  opts.username.clone().unwrap_or(String::new()),
            password:  opts.password.clone().unwrap_or(String::new())
        },
        AuthType::Token => AuthDetails::Token {
            token: opts.token.clone().unwrap_or(String::new())
        },
    }
}

async fn process_commands(mut secret: String, commands: &[String]) -> Result<String> {
    for command in commands {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to run the command '{}'", command))?;

        {
            let stdin = child.stdin.as_mut()
                .with_context(|| format!("Failed to open stdin for the command '{}'", command))?;
            stdin.write_all(secret.as_bytes())
                .await
                .with_context(|| format!("Failed to write to stdin for the command '{}'", command))?;
        }

        let output = child.wait_with_output()
            .await
            .with_context(|| format!("Failed to read stdout for the command '{}'", command))?;
        secret = String::from_utf8_lossy(&output.stdout).into_owned();

        if secret.is_empty() {
            let error_output = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("The command '{}' failed:\n\n'{}'", command, error_output));
        }
    }
    Ok(secret)
}

