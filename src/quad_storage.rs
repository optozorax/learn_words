use lazy_static::lazy_static;
use std::sync::Mutex;

#[cfg(not(target_arch = "wasm32"))]
use nanoserde::{DeJson, SerJson};

#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashMap;

/// The local storage with methods to read and write data.
#[cfg_attr(not(target_arch = "wasm32"), derive(DeJson, SerJson))]
pub struct LocalStorage {
    #[cfg(not(target_arch = "wasm32"))]
    local: HashMap<String, String>,
}

#[cfg(not(target_arch = "wasm32"))]
const LOCAL_FILE: &str = "local.data";

impl Default for LocalStorage {
    fn default() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self {}
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Ok(file) = std::fs::read_to_string(LOCAL_FILE) {
                LocalStorage::deserialize_json(&file).unwrap()
            } else {
                LocalStorage {
                    local: Default::default(),
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

impl LocalStorage {
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        #[cfg(target_arch = "wasm32")]
        {
            local_storage().and_then(|x| x.length().ok()).unwrap_or(0) as usize
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.local.len()
        }
    }

    /// Get key by its position
    pub fn key(&self, pos: usize) -> Option<String> {
        #[cfg(target_arch = "wasm32")]
        {
            local_storage()?.key(pos as u32).ok().flatten()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.local.keys().nth(pos).cloned()
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        #[cfg(target_arch = "wasm32")]
        {
            local_storage()?.get_item(key).ok().flatten()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.local.get(key).cloned()
        }
    }
    pub fn set(&mut self, key: &str, value: &str) {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(storage) = local_storage() {
                storage.set_item(key, value);
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.local.insert(key.to_string(), value.to_string());
            self.save();
        }
    }
    pub fn remove(&mut self, key: &str) {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(storage) = local_storage() {
                storage.remove_item(key);
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.local.remove(key);
            self.save();
        }
    }
    pub fn clear(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(storage) = local_storage() {
                storage.clear();
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.local.clear();
            self.save();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save(&self) {
        std::fs::write(LOCAL_FILE, self.serialize_json()).unwrap();
    }
}

lazy_static! {
    pub static ref STORAGE: Mutex<LocalStorage> = Mutex::new(Default::default());
}
