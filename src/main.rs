extern crate redis;
extern crate chrono;

mod factories;
mod models;

use chrono::{DateTime, Utc};
use models::{ContentMessage, BeforeMessageTime, MyError, NewActivity};
use amiquip::{Connection, ConsumerMessage, ConsumerOptions};
use redis::{Commands, RedisError};

const AMQP_CONNECTION_STRING: &str = "amqp://myQueue:myQueue!123456@localhost:12001/";
const AMQP_QUEUE_NAME: &str = "q_process";

const RADIS_CONNECTION_STRING: &str = "redis://:myCache!123456@localhost:12004";
const RADIS_KEY_BEFORE_MESSAGE: &str = "before_message";

const SQL_CONNECTION_STRING: &str = "postgresql://myDb:myDb!123456@localhost:12003/DB_ACTIVITY_TRACKER";
const SQL_INSERT_ACTIVITY: &str = "INSERT INTO activities.activity(uuid, machine_name, process_name, window_title, start_time, end_time) VALUES ($1, $2, $3, $4, $5, $6);";
const SQL_UPDATE_ACTIVITY: &str = "UPDATE activities.activity SET end_time=$2 WHERE uuid=$1;";

fn main() -> Result<(), MyError>  {

    let mut connection = Connection::insecure_open(&AMQP_CONNECTION_STRING).map_err(|e| MyError::RabbitMQ(e))?;
    let channel = connection.open_channel(None).map_err(|e| MyError::RabbitMQ(e))?;
    channel.qos(0, 1, true).map_err(|e| MyError::RabbitMQ(e))?;
    let options = factories::create_queue_declare_options();
    let queue = channel.queue_declare(AMQP_QUEUE_NAME, options).map_err(|e| MyError::RabbitMQ(e))?;
    let consumer = queue.consume(ConsumerOptions::default()).map_err(|e| MyError::RabbitMQ(e))?;
    
    let client = redis::Client::open(RADIS_CONNECTION_STRING).map_err(|e| MyError::Redis(e))?;
    let mut redis = client.get_connection().map_err(|e| MyError::Redis(e))?;

    let mut pg = postgres::Client::connect(SQL_CONNECTION_STRING, postgres::NoTls).map_err(|e| MyError::Postgres(e))?;

    for (i, message) in consumer.receiver().iter().enumerate() {
        match message {
            ConsumerMessage::Delivery(delivery) => {
                println!("Consumer '{}'.", i);
                let body = String::from_utf8_lossy(&delivery.body);
                let content: Vec<ContentMessage> = serde_json::from_str(&body).expect("Not parsed message!");
                let is_processed =  process(content, &mut redis, &mut pg);
                match is_processed {
                    Ok(_) => {
                        consumer.ack(delivery).map_err(|e| MyError::RabbitMQ(e))?;
                    },
                    Err(error) => {
                        if error_handler(error) {
                            consumer.ack(delivery).map_err(|e| MyError::RabbitMQ(e))?;
                        } else {
                            consumer.nack(delivery, true).map_err(|e| MyError::RabbitMQ(e))?;
                        }
                    }
                }
            }
            other => {
                println!("Consumer ended: {:?}", other);
                break;
            }
        }
    }

    let _ = connection.close().map_err(|e| MyError::RabbitMQ(e));
    return Ok(());

}

fn process(messages: Vec<ContentMessage>, cache: &mut redis::Connection, pg: &mut postgres::Client) -> Result<(), MyError> {
    let message 
        = messages.iter()
                  .find(|&x| x.is_active == true)
                  .ok_or(MyError::Exception(String::from("Message has not active activity!")))?;
    
    let default_before_message = BeforeMessageTime { hash: 0, time: Utc::now(), uuid: String::from("")};
    let before_message = get_before_message(cache).map_or(default_before_message, |f| f);

    let hash = factories::calculate_hash(&message);
    if hash == before_message.hash {
        process_activity(before_message.uuid, message.process_time, pg)?;
    } else {
        let new_activity = factories::create_new_activity(hash, &message);
        let _ = process_new_activity(new_activity, cache, pg)?;
    }
    
    return Ok(());
}

fn process_new_activity(data: NewActivity, cache: &mut redis::Connection, pg: &mut postgres::Client) -> Result<(), MyError> {
    println!("new activity");
    let activity = factories::create_activity(&data.message, data.start_time);
    pg.execute(
        SQL_INSERT_ACTIVITY, 
        &[
            &activity.uuid, 
            &activity.machine_name, 
            &activity.process_name,
            &activity.window_title,
            &activity.start.naive_local(),
            &activity.end.naive_local()])
      .map_err(|e| MyError::Postgres(e))?;
    
    let before_message = factories::create_before_message(data.hash, data.start_time, activity.uuid);
    set_before_message(before_message, cache)?;
    Ok(())
}

fn process_activity(uuid: String, time: DateTime<Utc>, pg: &mut postgres::Client) -> Result<(), MyError> {
    println!("activity");
    let affected_rows = pg.execute(SQL_UPDATE_ACTIVITY, &[&uuid, &time.naive_local()])
                               .map_err(|e| MyError::Postgres(e))?;
    
    if affected_rows > 0 {
        Ok(())
    } else {
        let error_message = format!("Not found activity with uuid={}", uuid); 
        Err(MyError::Exception(error_message))
    }
}

fn get_before_message(cache: &mut redis::Connection) -> Option<BeforeMessageTime> {
    let redis_value: Result<BeforeMessageTime, RedisError> 
        = cache.get(RADIS_KEY_BEFORE_MESSAGE);

    match redis_value {
        Ok(value) => {
            return Option::Some(value);
        },
        Err(_) => {
            return Option::None;
        }
    }
}

fn set_before_message(before_message: BeforeMessageTime, cache: &mut redis::Connection) -> Result<(), MyError> {
    let value = 
        serde_json::to_string(&before_message)
                  .map_err(|e| MyError::Json(e))?;
    
    let _ : () = redis::cmd("SET")
                    .arg(RADIS_KEY_BEFORE_MESSAGE)
                    .arg(value)
                    .query(cache)
                    .map_err(|e| MyError::Redis(e))?;

    return Ok(());
}

fn error_handler(error: MyError) -> bool{
    match error {
        MyError::Exception(error_message) => {
            println!("error: {}", error_message);
            return true;
        }
        MyError::Postgres(error_message) => {
            println!("error: {}", error_message);
            return false;
        },
        MyError::RabbitMQ(error_message) => {
            println!("error: {}", error_message);
            return false;
        },
        MyError::Redis(error_message) => {
            println!("error: {}", error_message);
            return false;
        },
        MyError::Json(error_message) => {
            println!("error: {}", error_message);
            return false;
        },
    }
}