use crate::rl::storage_handler::StorageHandler;
use std::path::{Path, PathBuf};

use chrono::Duration;
use dotenv::dotenv;
use std::collections::HashMap;
use std::{env, io::Read};
use yaml_rust::YamlLoader;

/// Used as value in Redis for a past request
struct RequestLog {}

/// conf maps the domain to the max number of requests per set duration unit
pub struct RateLimiter {
    storage_handler: StorageHandler,
    conf: HashMap<RequestType, u32>, // needs redis connection (u32 is req per min)
}

/// The RateLimiter will receive a Request, query redis to retreiv
/// We map a request to a RequestType so our RateLimiter knows what the config is for that request
/// Example: We may only allow 5 login events per hour, vs 10 message requests every minute
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum RequestType {
    Login,
    Message,
}

/// RateLimiter will respond indicating whether or not the request can succeed (request limit
/// hasn't been hit), or whether the request should be dropped (request limit hit), including a
/// message for the client
#[derive(Debug, PartialEq, Eq)]
pub enum RateLimiterResponse {
    Drop(String),
    Success,
}

impl RateLimiter {
    pub fn new() -> RateLimiter {
        dotenv().ok(); // load .env variables to environment

        let storage_handler = StorageHandler::new();

        // retrieve rate limiter config
        let yaml_conf_path =
            Path::new(&env::var("CARGO_MANIFEST_DIR").expect("Missing CARGO_MANIFEST_DIR env var"))
                .join(env::var("YAML_CONF_PATH").expect("Missing YAML_CONF_PATH env var"));
        let mut yaml_f =
            std::fs::File::open(yaml_conf_path).expect("Unable to find config yaml file");
        let mut yaml_str = String::new();
        yaml_f
            .read_to_string(&mut yaml_str)
            .expect("Unable to read yaml file into string");
        let docs = YamlLoader::load_from_str(&yaml_str).expect("Unable parse YAML string");

        // TEMP
        let mut conf: HashMap<RequestType, u32> = HashMap::new();
        conf.insert(RequestType::Message, 5);

        //println!("{:?} ", docs[0]["domains"][0]["messaging"]);
        ///println!("{:?} ", docs[0]["domains"][1][0]);
        //println!("{:?} ", docs[0]["domains"][0]["messaging"]);

        //println!("{:?} ", docs[0]["A"][0][0].as_str());
        //let domain = docs[0]["domains"][0].as_str().unwrap();
        //let requests_per_minute = docs[0]["domains"][0]["requests_per_unit"].as_i64().unwrap();
        //println!("{} {}", domain, requests_per_minute);

        //let x = docs[0].as_str().unwrap();

        //println!("YMAL: {}", x);
        // TODO we need rate limiter yaml config
        RateLimiter {
            storage_handler,
            conf,
        }
    }

    pub fn recv_request(&self, req: RequestType) -> RateLimiterResponse {
        let key = stringify!(req); // temp. instead use <request_type>:<user_id>
        let bucket_size = self.conf.get(&req).expect("Unknown request type in conf");
        let past_user_req = self.storage_handler.get(key);

        if past_user_req.len() < (*bucket_size as usize) {
            // we promote bucket_size since num bits for usize >= i32 (>= since it depends on machine)
            RateLimiterResponse::Success
        } else {
            RateLimiterResponse::Drop(String::from(
                "You have no request tokens left... Take a coffee break",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rl::rate_limiter::*;

    #[test]
    fn one_req() {
        let rl = RateLimiter::new();
        let resp = rl.recv_request(RequestType::Message);
        assert_eq!(resp, RateLimiterResponse::Success);
    }
}
