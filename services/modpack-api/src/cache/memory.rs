use crate::observability;
use moka::sync::Cache;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct ResponseCaches {
    search: Cache<String, Arc<Value>>,
    project: Cache<String, Arc<Value>>,
    versions: Cache<String, Arc<Value>>,
    version: Cache<String, Arc<Value>>,
    categories: Cache<String, Arc<Value>>,
    health: Cache<String, bool>,
}

impl Default for ResponseCaches {
    fn default() -> Self {
        Self {
            search: json_cache(2 * 60, 500),
            project: json_cache(10 * 60, 1_000),
            versions: json_cache(5 * 60, 1_000),
            version: json_cache(30 * 60, 2_000),
            categories: json_cache(6 * 60 * 60, 32),
            health: Cache::builder()
                .time_to_live(Duration::from_secs(30))
                .max_capacity(8)
                .build(),
        }
    }
}

impl ResponseCaches {
    #[must_use]
    pub fn get(&self, policy: CachePolicy, key: &str) -> Option<Arc<Value>> {
        let value = self.cache(policy).get(key);
        observability::record_cache_lookup(policy.metric_label(), value.is_some());
        value
    }

    pub fn insert(&self, policy: CachePolicy, key: String, value: Arc<Value>) {
        self.cache(policy).insert(key, value);
    }

    #[must_use]
    pub fn health(&self, key: &str) -> Option<bool> {
        self.health.get(key)
    }

    pub fn insert_health(&self, key: String, value: bool) {
        self.health.insert(key, value);
    }

    const fn cache(&self, policy: CachePolicy) -> &Cache<String, Arc<Value>> {
        match policy {
            CachePolicy::Search => &self.search,
            CachePolicy::Project => &self.project,
            CachePolicy::Versions => &self.versions,
            CachePolicy::Version => &self.version,
            CachePolicy::Categories => &self.categories,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CachePolicy {
    Search,
    Project,
    Versions,
    Version,
    Categories,
}

impl CachePolicy {
    const fn metric_label(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Project => "project",
            Self::Versions => "versions",
            Self::Version => "version",
            Self::Categories => "categories",
        }
    }
}

fn json_cache(seconds: u64, capacity: u64) -> Cache<String, Arc<Value>> {
    Cache::builder()
        .time_to_live(Duration::from_secs(seconds))
        .max_capacity(capacity)
        .build()
}
