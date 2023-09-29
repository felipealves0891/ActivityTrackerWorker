
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::{Activity, ContentMessage, BeforeMessageTime, NewActivity};

pub fn create_queue_declare_options() -> amiquip::QueueDeclareOptions {
    let mut options = amiquip::QueueDeclareOptions::default();
    options.durable = true;
    options.auto_delete = false;
    options.exclusive = false;
    return options;
}

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

pub fn create_activity(message: &ContentMessage, start_time: DateTime<Utc>) -> Activity {
    Activity { 
        uuid: Uuid::new_v4().to_string(), 
        machine_name: message.machine_name.to_string(), 
        process_name: message.process_name.to_string(), 
        window_title: message.window_title.to_string(), 
        start: start_time, 
        end: message.process_time 
    }
}

pub fn create_before_message(hash: u64, time: DateTime<Utc>, uuid: String) -> BeforeMessageTime {
    BeforeMessageTime { 
        hash: hash, 
        time: time,
        uuid: uuid
    }
}

pub fn create_new_activity(hash: u64, message: &ContentMessage) -> NewActivity {
    NewActivity { 
        hash: hash, 
        start_time: message.process_time, 
        message: message.clone()
    }
}