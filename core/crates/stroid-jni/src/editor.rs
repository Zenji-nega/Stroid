//! JNI functions for `dev.stroid.app.NativeEditor`.
//!
//! An editor is owned by Rust and referred to from Kotlin by an opaque `long`
//! handle returned from `create`/`open` and released with `destroy`. All
//! positions are Unicode scalar offsets (see `stroid_core::editor`).

use crate::util::{get_string, new_int_array, new_jstring, throw, to_i32};
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jint, jintArray, jlong, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use std::path::Path;
use std::sync::Mutex;
use stroid_core::editor::{Editor, Movement, SearchOptions};

struct EditorHandle(Mutex<Editor>);

fn into_handle(editor: Editor) -> jlong {
    Box::into_raw(Box::new(EditorHandle(Mutex::new(editor)))) as jlong
}

/// Runs `f` with the editor behind `handle`.
fn with<R>(handle: jlong, f: impl FnOnce(&mut Editor) -> R) -> R {
    assert!(handle != 0, "editor handle is null");
    // SAFETY: the handle was produced by `into_handle` and Kotlin guarantees it
    // is not used after `destroy` (see EditorBuffer).
    let h = unsafe { &*(handle as *const EditorHandle) };
    let mut guard = h.0.lock().unwrap_or_else(|p| p.into_inner());
    f(&mut guard)
}

