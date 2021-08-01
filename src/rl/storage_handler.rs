//! Used to handle the underlying storage for the database of past client requests
//! During dev, this was a simple hashmap<String, String>, but later will use redis

/// For String key: <domain>:<client_id>
/// For String value: <date_when_request_was_made>
/// For Vec we have the list of previous requests (for sliding window alg)
use redis::Commands;
use std::collections::HashMap;
use std::env;

pub struct StorageHandler {
    con: redis::Connection,
}

impl StorageHandler {
    pub fn new() -> Self {
        // setup redis
        let redis_url = env::var("REDIS_URL").expect(
            "Missing REDIS_URL env var.\
            Check it is included in .env and it was loaded in .env file",
        );
        println!("redis url: {}", redis_url);
        let client = redis::Client::open(redis_url).expect("Redis open command failed");
        let con = client.get_connection().expect("Redis connection failed");
        StorageHandler { con }
    }

    pub fn get(&mut self, key: &str) -> Vec<String> {
        let vals: Vec<String> = redis::cmd("lrange")
            .arg(key)
            .arg("0")
            .arg("-1")
            .query(&mut self.con)
            .expect("lrange failed");

        vals
    }

    pub fn append(&mut self, key: &str, val: &str) {
        let _: () = redis::cmd("rpush")
            .arg(key)
            .arg(val)
            .query(&mut self.con)
            .expect("rpush command failed");
    }

    pub fn remove_users_past_requests(&mut self, key: &str) {
        let _: () = redis::cmd("del")
            .arg(key)
            .query(&mut self.con)
            .expect("DEL failed");
    }

    pub fn pop_oldest_request(&mut self, key: &str) {
        let _: () = redis::cmd("lpop")
            .arg(key)
            .query(&mut self.con)
            .expect("LPOP failed");
    }
}
