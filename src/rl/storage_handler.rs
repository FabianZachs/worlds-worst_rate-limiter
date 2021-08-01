//! Used to handle the underlying storage for the database of past client requests
//! During dev, this was a simple hashmap<String, String>, but later will use redis

/// For String key: <domain>:<client_id>
/// For String value: <date_when_request_was_made>
/// For Vec we have the list of previous requests (for sliding window alg)
use redis::Commands;
use std::collections::HashMap;
use std::env;

pub struct StorageHandler {
    hm: HashMap<String, Vec<String>>,
    // redis_con: redis::Connection,
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
        StorageHandler { hm: HashMap::new() }
    }

    pub fn get(&self, key: &str) -> Vec<String> {
        let x = vec![String::from("1"), String::from("2")];

        x
    }
}
