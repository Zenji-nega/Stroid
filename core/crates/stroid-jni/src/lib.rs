//! JNI entry points.
//!
//! * `dev.stroid.app.NativeBridge`   - version / smoke test (this file)
//! * `dev.stroid.app.NativeEditor`   - text editor engine (`editor`)
//! * `dev.stroid.app.NativeTerminal` - terminal sessions (`terminal`)
//!
//! Each function name must match its Kotlin declaration.

mod editor;
mod terminal;
mod util;

use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use util::{get_string, new_jstring};

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeBridge_nativeVersion<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    new_jstring(&mut env, stroid_core::version())
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeBridge_nativeGreet<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    name: JString<'local>,
) -> jstring {
    let name = get_string(&mut env, &name);
    new_jstring(&mut env, &stroid_core::greet(&name))
}
