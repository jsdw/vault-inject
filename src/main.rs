mod auth;
mod secret;
mod utils;

use crate::auth::{ AuthDetails, AuthType, get_auth_token };
use crate::secret::{ SecretMapping, fetch_secret };
use anyhow::{ anyhow, Result, Context };
use structopt::StructOpt;
use std::process::Command;

#[derive(Debug,Clone,StructOpt)]
#[structopt(name="vault-inject", about = "Inject vault secrets into commands")]
struct Opts {
    /// The command you'd like to run
    command: String,

    /// Username to login with (you'll be prompted if this isn't provided)
    #[structopt(long="username", env="VAULT_INJECT_USERNAME")]
    username: Option<String>,

    /// Password to login with (you'll be prompted if this isn't provided)
    #[structopt(long="password", env="VAULT_INJECT_PASSWORD")]
    password: Option<String>,

    /// Token to use to login
    #[structopt(long="token", env="VAULT_INJECT_TOKEN")]
    token: Option<String>,

    /// URL of your vault instance (eg https://vault.yourdomain)
    #[structopt(long="vault-url", env="VAULT_INJECT_URL")]
    vault_url: url::Url,

    /// Which type of authentication would you like to use with vault?
    #[structopt(long="auth-type", default_value="ldap", env="VAULT_INJECT_AUTH_TYPE")]
    auth_type: AuthType,

    /// Where is the chosen authentication type mounted?
    #[structopt(long="auth-path", default_value="", env="VAULT_INJECT_AUTH_PATH")]
    auth_path: String,

    /// Map secrets to environment variables
    #[structopt(short="s", long="secret")]
    secrets: Vec<SecretMapping>
}

fn main() {
    let opts = Opts::from_args();
    if let Err(e) = run(opts) {
        eprintln!("{}", e);
    }
}

fn run(opts: Opts) -> Result<()> {

    if opts.secrets.is_empty() {
        return Err(anyhow!("One or more secret mappings should be provided using '--secret'"));
    }

    let auth_details = to_auth_details(&opts);
    let auth_token = get_auth_token(auth_details)?;

    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(&opts.command);

    for secret_mapping in &opts.secrets {
        let secret_value = fetch_secret(opts.vault_url.clone(), &auth_token, &secret_mapping.secret)?;
        cmd.env(&secret_mapping.env_var, secret_value);
    }

    cmd.spawn().with_context(|| format!("Failed to run the command '{}'", &opts.command))?;

    Ok(())
}

fn to_auth_details(opts: &Opts) -> AuthDetails {
    match  opts.auth_type {
        AuthType::Ldap => AuthDetails::Ldap {
            vault_url: opts.vault_url.clone(),
            path:      opts.auth_path.clone(),
            username:  opts.username.clone().unwrap_or(String::new()),
            password:  opts.password.clone().unwrap_or(String::new())
        },
        AuthType::UserPass => AuthDetails::UserPass {
            vault_url: opts.vault_url.clone(),
            path:      opts.auth_path.clone(),
            username:  opts.username.clone().unwrap_or(String::new()),
            password:  opts.password.clone().unwrap_or(String::new())
        },
        AuthType::Token => AuthDetails::Token {
            token: opts.token.clone().unwrap_or(String::new())
        },
    }
}