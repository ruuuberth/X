//! `xctl` — harness de desarrollo (interacción humana con sesión real).
//!
//! No es parte de la app: corre en tu teléfono/Termux con TUS cookies y
//! captura JSON crudo bajo `~/fixtures/` para desarrollar los mapeadores de
//! `xcore` contra payloads vivos (y validar que los queryIds semilla
//! funcionen hoy). Las cookies jamás salen del dispositivo.
//!
//! Comandos:
//! - `login <archivo->|-`  : pega cookies (header Cookie: o JSON) → `~/.xctl/session.json`
//! - `me`                  : v1.1 settings — verifica sesión viva, muestra handle
//! - `user <handle>`       : UserByScreenName → fixture + resumen

use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};
use xcore::graphql::GqlClient;
use xcore::session::Session;

type Res<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run(args));
    std::process::exit(code);
}

async fn run(args: Vec<String>) -> i32 {
    match args.first().map(String::as_str) {
        Some("-h" | "--help" | "help") | None => {
            usage();
            if args.is_empty() {
                2
            } else {
                0
            }
        }
        Some("login") => wrap(cmd_login(args.get(1).map(String::as_str)).await),
        Some("me") => wrap(cmd_me().await),
        Some("user") => match args.get(1) {
            Some(h) => wrap(cmd_user(h).await),
            None => {
                usage();
                2
            }
        },
        Some(other) => {
            eprintln!("xctl: comando desconocido: {other}");
            usage();
            2
        }
    }
}

fn wrap(r: Res) -> i32 {
    match r {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("xctl: {e}");
            1
        }
    }
}

fn usage() {
    eprintln!("uso: xctl <login <archivo|->|me|user <handle>>");
    eprintln!("  login -        pega cookies por stdin (header `Cookie:` o JSON)");
    eprintln!("  login file    o lee de un archivo");
    eprintln!(
        "  env: XCTL_SESSION (default ~/.xctl/session.json), XCTL_FIXTURES (default ~/fixtures)"
    );
}

// ---------------- rutas ----------------

fn session_path() -> PathBuf {
    if let Some(p) = std::env::var_os("XCTL_SESSION") {
        return PathBuf::from(p);
    }
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".xctl").join("session.json"))
        .unwrap_or_else(|| PathBuf::from(".xctl-session.json"))
}

fn fixtures_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("XCTL_FIXTURES") {
        return PathBuf::from(p);
    }
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join("fixtures"))
        .unwrap_or_else(|| PathBuf::from("fixtures"))
}

fn save_json(path: &PathBuf, v: &Value) -> Res {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, serde_json::to_vec_pretty(v)?)?;
    Ok(())
}

fn load_session() -> Res<Session> {
    let raw = fs::read_to_string(session_path())?;
    Ok(Session::from_json(&raw)?)
}

// ---------------- comandos ----------------

/// `login <file|->`: valida lo pegado, guarda sesión y la **verifica live**
/// contra v1.1 settings (aprende el handle propio).
async fn cmd_login(src: Option<&str>) -> Res {
    let Some(src) = src else {
        usage();
        std::process::exit(2);
    };
    let raw = if src == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        fs::read_to_string(src)?
    };

    let s = Session::from_pasted(&raw)?;
    let path = session_path();
    save_json(&path, &json!(s))?;
    println!("sesión guardada en {}", path.display());

    // verificación live: si las cookies están muertas, esto lo dice ahora.
    let c = GqlClient::new(s)?;
    let v = c.account_settings().await?;
    let mut s2 = c.session();
    if let Some(h) = v.get("screen_name").and_then(Value::as_str) {
        s2.handle = h.to_string();
        save_json(&path, &json!(s2))?;
        println!("sesión viva: @{}", h);
    } else {
        println!("sesión viva (settings OK, sin screen_name en body)");
    }
    save_json(&fixtures_dir().join("settings.json"), &v)?;
    println!("fixture: {}/settings.json", fixtures_dir().display());
    Ok(())
}

/// `me`: imprime v1.1 account/settings (ver equivalenta visual de sesión viva).
async fn cmd_me() -> Res {
    let s = load_session()?;
    let c = GqlClient::new(s)?;
    let v = c.account_settings().await?;
    save_json(&fixtures_dir().join("settings.json"), &v)?;
    println!("{}", serde_json::to_string_pretty(&v)?);
    println!("fixture: {}/settings.json", fixtures_dir().display());
    Ok(())
}

/// `user <handle>`: UserByScreenName → fixture crudo + resumen legible.
async fn cmd_user(handle: &str) -> Res {
    let s = load_session()?;
    let c = GqlClient::new(s)?;

    // variables/features espejo exacto de twscrape user_by_login (verificado hoy)
    let variables = json!({
        "screen_name": handle.trim_start_matches('@'),
        "withSafetyModeUserFields": true,
    });
    let extra = json!({
        "highlights_tweets_tab_ui_enabled": true,
        "hidden_profile_likes_enabled": true,
        "creator_subscriptions_tweet_preview_api_enabled": true,
        "hidden_profile_subscriptions_enabled": true,
        "subscriptions_verification_info_verified_since_enabled": true,
        "subscriptions_verification_info_is_identity_verified_enabled": false,
        "responsive_web_twitter_article_notes_tab_enabled": false,
        "subscriptions_feature_can_gift_premium": false,
        "profile_label_improvements_pcf_label_in_post_enabled": false,
    });
    let extra_map = extra
        .as_object()
        .expect("literal features es objeto")
        .clone();

    let v = c
        .get("UserByScreenName", variables, Some(&extra_map), None)
        .await?;

    // persistir la sesión con el ct0 posiblemente rotado.
    save_json(&session_path(), &json!(c.session()))?;

    let fixture = fixtures_dir()
        .join("user")
        .join(format!("{}.json", handle.trim_start_matches('@')));
    save_json(&fixture, &v)?;

    // resumen (tolerante a ausencia de campos)
    let r = &v["data"]["user"]["result"];
    if r.is_null() {
        println!(
            "(sin data.user.result — payload crudo en {})",
            fixture.display()
        );
        return Ok(());
    }
    let legacy = &r["legacy"];
    println!(
        "@{} (id {}) — {}",
        gstr(legacy, "screen_name"),
        gstr(r, "rest_id"),
        gstr(legacy, "name"),
    );
    println!(
        "followers={} following={} tweets={} created={}",
        num(legacy, "followers_count"),
        num(legacy, "friends_count"),
        num(legacy, "statuses_count"),
        gstr(legacy, "created_at"),
    );
    let desc = gstr(legacy, "description");
    if !desc.is_empty() {
        println!("bio: {}", desc);
    }
    println!("fixture: {}", fixture.display());
    Ok(())
}

fn gstr(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("?").to_string()
}

fn num(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(-1)
}
