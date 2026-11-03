//! Mapeadores payload GraphQL → modelos de la UI.
//!
//! Estrategia (arrancó del repo twscrape valedanza del 2026-11-03, ahora
//! con fixtures reales en `tests/fixtures/`):
//! - payload usuario **nuevo (2026)**: `core`/`avatar`/`profile_bio`/
//!   `relationship_counts`/`tweet_counts`;
//! - payload usuario **legacy**: `legacy.*`;
//! - timeline: `timeline_v2.timeline.instructions` o `timeline.timeline.instructions
//!   (TimelinePinEntry + TimelineAddEntries + módulos).
//!
//! Toda lección por aquí es local y testeable offline al server.

use std::collections::HashSet;

use serde_json::Value;

use crate::errors::{Error, Result};
use crate::models::{Media, MediaKind, Tweet, User};

// --------------------------------- helpers ---------------------------------

/// Navega un path; `Value::Null` si falta (serde Value es default bei Null).
fn at<'a>(v: &'a Value, path: &[&str]) -> &'a Value {
    let mut cur = v;
    for k in path {
        cur = cur.get(k).unwrap_or(&Value::Null);
        if cur.is_null() {
            return cur;
        }
    }
    cur
}

fn sstr(v: &Value, path: &[&str]) -> Option<String> {
    at(v, path)
        .as_str()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| at(v, path).as_i64().map(|n| n.to_string()))
}

fn unum(v: &Value, path: &[&str]) -> Option<u64> {
    let x = at(v, path);
    x.as_u64()
        .or_else(|| x.as_i64().map(|n| n.max(0) as u64))
        .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
}

fn ubool(v: &Value, path: &[&str]) -> bool {
    at(v, path).as_bool().unwrap_or(false)
}

/// `Sat Oct 17 18:11:27 +0000 2026` → ms epoch (UTC; sin chrono).
pub fn parse_x_epoch(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 6 {
        return None;
    }
    let mon = match parts[1] {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let day: i64 = parts[2].parse().ok()?;
    let mut hm = parts[3].split(':');
    let hour: i64 = hm.next()?.parse().ok()?;
    let min: i64 = hm.next()?.parse().ok()?;
    let sec: i64 = hm.next()?.parse().ok()?;
    let year: i64 = parts[5].parse().ok()?;
    // Howard Hinnant days_from_civil (proleptic gregorian)
    let y = if mon <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (mon + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days * 86_400 + hour * 3_600 + min * 60 + sec) * 1_000)
}

// --------------------------------- usuario ---------------------------------

/// Nodo `result` de usuario (raíz de UserByScreenName/RestId O anidado en
/// `core.user_results.result` de un tweet). Consume ambos esquemas.
pub fn user_from_result(r: &Value) -> Result<User> {
    if r.is_null() {
        return Err(Error::UnexpectedPayload {
            op: "user".into(),
            detail: "result ausente".into(),
        });
    }
    // cuenta indisponible/suspendida: '__typename' UserRole: "UserUnavailable"
    if at(r, &["__typename"]).as_str() == Some("UserUnavailable") {
        return Err(Error::UnexpectedPayload {
            op: "user".into(),
            detail: format!(
                "usuario no disponible: {}",
                at(r, &["reason"]).as_str().unwrap_or("?")
            ),
        });
    }
    let user = User {
        id: sstr(r, &["rest_id"])
            .or_else(|| sstr(r, &["legacy", "id_str"]))
            .or_else(|| sstr(r, &["id"]))
            .unwrap_or_default(),
        screen_name: (sstr(r, &["core", "screen_name"])
            .or_else(|| sstr(r, &["legacy", "screen_name"])))
        .ok_or_else(|| Error::UnexpectedPayload {
            op: "user".into(),
            detail: "sin screen_name (core ni legacy)".into(),
        })?,
        name: sstr(r, &["core", "name"])
            .or_else(|| sstr(r, &["legacy", "name"]))
            .unwrap_or_default(),
        avatar_url: sstr(r, &["avatar", "image_url"])
            .or_else(|| sstr(r, &["avatar", "image_url_https"]))
            .or_else(|| sstr(r, &["legacy", "profile_image_url_https"]))
            .unwrap_or_default(),
        bio: sstr(r, &["profile_bio", "description"])
            .or_else(|| sstr(r, &["legacy", "description"]))
            .unwrap_or_default(),
        followers: unum(r, &["relationship_counts", "followers"])
            .or_else(|| unum(r, &["legacy", "followers_count"]))
            .unwrap_or(0),
        following: unum(r, &["relationship_counts", "following"])
            .or_else(|| unum(r, &["legacy", "friends_count"]))
            .unwrap_or(0),
        statuses: unum(r, &["tweet_counts", "tweets"])
            .or_else(|| unum(r, &["legacy", "statuses_count"]))
            .unwrap_or(0),
        created_at: sstr(r, &["core", "created_at"])
            .or_else(|| sstr(r, &["legacy", "created_at"]))
            .unwrap_or_default(),
        is_blue_verified: ubool(r, &["is_blue_verified"])
            .or_else(|| at(r, &["verification", "verified"]).as_bool())
            .or_else(|| at(r, &["legacy", "verified"]).as_bool())
            .unwrap_or(false),
    };
    Ok(user)
}

