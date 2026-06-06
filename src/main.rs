use database::store::{Store, DbError};
use database::cli::{get_input, Call};

fn main() {
    let mut db = Store::build();
    loop {
        // TODO: fix this
        let call_result: Result<Call, DbError> = Call::parse(get_input().as_str());

        match call_result {
            Ok(call) => {
                if call == Call::Exit {
                    break;
                }
                match call.execute(&mut db) {
                    Ok(value) => {
                        println!("Operation successful, return value is {}", value.print());
                    },
                    Err(err) => eprintln!("Error: {}", err),
                }
            }
            Err(err) => eprintln!("Error: {}", err),
        }
    }
    println!("Exiting program...");
}
