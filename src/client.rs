use reqwest::{ Method };
use serde::{ Deserialize, Serialize, de::DeserializeOwned };
use url::Url;
use anyhow::{ anyhow, Result, Context };
use std::fmt;

#[derive(Clone)]
pub struct Client {
    vault_url: Url,
    client: reqwest::Client,
    token: Option<String>
}

impl Client {

    pub fn new(vault_url: Url) -> Client {
        Client {
            vault_url,
            client: reqwest::Client::new(),
            token: None
        }
    }

    pub fn with_token(&self, tok: String) -> Client {
        Client {
            vault_url: self.vault_url.clone(),
            client: self.client.clone(),
            token: Some(tok)
        }
    }

    async fn request<D: DeserializeOwned, P: AsRef<str>, B: Serialize>(&self, method: Method, path: P, body: Option<B>) -> Result<D> {
        let path_str = path.as_ref();
        let url = make_api_path(self.vault_url.clone(), path_str);
        let mut builder = self.client.request(method, url);
        if let Some(tok) = &self.token {
            builder = builder.header("Authorization", format!("Bearer {}", tok));
        }
        if let Some(body) = &body {
            builder = builder.json(body);
        }
        let res = builder.send()
            .await
            .with_context(|| anyhow!("Failed to make request to '{}'", path_str))?;

        if !res.status().is_success() {
            let reason = res.status().canonical_reason();
            let status_str = res.status().as_str().to_owned();
            let errors = res.json().await.unwrap_or(Errors::none());
            if errors.errors.is_empty() {
                return Err(match reason {
                    Some(reason) => anyhow!("{} {} response from Vault", status_str, reason),
                    None => anyhow!("{} response from Vault", status_str)
                });
            } else {
                return Err(errors.into());
            }
        }

        let res: D = res.json()
            .await
            .with_context(|| anyhow!("Failed to handle API response from request to '{}'", path_str))?;

        Ok(res)
    }

    pub async fn get<D: DeserializeOwned, P: AsRef<str>>(&self, path: P) -> Result<D> {
        self.request(Method::GET, path, None as Option<()>).await
    }

    pub async fn post<D: DeserializeOwned, P: AsRef<str>, B: Serialize>(&self, path: P, body: B) -> Result<D> {
        self.request(Method::POST, path, Some(body)).await
    }

}

fn make_api_path(mut url: url::Url, path: &str) -> url::Url {
    let path = format!(
        "{prefix}/v1/{path}",
        prefix = url.path().trim_matches('/'),
        path = path.trim_matches('/')
    );
    url.set_path(&path);
    url
}

/// Vault API errors come back in this format:
#[derive(Debug,Deserialize)]
struct Errors {
    errors: Vec<String>
}

impl Errors {
    fn none() -> Errors {
        Errors { errors: Vec::new() }
    }
}

impl std::error::Error for Errors {}

impl fmt::Display for Errors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for err in &self.errors {
            write!(f, "{}\n", err)?;
        }
        Ok(())
    }
}