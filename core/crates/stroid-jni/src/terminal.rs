//! JNI functions for `dev.stroid.app.NativeTerminal`.
//!
//! A terminal session is owned by Rust and referred to by an opaque `long`
//! handle, like the editor (see `editor.rs`).

use crate::util::{get_string, new_int_array, new_jstring, throw, to_i32};
use jni::objects::{JByteArray, JClass, JObjectArray, JString};
use jni::sys::{jboolean, jint, jintArray, jlong, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use std::path::PathBuf;
use stroid_terminal::input::{self, Key, Mods};
use stroid_terminal::{Session, SpawnConfig};

/// Returned by `exitCode` while the process is still running.
const STILL_RUNNING: jint = jint::MIN;

fn with<R>(handle: jlong, f: impl FnOnce(&Session) -> R) -> R {
    assert!(handle != 0, "terminal handle is null");
    // SAFETY: the handle comes from `create` and Kotlin guarantees it is not
    // used after `destroy` (see TerminalSession).
    let session = unsafe { &*(handle as *const Session) };
    f(session)
}

fn jbool(b: bool) -> jboolean {
    if b {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

fn dim(v: jint) -> u16 {
    v.clamp(1, u16::MAX as jint) as u16
}

fn string_array(env: &mut JNIEnv, array: &JObjectArray) -> Vec<String> {
    let len = env.get_array_length(array).unwrap_or(0);
    let mut out = Vec::with_capacity(len.max(0) as usize);
    for i in 0..len {
        if let Ok(obj) = env.get_object_array_element(array, i) {
            let s = JString::from(obj);
            out.push(get_string(env, &s));
        }
    }
    out
}

fn mods(bits: jint) -> Mods {
    Mods {
        shift: bits & 1 != 0,
        alt: bits & 2 != 0,
        ctrl: bits & 4 != 0,
    }
}

fn key(code: jint) -> Option<Key> {
    Some(match code {
        0 => Key::Up,
        1 => Key::Down,
        2 => Key::Left,
        3 => Key::Right,
        4 => Key::Home,
        5 => Key::End,
        6 => Key::PageUp,
        7 => Key::PageDown,
        8 => Key::Insert,
        9 => Key::Delete,
        10 => Key::Enter,
        11 => Key::Tab,
        12 => Key::Backspace,
        13 => Key::Escape,
        101..=112 => Key::F((code - 100) as u8),
        _ => return None,
    })
}

/// Starts `program` in a new pseudo-terminal. `args` are the arguments after
/// the program name; `env` entries are `KEY=VALUE`. Throws `IOException`
/// (returning 0) if the process cannot be started.
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeTerminal_create<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    program: JString<'l>,
    args: JObjectArray<'l>,
    environment: JObjectArray<'l>,
    cwd: JString<'l>,
    rows: jint,
    cols: jint,
) -> jlong {
    let mut cfg = SpawnConfig::new(get_string(&mut env, &program));
    cfg.args = string_array(&mut env, &args);
    cfg.env = string_array(&mut env, &environment)
        .into_iter()
        .filter_map(|kv| {
            kv.split_once('=')
                .map(|(k, v)| (k.to_string(), v.to_string()))
        })
        .collect();
    if !cwd.is_null() {
        cfg.cwd = Some(PathBuf::from(get_string(&mut env, &cwd)));
    }
    cfg.rows = dim(rows);
    cfg.cols = dim(cols);

    match Session::spawn(&cfg) {
        Ok(session) => Box::into_raw(Box::new(session)) as jlong,
        Err(e) => {
            throw(
                &mut env,
                "java/io/IOException",
                &format!("cannot start {}: {e}", cfg.program),
            );
            0
        }
    }
}

/// Kills the process and releases the session.
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeTerminal_destroy<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) {
    if handle != 0 {
        // SAFETY: see `with`; this is the single owner releasing the box.
        drop(unsafe { Box::from_raw(handle as *mut Session) });
    }
}

/// Sends raw bytes to the program.
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeTerminal_write<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    bytes: JByteArray<'l>,
) {
    if let Ok(data) = env.convert_byte_array(&bytes) {
        let _ = with(handle, |s| s.write(&data));
    }
}

/// Sends a non-character key (`code` as in `TerminalKey`).
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeTerminal_sendKey<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    code: jint,
    modifiers: jint,
) {
    if let Some(k) = key(code) {
        with(handle, |s| {
            let app = s.with_screen(|screen| screen.app_cursor_keys());
            let _ = s.write(&input::encode_key(k, mods(modifiers), app));
        });
    }
}

/// Sends a typed Unicode code point with modifiers.
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeTerminal_sendChar<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    code_point: jint,
    modifiers: jint,
) {
    if let Some(c) = u32::try_from(code_point).ok().and_then(char::from_u32) {
        let _ = with(handle, |s| s.write(&input::encode_char(c, mods(modifiers))));
    }
}

/// Pastes text, using bracketed paste if the program enabled it.
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeTerminal_paste<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    text: JString<'l>,
) {
    let text = get_string(&mut env, &text);
    with(handle, |s| {
        let bracketed = s.with_screen(|screen| screen.bracketed_paste());
        let _ = s.write(&input::encode_paste(&text, bracketed));
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeTerminal_resize<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    rows: jint,
    cols: jint,
) {
    let _ = with(handle, |s| s.resize(dim(rows), dim(cols)));
}

/// Changes whenever the screen may have changed.
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeTerminal_generation<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jlong {
    with(handle, |s| s.generation()) as jlong
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeTerminal_isClosed<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jboolean {
    jbool(with(handle, |s| s.is_closed()))
}

/// Exit code, or `Int.MIN_VALUE` while the process is running.
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeTerminal_exitCode<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jint {
    with(handle, |s| s.exit_code()).unwrap_or(STILL_RUNNING)
}

/// Screen metadata:
/// `[rows, cols, cursorRow, cursorCol, cursorVisible, scrollbackLines,
///   altScreen, appCursorKeys, bracketedPaste, bellCount]`.
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeTerminal_info<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jintArray {
    let info = with(handle, |s| {
        s.with_screen(|screen| {
            let (row, col) = screen.cursor();
            [
                to_i32(screen.rows()),
                to_i32(screen.cols()),
                to_i32(row),
                to_i32(col),
                screen.cursor_visible() as i32,
                to_i32(screen.scrollback_len()),
                screen.alt_screen_active() as i32,
                screen.app_cursor_keys() as i32,
                screen.bracketed_paste() as i32,
                screen.bell_count() as i32,
            ]
        })
    });
    new_int_array(&mut env, &info)
}

/// The viewport as `rows * cols * 4` ints (`[codepoint, fg, bg, flags]` per
/// cell), scrolled back by `scroll_offset` lines.
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeTerminal_snapshot<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    scroll_offset: jint,
) -> jintArray {
    let cells = with(handle, |s| {
        s.with_screen(|screen| screen.encode_view(scroll_offset.max(0) as usize))
    });
    new_int_array(&mut env, &cells)
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeTerminal_title<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jstring {
    let title = with(handle, |s| {
        s.with_screen(|screen| screen.title().to_string())
    });
    new_jstring(&mut env, &title)
}