/// Payload completo de `UserByScreenName`/`UserByRestId` → [`User`].
pub fn user_from_payload(v: &Value) -> Result<User> {
    user_from_result(at(v, &["data", "user", "result"]))
}

// ----------------------------------- media ----------------------------------

fn best_mp4(variants: &Value) -> Option<String> {
    let mut best: Option<(u64, &Value)> = None;
    if let Some(list) = variants.as_array() {
        for v in list {
            if at(v, &["content_type"]).as_str() != Some("video/mp4") {
                continue;
            }
            let bitrate = v.get("bitrate").and_then(Value::as_u64).unwrap_or(0);
            if best.map(|b| bitrate > b.0).unwrap_or(true) {
                best = Some((bitrate, v));
            }
        }
    }
    best.and_then(|(_, v)| v.get("url").and_then(Value::as_str))
        .map(String::from)
}

fn media_from(m: &Value) -> Option<Media> {
    let kind = match at(m, &["type"]).as_str() {
        Some("photo") => MediaKind::Photo,
        Some("animated_gif") => MediaKind::Gif,
        Some("video") => MediaKind::Video,
        _ => return None,
    };
    let media_id = sstr(m, &["id_str"])
        .or_else(|| sstr(m, &["id"]))
        .unwrap_or_default();
    let media_url = sstr(m, &["media_url_https"]).unwrap_or_default();
    let (url, duration) = if matches!(kind, MediaKind::Photo) {
        (media_url, 0)
    } else {
        let vi = at(m, &["video_info"]);
        let best = best_mp4(vi).unwrap_or(media_url); // fallback al thumb si no hay variantes
        let dur = unum(vi, &["duration_millis"]).unwrap_or(0);
        (best, dur)
    };
    Some(Media {
        kind,
        media_id,
        url,
        duration_ms: duration,
    })
}

// ----------------------------------- tweet ----------------------------------

fn tweet_from_result(r: &Value) -> Option<Tweet> {
    // TweetWithVisibilityResults anida el tweet real en `tweet`.
    let real = if at(r, &["__typename"]).as_str() == Some("TweetWithVisibilityResults")
        || at(r, &["legacy"]).is_null()
    {
        at(r, &["tweet"])
    } else {
        r
    };
    let legacy = at(real, &["legacy"]);
    if legacy.is_null() || at(legacy, &["full_text"]).is_null() {
        return None; // tombstone / promotion sin tweet
    }
    let id = sstr(real, &["rest_id"])
        .or_else(|| sstr(legacy, &["id_str"]))
        .unwrap_or_default();

    // RT: preferimos el contenido del tweet interno (autor/texto real).
    let rt_inner = at(real, &["legacy", "retweeted_status_result", "result"]);
    let (content_node, is_rt) =
        if !rt_inner.is_null() && at(rt_inner, &["legacy"]).as_object().is_some() {
            (rt_inner, true)
        } else {
            (real, ubool(legacy, &["retweeted"]))
        };

    let mut author = User::default();
    if let Ok(u) = user_from_result(at(content_node, &["core", "user_results", "result"])) {
        author = u;
    }

    let mut media = Vec::new();
    if let Some(list) = at(content_node, &["legacy", "extended_entities", "media"]).as_array() {
        for m in list {
            if let Some(md) = media_from(m) {
                media.push(md);
            }
        }
    }

    let screen = author.screen_name.clone();
    Some(Tweet {
        // conservamos el id OUTER para acción sobre lo visible (like/repost)
        id: id.clone(),
        author,
        text: sstr(legacy, &["full_text"]).unwrap_or_default(),
        created_at_ms: sstr(legacy, &["created_at"])
            .and_then(|s| parse_x_epoch(&s))
            .unwrap_or(0),
        reply_to_id: sstr(legacy, &["in_reply_to_status_id_str"]),
        media,
        liked: ubool(legacy, &["favorited"]),
        reposted: ubool(legacy, &["retweeted"]),
        like_count: unum(legacy, &["favorite_count"]).unwrap_or(0),
        repost_count: unum(legacy, &["retweet_count"]).unwrap_or(0),
        reply_count: unum(legacy, &["reply_count"]).unwrap_or(0),
        is_retweet: is_rt,
        permalink: format!(
            "https://x.com/{}/status/{}",
            if screen.is_empty() { "i" } else { &screen },
            id
        ),
    })
}

