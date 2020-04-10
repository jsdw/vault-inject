mod auth;
mod secret_store;
mod secret_mapping;
mod template;
mod client;
mod cache;

use crate::auth::{ Auth, AuthDetails, AuthType };
use crate::secret_store::SecretStore;
use crate::secret_mapping::SecretMapping;
use anyhow::{ anyhow, Result, Context };
use structopt::StructOpt;
use std::process::Stdio;
use tokio::process::Command;
use tokio::prelude::*;
use tokio::runtime;
use futures::stream::{ StreamExt, FuturesUnordered };
use colored::*;

#[derive(Debug,Clone,StructOpt)]
#[structopt(name="vault-inject", about = "Inject vault secrets into commands")]
struct Opts {
    /// The command you'd like to run, having secrets exposed to it via environment variables
    #[structopt(long="command", short="c")]
    command: String,

    /// Run this command against each secret we obtain (which is exposed as the env var $secret)
    #[structopt(long="each")]
    each: Vec<String>,

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

fn main() {
    if let Err(e) = run() {
        use std::io::{ self, Write};
        let _ = io::stderr().write_all(format!("{:?}\n",e).yellow().to_string().as_bytes());
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut runtime = runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .with_context(|| format!("Unable to start async runtime"))?;
    runtime.block_on(async { run_async().await })
}

async fn run_async() -> Result<()> {
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
            let secret_values = store.get(secret_mapping.path()).await?;
            let mut out_values = Vec::new();
            for (key,val) in secret_values {
                if let Some(env_var) = secret_mapping.env_var_from_key(&key) {
                    let secret_value = process_commands(val.into_bytes(), secret_mapping.processors()).await?;
                    out_values.push((env_var, secret_value));
                }
            }
            Ok::<_,anyhow::Error>(out_values)
        })
    }

    // When the above finishes, we set the env var => value mappings for the command.
    // If 'each' command(s) are given, we also run these against each variable, one after
    // the other:
    while let Some(res) = mappings.next().await {
        for (key, val) in res? {
            cmd.env(&key, &val);
            for each_cmd_str in &opts.each {
                Command::new("sh")
                    .arg("-c")
                    .arg(each_cmd_str)
                    .env("secret", &val)
                    .env("secret_key", &key)
                    .env("secret_value", &val)
                    .spawn()
                    .with_context(|| format!("Failed to run the 'each' command '{}'", &each_cmd_str))?
                    .await?;
            }
        }
    }

    // Run the main command we've been given:
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

async fn process_commands(mut secret: Vec<u8>, commands: &[String]) -> Result<String> {
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
            stdin.write_all(&secret)
                .await
                .with_context(|| format!("Failed to write to stdin for the command '{}'", command))?;
        }

        let output = child.wait_with_output()
            .await
            .with_context(|| format!("Failed to read stdout for the command '{}'", command))?;
        secret = output.stdout;

        if secret.ends_with(b"\n") {
            secret.pop();
            if secret.ends_with(b"\r") {
                secret.pop();
            }
        }

        if secret.is_empty() {
            let error_output = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("The command '{}' failed:\n\n'{}'", command, error_output));
        }
    }
    Ok(String::from_utf8_lossy(&secret).into_owned())
}

