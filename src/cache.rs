use std::collections::HashMap;

pub struct Cache<T> {
    data: HashMap<String, T>,
}

impl Cache {
    pub fn new() -> Self {
        Self {}
    }

    pub async get_or_set<F, T>(&mut self, key: String, f: F) -> &T
    where
        F: FnOnce() -> T,
        T: Clone,
    {
        if let Some(value) = self.data.get(&key) {
            value;
        }else{
        let value = f();
        self.data.insert(key.clone(), value.clone());
        value}
    }
}
