use dotenv::dotenv;
use rate_limiter::rl::rate_limiter;

fn main() {
    let rl = rate_limiter::RateLimiter::new(String::from("example_url"));
}
