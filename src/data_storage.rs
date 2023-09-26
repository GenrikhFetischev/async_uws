use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct DataStorage {
    pub storage: HashMap<TypeId, Box<dyn Any + Sync + Send + 'static>>,
}

impl DataStorage {
    pub fn new() -> Self {
        DataStorage {
            storage: HashMap::new(),
        }
    }

    pub fn add_data<T: Send + Sync + Clone + 'static>(&mut self, data: T) {
        let type_id = TypeId::of::<T>();
        self.storage.insert(type_id, Box::new(data));
    }

    pub fn get_data<T: Send + Sync + Clone + 'static>(&self) -> Option<&T> {
        self.storage
            .get(&TypeId::of::<T>())
            .and_then(|boxed| (&**boxed as &(dyn Any + 'static)).downcast_ref())
    }
}

pub type SharedDataStorage = Arc<DataStorage>;
