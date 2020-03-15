use directories::BaseDirs;
use std::path::{ Path, PathBuf };
use anyhow::{ anyhow, Result, Context };
use serde::{ Deserialize, Serialize };
use tokio::fs;

#[derive(Debug)]
pub struct Cache {
    dir: PathBuf,
    data: CacheData
}

#[derive(Debug,Serialize,Deserialize)]
struct CacheData {
    last_token: Option<CachedToken>
}

#[derive(Debug,Serialize,Deserialize)]
struct CachedToken {
    token: String
}

static FILENAME: &str = "cache";

impl Cache {

    /// Load the user specific cache from file system,
    /// returning defaults if no such cache exists or an
    /// error if we don't know where to look.
    pub async fn load() -> Result<Cache> {

        let base_dirs = BaseDirs::new().ok_or_else(||
            anyhow!("Could not resolve a path to the cache"))?;

        let mut cache_dir = base_dirs.cache_dir().to_owned();
        cache_dir.push("vault_inject");

        let cache_data = load_data(cache_dir.clone(), FILENAME).await;

        Ok(Cache {
            dir: cache_dir,
            data: cache_data
         })
    }

    /// Write the cache data back to disk.
    pub async fn save(&self) -> Result<()> {
        save_data(self.dir.clone(), FILENAME, &self.data).await
    }

    /// Store a token against some auth details, so it will be reused if
    /// the auth details are reused.
    pub fn set_token(&mut self, token: String) {
        self.data.last_token = Some(CachedToken {
            token: token
        })
    }

    /// Get a token back given some auth details if one is cached.
    pub fn get_token(&self) -> Option<String> {
        if let Some(cached) = &self.data.last_token {
            Some(cached.token.to_owned())
        } else {
            None
        }
    }

}

async fn load_data(mut path: PathBuf, filename: &str) -> CacheData {
    path.push(filename);

    async fn try_load_from_file(path: &Path) -> Result<CacheData> {
        use tokio::io::AsyncReadExt;
        let mut file = fs::File::open(path).await?;
        let mut contents = vec![];
        file.read_to_end(&mut contents).await?;
        serde_json::from_slice(&contents).map_err(|_| anyhow!("Cannot deserialize"))
    }

    // Try to load the cache from disk:
    if let Ok(cache_data) = try_load_from_file(&path).await {
        return cache_data;
    }

    // Failing that, return a default empty cache:
    CacheData {
        last_token: None
    }
}

async fn save_data(mut path: PathBuf, filename: &str, data: &CacheData) -> Result<()> {
    fs::create_dir_all(&path).await?;
    path.push(filename);

    let mut file = fs::File::create(path)
        .await
        .with_context(|| format!("Failed to update cached data"))?;

    let data = serde_json::to_vec(data)
        .with_context(|| format!("Failed to serialize cache data for writing"))?;

    use tokio::io::AsyncWriteExt;
    file.write_all(&data)
        .await
        .with_context(|| format!("Failed to write cache data"))?;
    file.sync_data()
        .await
        .with_context(|| format!("Failed to sync cache data to disk"))?;

    Ok(())
}