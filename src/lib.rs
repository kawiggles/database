pub mod store;

#[cfg(test)]
mod tests {
    use crate::store::{Store, Value};

    #[test]
    fn insert_and_get() {
        let input = "test";
        let key = "key";
        let mut db = Store::build();
        db.put(key, Value::text(input));
        assert_eq!(key, db.get(input).unwrap().as_text().unwrap());
    }
}
