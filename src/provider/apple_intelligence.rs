//! On-device LLM provider backed by Apple's FoundationModels framework.
//!
//! This module is gated to macOS 26+ on Apple Silicon and only compiles when
//! the `apple-intelligence` Cargo feature is enabled. The actual streaming
//! happens in a Swift bridge (see `apple_intelligence_bridge.swift`); this
//! file is the thin Rust wrapper around its C ABI.

use super::ApiEvent;
use crate::config::ProviderConfig;
use std::ffi::{c_char, c_void, CStr, CString};
use tokio::sync::mpsc;

unsafe extern "C" {
    fn apple_intelligence_available() -> i32;
    fn apple_intelligence_unavailable_reason(buf: *mut c_char, len: usize) -> usize;
    fn apple_intelligence_stream(
        instructions: *const c_char,
        prompt: *const c_char,
        user_data: *mut c_void,
        on_delta: extern "C" fn(*mut c_void, *const c_char),
        on_done: extern "C" fn(*mut c_void, i32, *const c_char),
    );
}

extern "C" fn on_delta(user_data: *mut c_void, text: *const c_char) {
    if user_data.is_null() || text.is_null() {
        return;
    }
    let tx = unsafe { &*(user_data as *const mpsc::UnboundedSender<ApiEvent>) };
    let s = unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned();
    let _ = tx.send(ApiEvent::Delta(s));
}

extern "C" fn on_done(user_data: *mut c_void, status: i32, err: *const c_char) {
    if user_data.is_null() {
        return;
    }
    let tx = unsafe { &*(user_data as *const mpsc::UnboundedSender<ApiEvent>) };
    if status == 0 {
        let _ = tx.send(ApiEvent::Done);
    } else {
        let msg = if err.is_null() {
            "Apple Intelligence stream failed".to_string()
        } else {
            unsafe { CStr::from_ptr(err) }
                .to_string_lossy()
                .into_owned()
        };
        let _ = tx.send(ApiEvent::Error(msg));
    }
}

pub async fn stream(
    _cfg: &ProviderConfig,
    query: &str,
    system_prompt: &str,
    tx: mpsc::UnboundedSender<ApiEvent>,
) {
    // Fast-path availability check so we surface a clean error instead of
    // hanging in the Swift Task when Apple Intelligence is off.
    if unsafe { apple_intelligence_available() } != 0 {
        let mut buf = [0u8; 256];
        let n = unsafe {
            apple_intelligence_unavailable_reason(buf.as_mut_ptr() as *mut c_char, buf.len())
        };
        let reason = std::str::from_utf8(&buf[..n])
            .unwrap_or("Apple Intelligence is unavailable")
            .to_string();
        let _ = tx.send(ApiEvent::Error(reason));
        return;
    }

    let instructions = match CString::new(system_prompt) {
        Ok(s) => s,
        Err(_) => {
            let _ = tx.send(ApiEvent::Error(
                "system_prompt contains a NUL byte".to_string(),
            ));
            return;
        }
    };
    let prompt = match CString::new(query) {
        Ok(s) => s,
        Err(_) => {
            let _ = tx.send(ApiEvent::Error("query contains a NUL byte".to_string()));
            return;
        }
    };

    // The Swift bridge blocks the calling thread until the async Task
    // finishes, so move it onto a blocking pool to keep the tokio runtime
    // responsive.
    let _ = tokio::task::spawn_blocking(move || {
        let boxed: Box<mpsc::UnboundedSender<ApiEvent>> = Box::new(tx);
        let user_data = Box::into_raw(boxed) as *mut c_void;
        unsafe {
            apple_intelligence_stream(
                instructions.as_ptr(),
                prompt.as_ptr(),
                user_data,
                on_delta,
                on_done,
            );
            // Reclaim the sender so it's dropped exactly once.
            drop(Box::from_raw(
                user_data as *mut mpsc::UnboundedSender<ApiEvent>,
            ));
        }
    })
    .await;
}
