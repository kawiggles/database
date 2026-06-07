pub mod store;
pub mod cli;

#[cfg(test)]
mod tests {
    use crate::{cli::Call, store::{Store, Value, DbError}};

    #[test]
    fn insert_and_get() {
        let input = "test";
        let key = "key";
        let mut db = Store::build();
        let _ = db.put(key, Value::Text(String::from(input)));
        assert_eq!("test", db.get(key).unwrap().as_text().unwrap());
    }

    #[test]
    fn insert_and_remove() {
        let input = "test";
        let key = "key";
        let mut db = Store::build();
        let _ = db.put(key, Value::Text(String::from(input)));
        assert_eq!("test", db.get(key).unwrap().as_text().unwrap());
    }

    #[test]
    fn test_parse_get() {
        let mut db = Store::build();
        let _ = db.put("key", Value::Text(String::from("test")));
        let test: Call = Call::parse("get key").unwrap();
        assert_eq!(test.execute(&mut db).unwrap(), db.get("key").unwrap());
    }

    #[test]
    fn test_parse_put() {
        let mut db = Store::build();
        let test: Call = Call::parse("put key value").unwrap();
        assert_eq!(test.execute(&mut db).unwrap(), db.get("key").unwrap());
    }

    #[test]
    fn test_parse_del() {
        let mut db = Store::build();
        let _ = db.put("key", Value::Int(3));
        let test: Call = Call::parse("del key").unwrap();
        assert_eq!(3, test.execute(&mut db).unwrap().as_int().unwrap());
        assert!(db.get("key").is_err());
    }
}
