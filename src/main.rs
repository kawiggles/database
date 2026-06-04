use database::store::{Store, Call};
use database::cli::get_input;

fn main() {
    let mut db = Store::build();
    loop {
        println!("Enter an API call: ");
        let call = get_input().unwrap();
        match call {
            Call::Get(key) => println!("{:?}", db.get(&key)),
            Call::Put{ key, value } => {
                println!("Inserting value at key {}", key);
                db.put(&key, value);
            },
            Call::Del(key) => {
                println!("Deleting value at {}", key);
                db.del(&key);
            },
            Call::Exit => break,
        }
    }
    println!("Exiting program...");
}
