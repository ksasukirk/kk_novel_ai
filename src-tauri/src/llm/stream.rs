//! 流式请求取消状态
//! 代码路径: kk_novel_ai/src-tauri/src/llm/stream.rs

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Default)]
pub struct CancelRegistry {
    inner: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl CancelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, request_id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.inner.lock().insert(request_id.to_string(), flag.clone());
        flag
    }

    pub fn cancel(&self, request_id: &str) -> bool {
        if let Some(flag) = self.inner.lock().get(request_id) {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn remove(&self, request_id: &str) {
        self.inner.lock().remove(request_id);
    }

    pub fn active_count(&self) -> usize {
        self.inner.lock().len()
    }
}
