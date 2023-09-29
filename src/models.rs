use std::hash::{Hash, Hasher};
use chrono::{DateTime, Utc};
use redis::{FromRedisValue, Value, RedisResult};
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct ContentMessage {
    pub machine_name: String,
    pub process_name: String,
    pub window_title: String,
    pub start_time: DateTime<Utc>,
    pub process_time: DateTime<Utc>,
    pub is_active: bool
}

impl Hash for ContentMessage {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.machine_name.hash(state);
        self.process_name.hash(state);
        self.window_title.hash(state);
        self.start_time.hash(state);
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BeforeMessageTime {
    pub hash: u64,
    pub uuid: String,
    pub time: DateTime<Utc>
}

impl FromRedisValue for BeforeMessageTime {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        let value: String = FromRedisValue::from_redis_value(v)?;
        let json: Result<BeforeMessageTime, serde_json::Error> = serde_json::from_str(&value);
        match json {
            Ok(message) => {
                return Ok(message);
            },
            Err(_) => {
                return Err((redis::ErrorKind::TypeError, "bad first token").into())
            }   
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Activity {
    pub uuid: String,
    pub machine_name: String,
    pub process_name: String,
    pub window_title: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

pub struct NewActivity {
    pub hash: u64,
    pub start_time: DateTime<Utc>,
    pub message: ContentMessage
}

#[derive(Debug)]
pub enum MyError {
    Redis(redis::RedisError),
    RabbitMQ(amiquip::Error),
    Postgres(postgres::Error),
    Json(serde_json::Error),
    Exception(String)
}
