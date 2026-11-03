//! `x-jni` — cdylib que expone `xcore` a Kotlin vía JNI.
//!
//! Estrategia (P0): funciones manuales por operación, con JSON
//! (serde_json) en ambos sentidos se añade desde P1.
//! Crates futuro importante: `ndk-context` (JavaVM), `tokio` (runtime),
//! callbacks de progreso (descarga/subida de media).
//!
//! Bridge de P0 mínimo: `ping` para validar toolchain extremo a extremo.

use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use xcore::APP_CODENAME;

#[no_mangle]
pub extern "system" fn Java_com_ruuuberth_xclient_RustBridge_ping(
    mut env: JNIEnv,
    _class: JClass,
    msg: JString,
) -> jstring {
    let m: String = env
        .get_string(&msg)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "desconocido".into());
    let reply = format!("[{APP_CODENAME}] pong: {m} (core {})", xcore::version());
    env.new_string(reply).unwrap().into_raw()
}
