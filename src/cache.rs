use directories::BaseDirs;
use std::path::{ Path, PathBuf };
use anyhow::{ anyhow, Result, Context };
use serde::{ Deserialize, Serialize };
use tokio::fs;
use crate::auth::AuthDetails;

pub struct Cache {
    path: PathBuf,
    data: CacheData
}

#[derive(Serialize,Deserialize)]
struct CacheData {
    last_token: CachedToken
}

#[derive(Serialize,Deserialize)]
struct CachedToken {
    auth_details: AuthDetails
}

impl Cache {

    /// Load the user specific cache from file system,
    /// returning defaults if no such cache exists or an
    /// error if we don't know where to look.
    pub async fn load() -> Result<Cache> {

        let base_dirs = BaseDirs::new().ok_or_else(||
            anyhow!("Could not resolve a path to the cached files"))?;

        let mut cache_dir = base_dirs.cache_dir().to_owned();
        cache_dir.push("vault-inject/cache");

        let cache_data = load_data(&cache_dir).await;

        Ok(Cache {
            path: cache_dir,
            data: cache_data
         })
    }

    /// Write the cache data back to disk.
    pub async fn save(&self) -> Result<()> {
        save_data(&self.path, &self.data).await
    }

    pub fn get(&self) -> &CacheData {
        &self.data
    }

    pub fn get_mut(&mut self) -> &mut CacheData {
        &mut self.data
    }

}

async fn load_data(path: &Path) -> CacheData {

    async fn try_load_from_file(path: &Path) -> Result<CacheData> {
        use tokio::io::AsyncReadExt;
        let mut file = fs::File::open(path).await?;
        let mut contents = vec![];
        file.read_to_end(&mut contents).await?;
        serde_json::from_slice(&contents).map_err(|_| anyhow!("Cannot deserialize"))
    }

    // Try to load the cache from disk:
    if let Ok(cache_data) = try_load_from_file(path).await {
        return cache_data;
    }

    // Failing that, return a default empty cache:
    CacheData {}
}

async fn save_data(path: &Path, data: &CacheData) -> Result<()> {
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