use lambda_http::lambda;

use commentable_rs::utils::http::ok;

fn main() {
  lambda!(|_, _| Ok(ok("")));
}
