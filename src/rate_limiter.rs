use crate::storage_handler::StorageHandler;
use std::path::Path;
use std::str::FromStr;

use chrono::Timelike;
use dotenv::dotenv;
use std::collections::HashMap;
use std::{env, io::Read};
use yaml_rust::YamlLoader;

//! This rate limiter uses a sliding window alogorithm to determine whether a user's request shoud
//! be dropped or passed on. The conig/rate_limiter_config.yaml file sets the number of requests
//! per minute per access type (ex. login, message, etc)

/// User requests are passed to the RateLimiter struct via ```recv_request```.
/// If ```recv_request``` retuns ```RateLimiterResponse::Success``` the request can be forwarded to
/// the actual servers. if ```RateLimiterResponse::Drop``` is returned, the client has exceeded
/// their allowed request per unit time, and the request should be dropped.
pub struct RateLimiter {
    /// Stores past user requests
    storage_handler: StorageHandler,
    /// Maps ```RequestType``` to the limit per unit time (minute)
    conf: HashMap<RequestType, u32>,
}

/// We map a request to a RequestType so our RateLimiter knows what the config is for that request
/// Example: We may only allow 5 login events per hour, vs 10 message requests every minute
/// A client's request must be mapped to one of these for the RateLimiter to know the allowed rate
/// for the type of request. Example, ```RequestType::Login``` may only allow 2 requests per minute
/// while ```RequestType::Message``` may allow 5.
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
    Drop,
    Success,
}

impl RequestType {
    /// We use this to create keys
    fn to_string(&self) -> String {
        match self {
            RequestType::Login => String::from("Login"),
            RequestType::Message => String::from("Message"),
        }
    }
}

impl RateLimiter {
    /// Create a new RateLimiter. Reads .env file for yaml config,
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

        let mut conf: HashMap<RequestType, u32> = HashMap::new();

        let bucket = &docs[0]["domains"][0][&RequestType::Message.to_string()[..]]
            .as_i64()
            .unwrap();
        println!("message_bucket:{:?} ", bucket);
        conf.insert(RequestType::Message, *bucket as u32);

        let bucket = &docs[0]["domains"][1][&RequestType::Login.to_string()[..]]
            .as_i64()
            .unwrap();
        println!("message_bucket:{:?} ", bucket);
        conf.insert(RequestType::Login, *bucket as u32);