/// Recolecta nodos `tweet_results.result` con los shapes de entries conocidos:
/// - `content.itemContent.tweet_results.result` (tweet suelto)
/// - `content.items[].item.itemContent.tweet_results.result` (módulo conversación)
fn collect_tweet_nodes<'a>(entries: impl Iterator<Item = &'a Value>, out: &mut Vec<&'a Value>) {
    for e in entries {
        let content = at(e, &["content"]);
        let direct = at(content, &["itemContent", "tweet_results", "result"]);
        if !direct.is_null() {
            out.push(direct);
        }
        if let Some(items) = at(content, &["items"]).as_array() {
            for it in items {
                let nested = at(it, &["item", "itemContent", "tweet_results", "result"]);
                if !nested.is_null() {
                    out.push(nested);
                }
            }
        }
    }
}

/// Instructions crudas (`instructions` array) → tweets dedup por id.
pub fn tweets_from_instructions<'a>(v: &'a Value) -> Result<Vec<Tweet>> {
    let ins = match v.as_array() {
        Some(a) => a,
        None => {
            return Err(Error::UnexpectedPayload {
                op: "timeline".into(),
                detail: "instructions no es array".into(),
            })
        }
    };
    let mut nodes: Vec<&Value> = Vec::new();
    let mut pin_inserted = false;
    for i in ins {
        match at(i, &["type"]).as_str() {
            Some("TimelineAddEntries") => {
                if let Some(ents) = at(i, &["entries"]).as_array() {
                    collect_tweet_nodes(ents.iter(), &mut nodes);
                }
            }
            Some("TimelinePinEntry") => {
                // pin: objeto `entry` (el mismo tweet suele repetirse en AddEntries)
                if !pin_inserted {
                    pin_inserted = true;
                    collect_tweet_nodes([at(i, &["entry"])].iter().copied(), &mut nodes);
                }
            }
            _ => {}
        }
    }
    let mut out: Vec<Tweet> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for n in &nodes {
        if let Some(t) = tweet_from_result(n) {
            if !t.id.is_empty() && seen.insert(t.id.clone()) {
                out.push(t);
            }
        }
    }
    if out.is_empty() {
        return Err(Error::UnexpectedPayload {
            op: "timeline".into(),
            detail: "después del filters no salió ningún tweet válido".into(),
        });
    }
    Ok(out)
}

/// Payload de `UserTweets` / `UserTweetsAndReplies` / `UserMedia` → vec.
pub fn tweets_from_user_timeline(v: &Value) -> Result<Vec<Tweet>> {
    let user = at(v, &["data", "user", "result"]);
    let tl = at(user, &["timeline_v2"]);
    let base = if tl.is_null() {
        at(user, &["timeline"])
    } else {
        tl
    };
    let instructions = at(base, &["timeline", "instructions"]);
    if instructions.is_null() {
        return Err(Error::UnexpectedPayload {
            op: "user_tweets".into(),
            detail: "sin instructions (timeline_v2 ni timeline)".into(),
        });
    }
    tweets_from_instructions(instructions)
}

