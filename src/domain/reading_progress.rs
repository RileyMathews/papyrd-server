use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ReadingProgress {
    pub user_id: Uuid,
    pub document: String,
    pub progress: String,
    pub percentage: f64,
    pub device: String,
    pub device_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}
