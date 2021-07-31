use dotenv::dotenv;
use redis::Commands;
use std::{env, io::Read};
use yaml_rust::YamlLoader;

pub struct RateLimiter {
    redis_con: redis::Connection,
    // needs redis connection
}

/// The RateLimiter will receive a Request, query redis to retreiv
/// We map a request to a RequestType so our RateLimiter knows what the config is for that request
/// Example: We may only allow 5 login events per hour, vs 10 message requests every minute
pub enum RequestType {
    Login,
    Message,
}

/// RateLimiter will respond indicating whether or not the request can succeed (request limit
/// hasn't been hit), or whether the request should be dropped (request limit hit), including a
/// message for the client
pub enum RateLimiterResponse {
    Drop(String),
    Success,
}

impl RateLimiter {
    pub fn new(redis_url: String) -> RateLimiter {
        dotenv(); // load .env variables to environment

        // setup redis
        let redis_url = env::var("REDIS_URL").expect("Missing REDIS_URL in .env file");
        let client = redis::Client::open(redis_url).expect("Redis open command failed");
        let con = client.get_connection().expect("Redis connection failed");

        // retrieve rate limiter config
        let yaml_conf_path =
            env::var("YAML_CONF_PATH").expect("Missing YAML_CONF_PATH in .env file");
        let mut yaml_f =
            std::fs::File::open(yaml_conf_path).expect("Unable to find config yaml file");
        let mut yaml_str = String::new();
        yaml_f
            .read_to_string(&mut yaml_str)
            .expect("Unable to read yaml file into string");
        let docs = YamlLoader::load_from_str(&yaml_str).expect("Unable parse YAML string");

        println!("YMAL: {:?}", docs);

        // TODO we need rate limiter yaml config
        RateLimiter { redis_con: con }
    }

    pub fn recv_request(&self) -> RateLimiterResponse {
        RateLimiterResponse::Success
    }
}
