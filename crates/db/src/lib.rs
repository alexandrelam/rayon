use rayon_types::InstalledApp;
use sonic_channel::{
    Dest, FlushRequest, IngestChannel, PushRequest, QueryRequest, SearchChannel, SonicChannel,
};
use std::env;
use std::fmt;

const DEFAULT_SONIC_HOST: &str = "127.0.0.1";
const DEFAULT_SONIC_PORT: u16 = 1491;
const DEFAULT_SONIC_COLLECTION: &str = "apps";
const DEFAULT_SONIC_BUCKET: &str = "macos";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SonicConfig {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub collection: String,
    pub bucket: String,
}

impl SonicConfig {
    pub fn from_env() -> Result<Option<Self>, SonicConfigError> {
        let password = match env::var("RAYON_SONIC_PASSWORD") {
            Ok(value) if !value.trim().is_empty() => value,
            Ok(_) => return Ok(None),
            Err(env::VarError::NotPresent) => return Ok(None),
            Err(error) => return Err(SonicConfigError::InvalidValue(error.to_string())),
        };

        let host = env::var("RAYON_SONIC_HOST").ok();
        let port = match env::var("RAYON_SONIC_PORT") {
            Ok(value) => Some(value),
            Err(env::VarError::NotPresent) => None,
            Err(error) => return Err(SonicConfigError::InvalidValue(error.to_string())),
        };
        let collection = env::var("RAYON_SONIC_COLLECTION").ok();
        let bucket = env::var("RAYON_SONIC_BUCKET").ok();

        Self::from_values(password, host, port, collection, bucket).map(Some)
    }

    fn from_values(
        password: String,
        host: Option<String>,
        port: Option<String>,
        collection: Option<String>,
        bucket: Option<String>,
    ) -> Result<Self, SonicConfigError> {
        let port = match port {
            Some(value) => value
                .parse::<u16>()
                .map_err(|_| SonicConfigError::InvalidPort(value))?,
            None => DEFAULT_SONIC_PORT,
        };

        Ok(Self {
            host: host.unwrap_or_else(|| DEFAULT_SONIC_HOST.into()),
            port,
            password,
            collection: collection.unwrap_or_else(|| DEFAULT_SONIC_COLLECTION.into()),
            bucket: bucket.unwrap_or_else(|| DEFAULT_SONIC_BUCKET.into()),
        })
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SonicConfigError {
    InvalidPort(String),
    InvalidValue(String),
}

impl fmt::Display for SonicConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPort(value) => write!(f, "invalid RAYON_SONIC_PORT value: {value}"),
            Self::InvalidValue(value) => write!(f, "invalid Sonic environment value: {value}"),
        }
    }
}

impl std::error::Error for SonicConfigError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIndexStats {
    pub discovered_count: usize,
    pub indexed_count: usize,
    pub skipped_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SonicIndexError {
    NotConfigured,
    Config(SonicConfigError),
    Transport(String),
}

impl fmt::Display for SonicIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConfigured => write!(
                f,
                "Sonic is not configured. Set RAYON_SONIC_PASSWORD to enable app search and indexing."
            ),
            Self::Config(error) => write!(f, "{error}"),
            Self::Transport(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for SonicIndexError {}

pub struct SonicAppIndex {
    config: Option<SonicConfig>,
}

impl SonicAppIndex {
    pub fn from_env() -> Result<Self, SonicIndexError> {
        let config = SonicConfig::from_env().map_err(SonicIndexError::Config)?;
        Ok(Self { config })
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    pub fn search_app_ids(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<String>, SonicIndexError> {
        let query = query.trim();
        if query.is_empty() || !self.is_configured() {
            return Ok(Vec::new());
        }

        let config = self.config.as_ref().ok_or(SonicIndexError::NotConfigured)?;
        let channel = SearchChannel::start(config.address(), &config.password)
            .map_err(|error| SonicIndexError::Transport(error.to_string()))?;
        let dest = Dest::col_buc(&config.collection, &config.bucket);
        let request = QueryRequest::new(dest, query).limit(limit);
        channel
            .query(request)
            .map_err(|error| SonicIndexError::Transport(error.to_string()))
    }

    pub fn reindex_apps(&self, apps: &[InstalledApp]) -> Result<AppIndexStats, SonicIndexError> {
        let config = self.config.as_ref().ok_or(SonicIndexError::NotConfigured)?;
        let channel = IngestChannel::start(config.address(), &config.password)
            .map_err(|error| SonicIndexError::Transport(error.to_string()))?;

        channel
            .flush(FlushRequest::bucket(&config.collection, &config.bucket))
            .map_err(|error| SonicIndexError::Transport(error.to_string()))?;

        let dest = Dest::col_buc(&config.collection, &config.bucket);
        let mut indexed_count = 0;
        let mut skipped_count = 0;

        for app in apps {
            let text = app.search_text();
            if text.is_empty() {
                skipped_count += 1;
                continue;
            }

            let request = PushRequest::new(dest.clone().obj(app.id.as_str()), text);
            channel
                .push(request)
                .map_err(|error| SonicIndexError::Transport(error.to_string()))?;
            indexed_count += 1;
        }

        Ok(AppIndexStats {
            discovered_count: apps.len(),
            indexed_count,
            skipped_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayon_types::CommandId;

    #[test]
    fn from_values_applies_defaults() {
        let config = SonicConfig::from_values("secret".into(), None, None, None, None).unwrap();

        assert_eq!(config.host, DEFAULT_SONIC_HOST);
        assert_eq!(config.port, DEFAULT_SONIC_PORT);
        assert_eq!(config.collection, DEFAULT_SONIC_COLLECTION);
        assert_eq!(config.bucket, DEFAULT_SONIC_BUCKET);
    }

    #[test]
    fn from_values_respects_overrides() {
        let config = SonicConfig::from_values(
            "secret".into(),
            Some("sonic.local".into()),
            Some("1499".into()),
            Some("custom-apps".into()),
            Some("desktop".into()),
        )
        .unwrap();

        assert_eq!(config.host, "sonic.local");
        assert_eq!(config.port, 1499);
        assert_eq!(config.collection, "custom-apps");
        assert_eq!(config.bucket, "desktop");
    }

    #[test]
    fn rejects_invalid_port() {
        let error =
            SonicConfig::from_values("secret".into(), None, Some("invalid".into()), None, None)
                .unwrap_err();

        assert_eq!(error, SonicConfigError::InvalidPort("invalid".into()));
    }

    #[test]
    fn search_is_empty_when_unconfigured() {
        let index = SonicAppIndex { config: None };

        let results = index.search_app_ids("arc", 10).unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn app_index_stats_reflect_search_text_presence() {
        let app = InstalledApp {
            id: CommandId::from("app:macos:com.example.arc"),
            title: "Arc".into(),
            bundle_identifier: Some("com.example.arc".into()),
            path: "/Applications/Arc.app".into(),
        };

        assert_eq!(app.search_text(), "Arc com.example.arc Arc");
    }
}