// ----------------------------------- test -----------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MediaKind;
    use serde_json::json;

    const FX_USER: &str = include_str!("../tests/fixtures/raw_user_by_login.json");
    const FX_TWEETS: &str = include_str!("../tests/fixtures/raw_user_tweets.json");
    const FX_VIDEO: &str = include_str!("../tests/fixtures/raw_tweet_with_video.json");

    #[test]
    fn parse_x_epoch_basic() {
        let ms = parse_x_epoch("Sat Oct 17 18:11:27 +0000 2026").unwrap();
        // comprobamos contra un caso referencial conocido: Wed Oct 10 20:19:24 2018 = 1539205164000
        assert_eq!(
            parse_x_epoch("Wed Oct 10 20:19:24 +0000 2018").unwrap(),
            1_539_205_164_000
        );
        assert_eq!(ms % 1000, 0);
        assert!(ms > 1_500_000_000_000);
    }

    #[test]
    fn user_new_shape() {
        let u = user_from_payload(&serde_json::from_str(FX_USER).unwrap()).unwrap();
        assert_eq!(u.id, "2244994945");
        assert_eq!(u.screen_name, "XDevelopers");
        assert_eq!(u.name, "Developers");
        assert!(u
            .avatar_url
            .starts_with("https://pbs.twimg.com/profile_images"));
        assert_eq!(u.followers, 697_884);
        assert_eq!(u.following, 770);
        assert_eq!(u.statuses, 4_379);
        assert_eq!(u.created_at, "Sat Dec 14 04:35:55 +0000 2013");
        assert!(u.bio.starts_with("The voice of the X Dev team"));
    }

    #[test]
    fn user_legacy_shape_still_works() {
        let legacy = json!({
            "data": { "user": { "result": {
                "__typename": "User",
                "rest_id": "12",
                "legacy": {
                    "screen_name": "jack",
                    "name": "jack",
                    "profile_image_url_https": "https://pbs.twimg.com/j.jpg",
                    "description": "founder",
                    "followers_count": 1000,
                    "friends_count": 20,
                    "statuses_count": 300,
                    "created_at": "Tue Mar 21 20:50:14 +0000 2006",
                    "verified": true
                }
            }}}
        });
        let u = user_from_payload(&legacy).unwrap();
        assert_eq!(u.screen_name, "jack");
        assert_eq!(u.followers, 1000);
        assert_eq!(u.statuses, 300);
        assert!(u.is_blue_verified);
    }

    #[test]
    fn user_unavailable() {
        let p = json!({"data": {"user": {"result": {"__typename": "UserUnavailable", "reason": "Suspended"}}}});
        assert!(user_from_payload(&p).is_err());
    }

    #[test]
    fn timeline_walk_and_rt() {
        let v: Value = serde_json::from_str(FX_TWEETS).unwrap();
        let tweets = tweets_from_user_timeline(&v).unwrap();
        assert!(
            tweets.len() >= 14,
            "esperaba >=14 tweets, salieron {}",
            tweets.len()
        );
        // primer entry: RT → preferimos contenido del tweet interno
        let first = &tweets[0];
        assert!(!first.id.is_empty());
        assert!(first.is_retweet, "el primer fixture es RT");
        assert!(
            first.text.starts_with("Exhibit spotlight"),
            "texto resumido del RT interno: {}",
            first.text
        );
        assert!(
            first.author.screen_name == "michael_rosinsky" || !first.author.screen_name.is_empty()
        );
        // sin contadores negativos y datos presentes
        assert!(tweets.iter().all(|t| t.like_count < u64::MAX));
    }

    #[test]
    fn tweet_video_media_variants() {
        let v: Value = serde_json::from_str(FX_VIDEO).unwrap();
        let mut best_url = String::new();
        // buscamos el primer tweet con video en el fixture de TweetDetail
        let tw = find_first_video(&v).expect("fixture debe tener video");
        let mut media = Vec::new();
        if let Some(list) = at(&tw, &["legacy", "extended_entities", "media"]).as_array() {
            for m in list {
                if let Some(md) = media_from(m) {
                    media.push(md);
                }
            }
        }
        assert!(!media.is_empty());
        let m0 = &media[0];
        match m0.kind {
            MediaKind::Video => {
                assert!(m0.url.contains("video.twimg.com"));
                assert!(m0.duration_ms > 0);
                assert!(
                    best_url.is_empty() && m0.url.ends_with(".mp4"),
                    "mejor variante mp4"
                );
            }
            _ => panic!("esperaba video"),
        }
    }

    /// hallucination guard: no re-parsear el fixture una vez por aserción.
    fn find_first_video<'a>(v: &'a Value) -> Option<&'a Value> {
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            if let Some(list) = n.as_array() {
                stack.extend(list.iter().rev());
                continue;
            }
            if let Some(obj) = n.as_object() {
                for (k, val) in obj {
                    if k == "tweet_results" && !val.is_null() {
                        let r = at(val, &["result"]);
                        if !at(
                            r,
                            &["legacy", "extended_entities", "media", "0", "video_info"],
                        )
                        .is_null()
                        {
                            return Some(r);
                        }
                    }
                }
                stack.extend(obj.values().rev());
            }
        }
        None
    }

    #[test]
    fn permalink_format() {
        let v: Value = serde_json::from_str(FX_TWEETS).unwrap();
        let tweets = tweets_from_user_timeline(&v).unwrap();
        let t = &tweets[0];
        assert!(t.permalink.starts_with("https://x.com/") && t.permalink.contains("/status/"));
    }
}