fn jbool(b: bool) -> jboolean {
    if b {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

fn options(case_sensitive: jboolean, whole_word: jboolean, regex: jboolean) -> SearchOptions {
    SearchOptions {
        case_sensitive: case_sensitive != 0,
        whole_word: whole_word != 0,
        regex: regex != 0,
    }
}

fn pos(v: jint) -> usize {
    v.max(0) as usize
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_create<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jlong {
    into_handle(Editor::new())
}

/// Throws `java.io.IOException` (returning 0) if the file cannot be read as UTF-8.
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_open<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    path: JString<'l>,
) -> jlong {
    let path = get_string(&mut env, &path);
    match Editor::open(Path::new(&path)) {
        Ok(editor) => into_handle(editor),
        Err(e) => {
            throw(&mut env, "java/io/IOException", &format!("{path}: {e}"));
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_destroy<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) {
    if handle != 0 {
        // SAFETY: see `with`; this is the single owner releasing the box.
        drop(unsafe { Box::from_raw(handle as *mut EditorHandle) });
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_save<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    path: JString<'l>,
) {
    let path = get_string(&mut env, &path);
    if let Err(e) = with(handle, |ed| ed.save(Path::new(&path))) {
        throw(&mut env, "java/io/IOException", &format!("{path}: {e}"));
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_isModified<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jboolean {
    jbool(with(handle, |ed| ed.is_modified()))
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_lenChars<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jint {
    to_i32(with(handle, |ed| ed.len_chars()))
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_lineCount<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jint {
    to_i32(with(handle, |ed| ed.line_count()))
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_lineLen<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    line: jint,
) -> jint {
    to_i32(with(handle, |ed| ed.line_len(pos(line))))
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_text<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jstring {
    let text = with(handle, |ed| ed.text());
    new_jstring(&mut env, &text)
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_slice<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    start: jint,
    end: jint,
) -> jstring {
    let text = with(handle, |ed| ed.slice(pos(start), pos(end)));
    new_jstring(&mut env, &text)
}

/// `count` lines starting at `first`, joined with `\n` (lines never contain
/// `\n`, so the caller can split on it).
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_lines<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    first: jint,
    count: jint,
) -> jstring {
    let joined = with(handle, |ed| ed.lines(pos(first), pos(count)).join("\n"));
    new_jstring(&mut env, &joined)
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_selectedText<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jstring {
    let text = with(handle, |ed| ed.selected_text());
    new_jstring(&mut env, &text)
}

/// Flat `[anchor0, head0, anchor1, head1, ...]`.
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_selections<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jintArray {
    let flat: Vec<i32> = with(handle, |ed| {
        ed.selections()
            .iter()
            .flat_map(|s| [to_i32(s.anchor), to_i32(s.head)])
            .collect()
    });
    new_int_array(&mut env, &flat)
}

/// `[line, column]` of a position.
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_posToLineCol<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    position: jint,
) -> jintArray {
    let (line, col) = with(handle, |ed| ed.pos_to_line_col(pos(position)));
    new_int_array(&mut env, &[to_i32(line), to_i32(col)])
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_lineColToPos<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    line: jint,
    col: jint,
) -> jint {
    to_i32(with(handle, |ed| ed.line_col_to_pos(pos(line), pos(col))))
}

// ------------------------------------------------------------------ editing

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_insert<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    text: JString<'l>,
) {
    let text = get_string(&mut env, &text);
    with(handle, |ed| ed.insert(&text));
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_insertNewline<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) {
    with(handle, |ed| ed.insert_newline());
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_deleteBackward<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) {
    with(handle, |ed| ed.delete_backward());
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_deleteForward<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) {
    with(handle, |ed| ed.delete_forward());
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_replaceRange<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    start: jint,
    end: jint,
    text: JString<'l>,
) {
    let text = get_string(&mut env, &text);
    with(handle, |ed| ed.replace_range(pos(start)..pos(end), &text));
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_undo<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jboolean {
    jbool(with(handle, |ed| ed.undo()))
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_redo<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jboolean {
    jbool(with(handle, |ed| ed.redo()))
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_canUndo<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jboolean {
    jbool(with(handle, |ed| ed.can_undo()))
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_canRedo<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jboolean {
    jbool(with(handle, |ed| ed.can_redo()))
}

// ---------------------------------------------------------------- selection

fn movement(code: jint, arg: jint) -> Option<Movement> {
    Some(match code {
        0 => Movement::Left,
        1 => Movement::Right,
        2 => Movement::Vertical(arg),
        3 => Movement::LineStart,
        4 => Movement::LineEnd,
        5 => Movement::WordLeft,
        6 => Movement::WordRight,
        7 => Movement::DocStart,
        8 => Movement::DocEnd,
        _ => return None,
    })
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_move<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    code: jint,
    arg: jint,
    extend: jboolean,
) {
    if let Some(m) = movement(code, arg) {
        with(handle, |ed| ed.move_selections(m, extend != 0));
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_selectAll<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) {
    with(handle, |ed| ed.select_all());
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_setCursor<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    position: jint,
) {
    with(handle, |ed| ed.set_cursor(pos(position)));
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_setSelection<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    anchor: jint,
    head: jint,
) {
    with(handle, |ed| ed.set_selection(pos(anchor), pos(head)));
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_addCursor<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    position: jint,
) {
    with(handle, |ed| ed.add_cursor(pos(position)));
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_addCursorVertical<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    down: jboolean,
) {
    with(handle, |ed| ed.add_cursor_vertical(down != 0));
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_selectWordAt<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    position: jint,
) {
    with(handle, |ed| ed.select_word_at(pos(position)));
}

#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_addNextOccurrence<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
) -> jboolean {
    jbool(with(handle, |ed| ed.add_next_occurrence()))
}

// ------------------------------------------------------------------- search

/// Flat `[start0, end0, start1, end1, ...]`. Throws
/// `IllegalArgumentException` for an invalid regular expression.
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_findAll<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    query: JString<'l>,
    case_sensitive: jboolean,
    whole_word: jboolean,
    regex: jboolean,
) -> jintArray {
    let query = get_string(&mut env, &query);
    let opts = options(case_sensitive, whole_word, regex);
    match with(handle, |ed| ed.find_all(&query, &opts)) {
        Ok(matches) => {
            let flat: Vec<i32> = matches
                .iter()
                .flat_map(|&(s, e)| [to_i32(s), to_i32(e)])
                .collect();
            new_int_array(&mut env, &flat)
        }
        Err(e) => {
            throw(
                &mut env,
                "java/lang/IllegalArgumentException",
                &e.to_string(),
            );
            std::ptr::null_mut()
        }
    }
}

/// Returns the number of replacements; throws like `findAll`.
#[no_mangle]
pub extern "system" fn Java_dev_stroid_app_NativeEditor_replaceAll<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    handle: jlong,
    query: JString<'l>,
    replacement: JString<'l>,
    case_sensitive: jboolean,
    whole_word: jboolean,
    regex: jboolean,
) -> jint {
    let query = get_string(&mut env, &query);
    let replacement = get_string(&mut env, &replacement);
    let opts = options(case_sensitive, whole_word, regex);
    match with(handle, |ed| ed.replace_all(&query, &replacement, &opts)) {
        Ok(n) => to_i32(n),
        Err(e) => {
            throw(
                &mut env,
                "java/lang/IllegalArgumentException",
                &e.to_string(),
            );
            0
        }
    }
}
