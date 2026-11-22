//! JNI entry points. Each function name must match the Kotlin declaration in
//! `dev.stroid.app.NativeBridge` (package `dev.stroid.app`, object `NativeBridge`).

use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;

/// Converts a Rust string into a Java string, or null if allocation fails.
fn to_jstring(env: &mut JNIEnv, value: &str) -> jstring {
    match env.new_string(value) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeBridge_nativeVersion<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    to_jstring(&mut env, stroid_core::version())
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeBridge_nativeGreet<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    name: JString<'local>,
) -> jstring {
    let name: String = match env.get_string(&name) {
        Ok(s) => s.into(),
        Err(_) => String::new(),
    };
    let greeting = stroid_core::greet(&name);
    to_jstring(&mut env, &greeting)
}
