use redis::{AsyncCommands, Client};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::types::{AuditReport, AuditStatus};

/// Redis key prefix for audit jobs
const AUDIT_KEY_PREFIX: &str = "watchy:audit:";
/// TTL for audit jobs (7 days)
const AUDIT_TTL_SECONDS: u64 = 7 * 24 * 60 * 60;

/// Represents an audit job
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditJob {
    pub id: String,
    pub agent_id: u64,
    pub chain_id: u64,
    pub status: AuditStatus,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    pub result: Option<AuditReport>,
    pub error: Option<String>,
}

/// Audit store with Redis backend and in-memory fallback
pub struct AuditStore {
    redis: Option<RwLock<redis::aio::ConnectionManager>>,
    /// Fallback in-memory store when Redis is unavailable
    fallback: RwLock<std::collections::HashMap<String, AuditJob>>,
}

impl AuditStore {
    /// Create a new store with Redis connection
    pub async fn new(redis_url: Option<&str>) -> Self {
        let redis = if let Some(url) = redis_url {
            match Client::open(url) {
                Ok(client) => match client.get_connection_manager().await {
                    Ok(conn) => {
                        info!("Connected to Redis at {}", url);
                        Some(RwLock::new(conn))
                    }
                    Err(e) => {
                        warn!("Failed to connect to Redis: {}. Using in-memory fallback.", e);
                        None
                    }
                },
                Err(e) => {
                    warn!("Invalid Redis URL: {}. Using in-memory fallback.", e);
                    None
                }
            }
        } else {
            info!("No Redis URL configured. Using in-memory store.");
            None
        };

        Self {
            redis,
            fallback: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Create a new in-memory only store (for testing)
    pub fn in_memory() -> Self {
        Self {
            redis: None,
            fallback: RwLock::new(std::collections::HashMap::new()),
        }
    }

    fn make_key(id: &str) -> String {
        format!("{}{}", AUDIT_KEY_PREFIX, id)
    }

    /// Create a new audit job and return its ID
    pub async fn create_job(&self, agent_id: u64, chain_id: u64) -> String {
        let id = format!("aud_{}", uuid::Uuid::new_v4().simple());
        let now = chrono::Utc::now().timestamp() as u64;

        let job = AuditJob {
            id: id.clone(),
            agent_id,
            chain_id,
            status: AuditStatus::Pending,
            created_at: now,
            completed_at: None,
            result: None,
            error: None,
        };

        if let Some(redis) = &self.redis {
            let key = Self::make_key(&id);
            match serde_json::to_string(&job) {
                Ok(json) => {
                    let mut conn = redis.write().await;
                    let result: Result<(), redis::RedisError> = conn
                        .set_ex(&key, &json, AUDIT_TTL_SECONDS)
                        .await;
                    if let Err(e) = result {
                        error!("Redis SET failed: {}. Storing in memory.", e);
                        self.fallback.write().await.insert(id.clone(), job);
                    } else {
                        debug!("Stored job {} in Redis", id);
                    }
                }
                Err(e) => {
                    error!("Failed to serialize job: {}", e);
                    self.fallback.write().await.insert(id.clone(), job);
                }
            }
        } else {
            self.fallback.write().await.insert(id.clone(), job);
        }

        id
    }

    /// Get a job by ID
    pub async fn get_job(&self, id: &str) -> Option<AuditJob> {
        if let Some(redis) = &self.redis {
            let key = Self::make_key(id);
            let mut conn = redis.write().await;
            let result: Result<Option<String>, redis::RedisError> = conn.get(&key).await;
            match result {
                Ok(Some(json)) => match serde_json::from_str(&json) {
                    Ok(job) => return Some(job),
                    Err(e) => {
                        error!("Failed to deserialize job {}: {}", id, e);
                    }
                },
                Ok(None) => {
                    // Check fallback
                    return self.fallback.read().await.get(id).cloned();
                }
                Err(e) => {
                    error!("Redis GET failed: {}. Checking fallback.", e);
                }
            }
        }

        self.fallback.read().await.get(id).cloned()
    }

    /// Update a job in the store
    async fn update_job(&self, job: &AuditJob) {
        if let Some(redis) = &self.redis {
            let key = Self::make_key(&job.id);
            match serde_json::to_string(job) {
                Ok(json) => {
                    let mut conn = redis.write().await;
                    let result: Result<(), redis::RedisError> = conn
                        .set_ex(&key, &json, AUDIT_TTL_SECONDS)
                        .await;
                    if let Err(e) = result {
                        error!("Redis SET failed: {}. Updating fallback.", e);
                        self.fallback.write().await.insert(job.id.clone(), job.clone());
                    }
                }
                Err(e) => {
                    error!("Failed to serialize job: {}", e);
                }
            }
        } else {
            self.fallback.write().await.insert(job.id.clone(), job.clone());
        }
    }

    /// Update job status
    pub async fn update_status(&self, id: &str, status: AuditStatus) {
        if let Some(mut job) = self.get_job(id).await {
            job.status = status;
            self.update_job(&job).await;
        }
    }

    /// Set job result (marks as completed)
    pub async fn set_result(&self, id: &str, result: AuditReport) {
        if let Some(mut job) = self.get_job(id).await {
            job.status = AuditStatus::Completed;
            job.completed_at = Some(chrono::Utc::now().timestamp() as u64);
            job.result = Some(result);
            self.update_job(&job).await;
        }
    }

    /// Set job error (marks as failed)
    pub async fn set_error(&self, id: &str, error: String) {
        if let Some(mut job) = self.get_job(id).await {
            job.status = AuditStatus::Failed;
            job.completed_at = Some(chrono::Utc::now().timestamp() as u64);
            job.error = Some(error);
            self.update_job(&job).await;
        }
    }

    /// Check if Redis is connected
    pub fn has_redis(&self) -> bool {
        self.redis.is_some()
    }
}
