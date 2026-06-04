use database::store::{Store, Call};
use database::cli::get_input;

fn main() {
    let db = Store::build();
    loop {
        println!("Enter an API call: ");
        let call = get_input().unwrap();
        match call {
            Call::Get(key) => println!("{:}", db.get(&key)),
        }
    }
}
