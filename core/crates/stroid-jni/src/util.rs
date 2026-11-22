//! Small helpers shared by the JNI modules.

use jni::objects::JString;
use jni::sys::{jintArray, jstring};
use jni::JNIEnv;

/// Java string -> Rust string (empty on failure).
pub fn get_string(env: &mut JNIEnv, s: &JString) -> String {
    env.get_string(s).map(String::from).unwrap_or_default()
}

/// Rust string -> Java string (null on failure).
pub fn new_jstring(env: &mut JNIEnv, s: &str) -> jstring {
    match env.new_string(s) {
        Ok(j) => j.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// `&[i32]` -> `int[]` (null on failure).
pub fn new_int_array(env: &mut JNIEnv, data: &[i32]) -> jintArray {
    let Ok(len) = i32::try_from(data.len()) else {
        return std::ptr::null_mut();
    };
    match env.new_int_array(len) {
        Ok(arr) => {
            if env.set_int_array_region(&arr, 0, data).is_err() {
                return std::ptr::null_mut();
            }
            arr.into_raw()
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Raises a Java exception; the caller then returns a dummy value.
pub fn throw(env: &mut JNIEnv, class: &str, message: &str) {
    let _ = env.throw_new(class, message);
}

/// Saturating `usize` -> `i32`.
pub fn to_i32(v: usize) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}
