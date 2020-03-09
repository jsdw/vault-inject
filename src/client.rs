use reqwest::{ Method };
use serde::{ Serialize, de::DeserializeOwned };
use url::Url;
use anyhow::Result;

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

    pub fn set_token(&mut self, tok: String) {
        self.token = Some(tok);
    }

    async fn request<D: DeserializeOwned, P: AsRef<str>, B: Serialize>(&self, method: Method, path: P, body: Option<B>) -> Result<D> {
        let url = make_api_path(self.vault_url.clone(), path.as_ref());
        let mut builder = self.client.request(method, url);
        if let Some(tok) = &self.token {
            builder = builder.header("Authorization", tok);
        }
        if let Some(body) = &body {
            builder = builder.json(body);
        }
        let res: D = builder.send().await?.json().await?;
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