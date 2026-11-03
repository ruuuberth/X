# X Client (nombre interno: `xclient`)

App Android para usar **X (ex Twitter)** con núcleo **Rust** y UI **Kotlin/Compose**,
construida y operada desde **Termux** (sin proot-distro) con escalada a **GitHub
Actions / Codespaces** cuando algo es demasiado pesado localmente.

## Decisiones registradas

| Tema | Decisión |
|---|---|
| Acceso a X | API GraphQL interna de x.com con sesión web (no API oficial) |
| Login | Pegar cookies `auth_token` + `ct0` (login por contraseña: fase futura) |
| UI | Jetpack Compose (Kotlin) |
| Núcleo | Rust por JNI (JSON→JSON; UniFFI si se justifica) |
| Multimedia | `MANAGE_EXTERNAL_STORAGE`, escritura directa a `/sdcard/Pictures/X` y `/Movies/X` |
| Video | Player del sistema vía `ACTION_VIEW` + FileProvider (androidx) |
| Build | **N2 CI** para APKs (la red del móvil limita CDN grandes a ~8KB/s) · N1 local solo para Rust/tests · N3 Codespaces interactivo |
| Rama/CI | `develop` = default, `main` = producción; CI test -> artefacto APK debug |
| Repo | Público (gh: `ruuuberth`) |

## Stack fijado (P0)

- Kotlin 2.4.21 · AGP 9.5.0 · Gradle 9.8.2 (runner/CI) · Compose BOM 2026.09.00
- compileSdk 37 · minSdk 26 · targetSdk 36
- Rust stable de Termux (`rust-std-aarch64-linux-android` ya instalado, sin rustup)
- Mirror: `maven.aliyun.com` (Google/Gradle) — `dl.google.com/android/repository` está bloqueado en esta red

## Estructura

```
proyectos/X/
├── crates/
│   ├── xcore/     # lib Rust pura (testeable en host)
│   └── x-jni/     # cdylib → android/app/src/main/jniLibs/arm64-v8a/libxjni.so
├── android/       # app Kotlin + Compose
├── scripts/       # bootstrap, build, keystore
├── .github/workflows/ # ci.yml (develop) + release.yml (main)
└── docs/          # este plan y notas
```

## Fases

| # | Fase | Criterio de aceptación |
|---|---|---|
| P0a | Spike build local Termux (N1) | APK compila + ping Rust dentro de la app |
| P0b | Repo remoto + CI + secrets | push→APK artefacto; `develop` default; `main` protegido |
| P1 | `xcore`: sesión cookies, `me()`, `UserByScreenName`, `UserTweets` | Timeline propia visible |
| P2 | Feed `HomeLatestTimeline` + scroll infinito + Coil | Feed scrolleable, caché offline |
| P3 | Likes/reposts/replies (optimistic UI) | Reflejados en app/web oficial |
| P4 | Composer + upload media chunked + alt text | Tweet con imagen/video publicado |
| P5 | Descargador media (tweet/lote cuenta) | Archivos en /sdcard visibles en galería |
| P6 | Multi-cuenta, endpoints auto-update, settings | 2 cuentas conviven |
| P7 | Backoff 429, i18n, tema, release firmado, extractor queryId en CI | Release estable |

## Reglas de oro

1. **Nunca proot-distro localmente.** escalada = N2/N3 en la nube.
2. Cookies/keystore **jamás** al repo (van a secrets de GitHub o local/ gitignored).
3. Keystore único compartido local/CI → APKs instalables encima sin desinstalar.
4. Escrituras a X solo iniciadas por el usuario (sin automatización de escrituras).

## Riesgos activos

1. Rede del móvil: ~8 KB/s hacia CDNs grandes → APKs y distros SIEMPRE desde CI/Codespace; `scripts/build.sh` local solo construye el `.so` de Rust.
2. queryIds GraphQL rotan → registry `endpoints.ron` + extractor del bundle JS (P6).
3. AGP 9.5 (built-in Kotlin) posible roce de DSL nuevo → fallback a `org.jetbrains.kotlin.android` explícito si falla el build.
