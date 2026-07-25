//! Instance persistence (`%LOCALAPPDATA%/PandaNote/instances.json`).

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use panda_api::ApiClient;
use panda_core::{Instance, normalize_api_url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct InstancesFile {
    active_id: Option<String>,
    instances: Vec<Instance>,
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    path: PathBuf,
    data: InstancesFile,
}

impl SessionStore {
    pub fn empty() -> Self {
        let path = instances_path().unwrap_or_else(|_| {
            std::env::temp_dir()
                .join("PandaNote")
                .join("instances.json")
        });
        Self {
            path,
            data: InstancesFile::default(),
        }
    }

    pub fn load() -> Result<Self> {
        let path = instances_path()?;
        let data = if path.exists() {
            let raw =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            // Tolerate UTF-8 BOM from some editors / PowerShell Set-Content.
            let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
            let mut data: InstancesFile = serde_json::from_str(raw)
                .with_context(|| format!("parse instances.json ({})", path.display()))?;
            for instance in &mut data.instances {
                instance.api_url = normalize_api_url(&instance.api_url);
            }
            data
        } else {
            InstancesFile::default()
        };
        Ok(Self { path, data })
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn instances(&self) -> &[Instance] {
        &self.data.instances
    }

    pub fn active(&self) -> Option<&Instance> {
        let id = self.data.active_id.as_deref()?;
        self.data.instances.iter().find(|i| i.id == id)
    }

    pub fn set_active(&mut self, id: &str) -> Result<()> {
        if !self.data.instances.iter().any(|i| i.id == id) {
            return Err(anyhow!("unknown instance {id}"));
        }
        self.data.active_id = Some(id.to_string());
        self.save()
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(&self.data)?;
        fs::write(&self.path, raw)?;
        Ok(())
    }

    pub fn upsert(&mut self, instance: Instance) -> Result<()> {
        if let Some(existing) = self.data.instances.iter_mut().find(|i| i.id == instance.id) {
            *existing = instance.clone();
        } else {
            self.data.instances.push(instance.clone());
        }
        self.data.active_id = Some(instance.id);
        self.save()
    }

    pub fn remove(&mut self, id: &str) -> Result<()> {
        self.data.instances.retain(|i| i.id != id);
        if self.data.active_id.as_deref() == Some(id) {
            self.data.active_id = self.data.instances.first().map(|i| i.id.clone());
        }
        self.save()
    }

    /// Update display name / API URL without minting a new token.
    pub fn update_details(&mut self, id: &str, name: &str, api_url: &str) -> Result<()> {
        let Some(instance) = self.data.instances.iter_mut().find(|i| i.id == id) else {
            return Err(anyhow!("unknown instance {id}"));
        };
        instance.name = name.to_string();
        instance.api_url = normalize_api_url(api_url);
        self.save()
    }

    /// Replace credentials for an existing instance (keeps the same id).
    pub fn update_via_login(
        &mut self,
        id: &str,
        name: &str,
        api_url: &str,
        username: &str,
        password: &str,
    ) -> Result<Instance> {
        let api_url = normalize_api_url(api_url);
        let device_id = self
            .data
            .instances
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.device_id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let token = ApiClient::bootstrap_token_blocking(
            &api_url,
            username,
            password,
            &device_id,
            &format!("panda-desktop-{name}"),
        )
        .map_err(|e| anyhow!("{e}"))?;
        let instance = Instance {
            id: id.to_string(),
            name: name.to_string(),
            api_url,
            token,
            device_id,
        };
        self.upsert(instance.clone())?;
        Ok(instance)
    }

    pub fn update_via_token(
        &mut self,
        id: &str,
        name: &str,
        api_url: &str,
        token: &str,
    ) -> Result<Instance> {
        let device_id = self
            .data
            .instances
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.device_id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let instance = Instance {
            id: id.to_string(),
            name: name.to_string(),
            api_url: normalize_api_url(api_url),
            token: token.trim().to_string(),
            device_id,
        };
        self.upsert(instance.clone())?;
        Ok(instance)
    }

    /// Add instance via username/password → mint `pnda_` token.
    pub fn add_via_login(
        &mut self,
        name: &str,
        api_url: &str,
        username: &str,
        password: &str,
    ) -> Result<Instance> {
        let api_url = normalize_api_url(api_url);
        let device_id = Uuid::new_v4().to_string();
        let token = ApiClient::bootstrap_token_blocking(
            &api_url,
            username,
            password,
            &device_id,
            &format!("panda-desktop-{name}"),
        )
        .map_err(|e| anyhow!("{e}"))?;
        let instance = Instance {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            api_url,
            token,
            device_id,
        };
        self.upsert(instance.clone())?;
        Ok(instance)
    }

    /// Add instance with an existing API token.
    pub fn add_via_token(&mut self, name: &str, api_url: &str, token: &str) -> Result<Instance> {
        let instance = Instance {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            api_url: normalize_api_url(api_url),
            token: token.trim().to_string(),
            device_id: Uuid::new_v4().to_string(),
        };
        self.upsert(instance.clone())?;
        Ok(instance)
    }
}

pub fn instances_path() -> Result<PathBuf> {
    let base = dirs::data_local_dir().ok_or_else(|| anyhow!("no local data dir"))?;
    Ok(base.join("PandaNote").join("instances.json"))
}

pub fn app_data_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir().ok_or_else(|| anyhow!("no local data dir"))?;
    let dir = base.join("PandaNote");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Placeholder session handle used by older stubs.
#[derive(Debug, Default)]
pub struct Session;
