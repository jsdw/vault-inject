use reqwest::{ Method, RequestBuilder };
use url::Url;

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

    pub fn request<P: AsRef<str>>(&self, method: Method, path: P) -> RequestBuilder {
        let url = make_api_path(self.vault_url.clone(), path.as_ref());
        let mut builder = self.client.request(method, url);
        if let Some(tok) = &self.token {
            builder = builder.header("Authorization", tok);
        }
        builder
    }

    pub fn get<P: AsRef<str>>(&self, path: P) -> RequestBuilder {
        self.request(Method::GET, path)
    }

    pub fn post<P: AsRef<str>>(&self, path: P) -> RequestBuilder {
        self.request(Method::POST, path)
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