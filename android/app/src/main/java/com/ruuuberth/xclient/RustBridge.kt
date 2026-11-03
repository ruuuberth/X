package com.ruuuberth.xclient

object RustBridge {
    init {
        System.loadLibrary("xjni")
    }

    external fun ping(msg: String): String

    /** Carga el core con el filesDir de la app; devuelve {"ok":…,"session":{…}|null} */
    external fun initCore(dataDir: String): String

    /** Login web/pegado: header `Cookie:` crudo → {"ok":…,"session":{…}} */
    external fun loginWithCookies(raw: String): String

    /** Login usuario+contraseña → {"ok":true,"session":{…}} | {"ok":true,"need":"…"} */
    external fun loginStart(username: String, password: String): String

    /** Reanuda el flow tras el código intermedio ("need" de loginStart) */
    external fun loginSubmitCode(code: String): String

    external fun me(): String

    external fun logout(): String
}