        RateLimiter {
            storage_handler,
            conf,
        }
    }

    /// Retreives the configured max number of requests in the one minute window for a specific request type
    fn get_bucket_size_for_request_type(&self, req_type: RequestType) -> u32 {
        *self
            .conf
            .get(&req_type)
            .expect("Unknown request type in conf")
    }

    /// Returns the key into the DB for a RequestType,user_id tuple
    fn get_request_key(req: &RequestType, user_id: u32) -> String {
        String::from(format!("{}:{}", req.to_string(), user_id))
    }

    /// User requests should be passed into ```recv_request```.
    /// This implements our rate limiter algorithm (see README) and retuns whether the request
    /// should be dropped (```RateLimiterResponse::Drop```) (limit already hit), or if the request should be forwarded (```RateLimiterResponse::Success```) to the server.
    pub fn recv_request(&mut self, req: RequestType, user_id: u32) -> RateLimiterResponse {
        let key = RateLimiter::get_request_key(&req, user_id);
        let bucket_size = self.get_bucket_size_for_request_type(req);
        let past_user_reqs: Vec<String> = self.storage_handler.get(&key);

        let now = chrono::Utc::now();

        // remove request which were in a past sliding window
        for req in &past_user_reqs {
            let req_time: chrono::DateTime<chrono::Utc> =
                chrono::DateTime::from_str(req).expect("Redis key was not a parsable date");

            if req_time < (now - chrono::Duration::minutes(1)) {
                self.storage_handler.pop_oldest_request(&key);
                continue;
            }
            break; // we can break since they are in order
        }
        let past_user_reqs: Vec<String> = self.storage_handler.get(&key); // get updated list from redis

        let percent_into_current_window = now.second() as f32 / 60.0;

        // find number of requests in current minute window
        let mut num_requests_in_current_window = 0;
        for req in past_user_reqs.iter().rev() {
            let req_time: chrono::DateTime<chrono::Utc> =
                chrono::DateTime::from_str(req).expect("Redis key was not a parsable date");
            if req_time > now.with_second(0).unwrap() {
                num_requests_in_current_window += 1
            } else {
                break;
            }
        }

        let num_requests_in_prev_window = past_user_reqs.len() - num_requests_in_current_window;
        let rolling_requests = num_requests_in_current_window as f32
            + num_requests_in_prev_window as f32 * (1.0 - percent_into_current_window);

        if rolling_requests < (bucket_size as f32) {
            self.storage_handler.append(&key, &now.to_rfc3339());
            RateLimiterResponse::Success
        } else {
            RateLimiterResponse::Drop
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use crate::rate_limiter::*;

    #[test]
    fn one_req() {
        let user_id = 1;
        let request_key = RateLimiter::get_request_key(&RequestType::Message, user_id); // to remove what we've added
        let mut rl = RateLimiter::new();
        rl.storage_handler.remove_users_past_requests(&request_key); // to ensure past test runs don't mess up
        let resp = rl.recv_request(RequestType::Message, user_id);
        assert_eq!(resp, RateLimiterResponse::Success);
    }

    #[test]
    fn multiple_requests_ok_immediate() {
        let user_id = 2;
        let request_key = RateLimiter::get_request_key(&RequestType::Message, user_id); // to remove what we've added
        let mut rl = RateLimiter::new();
        rl.storage_handler.remove_users_past_requests(&request_key); // to ensure past test runs don't mess up
        let max_num_requests_in_window = rl.get_bucket_size_for_request_type(RequestType::Message);
        for _ in 0..max_num_requests_in_window {
            let resp = rl.recv_request(RequestType::Message, user_id);
            assert_eq!(resp, RateLimiterResponse::Success);
        }
    }

    #[test]
    fn multiple_requests_not_ok_immediate() {
        let user_id = 3;
        let mut rl = RateLimiter::new();
        let request_key = RateLimiter::get_request_key(&RequestType::Message, user_id); // to remove what we've added
        rl.storage_handler.remove_users_past_requests(&request_key); // to ensure past test runs don't mess up
        let max_num_requests_in_window = rl.get_bucket_size_for_request_type(RequestType::Message);
        for _ in 0..max_num_requests_in_window {
            let resp = rl.recv_request(RequestType::Message, user_id);
            assert_eq!(resp, RateLimiterResponse::Success);
        }
        let resp = rl.recv_request(RequestType::Message, user_id);
        assert_eq!(resp, RateLimiterResponse::Drop);
    }

    #[test]
    fn multiple_requests_ok_timed() {
        let user_id = 4;
        let mut rl = RateLimiter::new();
        let request_key = RateLimiter::get_request_key(&RequestType::Message, user_id); // to remove what we've added
        rl.storage_handler.remove_users_past_requests(&request_key); // to ensure past test runs don't mess up

        let max_num_requests_in_window = rl.get_bucket_size_for_request_type(RequestType::Message);
        for _ in 0..max_num_requests_in_window {
            let resp = rl.recv_request(RequestType::Message, user_id);
            assert_eq!(resp, RateLimiterResponse::Success);
            sleep(std::time::Duration::new(2, 0));
        }
        let _ = rl.recv_request(RequestType::Message, user_id); // this can go either way given where the minute boundary is (we use waited rolling_requests)
        let resp = rl.recv_request(RequestType::Message, user_id);
        assert_eq!(resp, RateLimiterResponse::Drop);
        sleep(std::time::Duration::new(55, 0)); // window size is 60 seconds, so by now should be able to request more
        let resp = rl.recv_request(RequestType::Message, user_id);
        assert_eq!(resp, RateLimiterResponse::Success);
        sleep(std::time::Duration::new(5, 0));
        let resp = rl.recv_request(RequestType::Message, user_id);
        assert_eq!(resp, RateLimiterResponse::Success);
        sleep(std::time::Duration::new(5, 0));
        let resp = rl.recv_request(RequestType::Message, user_id);
        assert_eq!(resp, RateLimiterResponse::Success);
    }
}
