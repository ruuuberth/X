//! `x-jni` — cdylib que expone `xcore` a Kotlin vía JNI.
//!
//! Protocolo: cada función devuelve JSON `{"ok":bool,…}`, y ante error
//! `{"ok":false,"error":"..."}` (sin excepciones cruzando el puente).
//! Regla Android: Kotlin debe llamar las funciones de red desde
//! `Dispatchers.IO`; internamente bloqueamos con un runtime tokio.
//!
//! Estado (P1): sesión por cookies en `filesDir/xsession.json`.
//! Cifrado con Android Keystore llega en P6.

use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use serde_json::{json, Value};
use xcore::graphql::GqlClient;
use xcore::session::Session;
use xcore::{version, APP_CODENAME};

struct Core {
    dir: PathBuf,
    session: Mutex<Option<Session>>,
    rt: tokio::runtime::Runtime,
}

static CORE: OnceLock<Core> = OnceLock::new();

impl Core {
    fn session_path(&self) -> PathBuf {
        self.dir.join("xsession.json")
    }

    fn load_session(&self) -> Option<Session> {
        let raw = fs::read_to_string(self.session_path()).ok()?;
        Session::from_json(&raw).ok()
    }

    fn persist(&self, s: &Session) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let json = s.to_json().map_err(|e| e.to_string())?;
        fs::write(self.session_path(), json).map_err(|e| e.to_string())
    }
}

fn read_jstr(env: &mut JNIEnv, s: &JString) -> String {
    env.get_string(s)
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn err_json(e: xcore::errors::Error) -> Value {
    json!({"ok": false, "error": e.to_string()})
}

fn reply(env: &mut JNIEnv, v: Value) -> jstring {
    env.new_string(v.to_string()).unwrap().into_raw()
}

// ---------------- JNI ----------------

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_com_ruuuberth_xclient_RustBridge_ping(
    mut env: JNIEnv,
    _class: JClass,
    msg: JString,
) -> jstring {
    let m = read_jstr(&mut env, &msg);
    let reply = format!("[{APP_CODENAME}] pong: {m} (core {})", version());
    env.new_string(reply).unwrap_or_default().into_raw()
}

/// Carga (o crea) el core: recibe el `filesDir` absoluto de la app y devuelve
/// la sesión persistida (`session: null` si no hay).
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_com_ruuuberth_xclient_RustBridge_initCore(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
) -> jstring {
    let dir = PathBuf::from(read_jstr(&mut env, &data_dir));
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let core = Core {
        dir,
        session: Mutex::new(None),
        rt,
    };
    let _ = CORE.set(core); // si ya estaba (Activity recreada), conservamos el existente
    let Some(c) = CORE.get() else {
        return reply(
            &mut env,
            json!({"ok": false, "error": "fallo al inicializar core"}),
        );
    };
    match c.load_session() {
        Some(s) => {
            *c.session.lock().unwrap() = Some(s.clone());
            reply(
                &mut env,
                json!({"ok": true, "session": serde_json::to_value(&s).unwrap_or(Value::Null)}),
            )
        }
        None => reply(&mut env, json!({"ok": true, "session": Value::Null})),
    }
}

/// Login web (llamado tras completar el WebView o pegado manual).
/// Recibe el header `Cookie:` crudo; valida, verifica LIVE contra
/// v1.1 settings (aprende el handle) y persiste. Kotlin lo llama desde IO.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_com_ruuuberth_xclient_RustBridge_loginWithCookies(
    mut env: JNIEnv,
    _class: JClass,
    raw_cookies: JString,
) -> jstring {
    let raw = read_jstr(&mut env, &raw_cookies);
    let Some(c) = CORE.get() else {
        return reply(
            &mut env,
            json!({"ok": false, "error": "core no inicializado; initCore primero"}),
        );
    };

    let mut s = match Session::from_pasted(&raw) {
        Ok(s) => s,
        Err(e) => return reply(&mut env, err_json(e)),
    };

    let client = match GqlClient::new(s.clone()) {
        Ok(client) => client,
        Err(e) => return reply(&mut env, err_json(e)),
    };

    // verificación live: si esto falla, las cookies no sirven.
    match c.rt.block_on(client.account_settings()) {
        Ok(v) => {
            if let Some(h) = v.get("screen_name").and_then(Value::as_str) {
                s.handle = h.to_string();
            }
        }
        Err(e) => return reply(&mut env, err_json(e)),
    }

    if let Some(msg) = c.persist(&s).err() {
        return reply(&mut env, json!({"ok": false, "error": msg}));
    }
    *c.session.lock().unwrap() = Some(s.clone());
    reply(
        &mut env,
        json!({"ok": true, "session": serde_json::to_value(&s).unwrap_or(Value::Null)}),
    )
}

/// Devuelve v1.1 settings de la sesión actual (verificación equivalente y debug).
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_com_ruuuberth_xclient_RustBridge_me(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let Some(c) = CORE.get() else {
        return reply(
            &mut env,
            json!({"ok": false, "error": "core no inicializado; initCore primero"}),
        );
    };
    let Some(s) = c.session.lock().unwrap().clone() else {
        return reply(&mut env, json!({"ok": false, "error": "sin sesión"}));
    };
    let Ok(client) = GqlClient::new(s) else {
        return reply(&mut env, json!({"ok": false, "error": "registro inválido"}));
    };
    match c.rt.block_on(client.account_settings()) {
        Ok(v) => reply(&mut env, json!({"ok": true, "settings": v})),
        Err(e) => reply(&mut env, err_json(e)),
    }
}

/// Cierra sesión: borra el archivo persistente y limpia el core.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_com_ruuuberth_xclient_RustBridge_logout(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let Some(c) = CORE.get() else {
        return reply(
            &mut env,
            json!({"ok": false, "error": "core no inicializado; initCore primero"}),
        );
    };
    *c.session.lock().unwrap() = None;
    let _ = fs::remove_file(c.session_path());
    reply(&mut env, json!({"ok": true}))
}
