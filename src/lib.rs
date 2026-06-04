pub mod store;
pub mod cli;

#[cfg(test)]
mod tests {
    use crate::store::{Store, Value};

    #[test]
    fn insert_and_get() {
        let input = "test";
        let key = "key";
        let mut db = Store::build();
        db.put(key, Value::Text(String::from(input)));
        assert_eq!("test", db.get(key).unwrap().as_text().unwrap());
    }

    #[test]
    fn insert_and_remove() {
        let input = "test";
        let key = "key";
        let mut db = Store::build();
        db.put(key, Value::Text(String::from(input)));
        assert_eq!("test", db.get(key).unwrap().as_text().unwrap());
    }
}
