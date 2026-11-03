package com.ruuuberth.xclient

import android.annotation.SuppressLint
import android.os.Bundle
import android.webkit.CookieManager
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject

// Login móvil directo de x.com (el flow estándar que sirve el navegador
// del teléfono); SIN spoof de UA — el WebView corre exactamente el JS que
// su motor soporta (el spoof del draft rompía el render del login).
private const val LOGIN_URL = "https://x.com/i/flow/login"

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val loaded = runCatching { RustBridge.initCore(filesDir.absolutePath) }
            .getOrElse { """{"ok":false,"error":"JNI init: $it"}""" }

        setContent {
            MaterialTheme {
                Surface(Modifier.fillMaxSize()) {
                    var rawState by remember { mutableStateOf(loaded) }
                    val scope = androidx.compose.runtime.rememberCoroutineScope()

                    val session = parseSession(rawState)
                    if (session == null) {
                        LoginScreen(
                            onLogged = { json -> rawState = json },
                        )
                    } else {
                        HomeScreen(
                            session = session,
                            onLogout = {
                                scope.launch(Dispatchers.IO) {
                                    runCatching { RustBridge.logout() }
                                }
                                // cambia a login en la siguiente recomposición
                                rawState = """{"ok":false}"""
                            },
                        )
                    }
                }
            }
        }
    }
}

/** NULL sin sesión; JSONObject de `{"ok":true,"session":…}` con sesión. */
private fun parseSession(raw: String): JSONObject? = runCatching {
    val r = JSONObject(raw)
    if (!r.optBoolean("ok", false)) return null
    r.optJSONObject("session") ?: return null
}.getOrNull()

// ---------------- Login ----------------

@Composable
private fun LoginScreen(onLogged: (String) -> Unit) {
    var useWebView by remember { mutableStateOf(true) }

    Column(Modifier.fillMaxSize()) {
        if (useWebView) {
            WebViewLogin(onAuth = onLogged)
            TextButton(
                onClick = { useWebView = false },
                modifier = Modifier.fillMaxWidth().padding(8.dp),
            ) {
                Text("Problemas con el navegador? Pegar cookies manual")
            }
        } else {
            PasteLogin(onLogged = onLogged)
            TextButton(
                onClick = { useWebView = true },
                modifier = Modifier.fillMaxWidth().padding(8.dp),
            ) {
                Text("Volver al navegador")
            }
        }
    }
}

/**
 * Login embedded: x.com web (con Google/contraseña/2FA tal cual) y
 * cosecha automática de auth_token+ct0 al terminar la sesión.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
private fun WebViewLogin(onAuth: (String) -> Unit) {
    var status by remember { mutableStateOf("") }
    var inFlight by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    Column(Modifier.fillMaxSize()) {
        if (status.isNotEmpty()) {
            Text(
                status,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.padding(8.dp),
            )
        }
        AndroidView(
            factory = { ctx ->
                WebView(ctx).apply {
                    val cm = CookieManager.getInstance()
                    settings.javaScriptEnabled = true
                    settings.domStorageEnabled = true
                    settings.databaseEnabled = true
                    // viewport móvil correcto (el draft: box media-fuera-de-pantalla)
                    settings.useWideViewPort = true
                    settings.loadWithOverviewMode = true
                    // window.open de la web → navegar in-place
                    settings.setSupportMultipleWindows(false)
                    settings.javaScriptCanOpenWindowsAutomatically = true
                    settings.mixedContentMode = WebSettings.MIXED_CONTENT_COMPATIBILITY_MODE
                    cm.setAcceptCookie(true)
                    cm.setAcceptThirdPartyCookies(this, true)

                    fun tryHarvest() {
                        if (inFlight) return
                        val jar = cm.getCookie("https://x.com") ?: return
                        val token = Regex("auth_token=([^;\\s]+)").find(jar)?.groupValues?.get(1) ?: return
                        val csrf = Regex("ct0=([^;\\s]+)").find(jar)?.groupValues?.get(1) ?: ""
                        if (token.isEmpty()) return
                        inFlight = true
                        scope.launch(Dispatchers.IO) {
                            val r = runCatching { RustBridge.loginWithCookies("auth_token=$token; ct0=$csrf") }
                                .getOrElse { """{"ok":false,"error":"JNI: $it"}""" }
                            val j = runCatching { JSONObject(r) }.getOrElse { JSONObject() }
                            withContext(Dispatchers.Main) {
                                if (j.optBoolean("ok", false)) {
                                    status = ""
                                    onAuth(r)
                                } else {
                                    inFlight = false
                                    status = "Sesión rechazada: ${j.optString("error")}"
                                }
                            }
                        }
                    }

                    webViewClient = object : WebViewClient() {
                        override fun doUpdateVisitedHistory(view: WebView, url: String?, isReload: Boolean) {
                            tryHarvest()
                        }

                        override fun onPageFinished(view: WebView, url: String?) {
                            tryHarvest()
                        }

                        override fun onReceivedError(
                            view: WebView,
                            request: WebResourceRequest,
                            error: WebResourceError,
                        ) {
                            if (request.isForMainFrame) {
                                status = "error página: ${error.description} (${request.url})"
                            }
                        }
                    }
                    webChromeClient = WebChromeClient()
                    loadUrl(LOGIN_URL)
                }
            },
            update = {},
            modifier = Modifier.fillMaxWidth().weight(1f),
        )
    }
}

/** Respaldo para usuarios técnicos: pega el header `Cookie:` en texto. */
@Composable
private fun PasteLogin(onLogged: (String) -> Unit) {
    var text by remember { mutableStateOf("") }
    var status by remember { mutableStateOf("") }
    val scope = rememberCoroutineScope()

    Column(Modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("Entrar pegando cookies", style = MaterialTheme.typography.titleMedium)
        OutlinedTextField(
            value = text,
            onValueChange = { text = it },
            label = { Text("Cookie: auth_token=…; ct0=…") },
            minLines = 3,
            modifier = Modifier.fillMaxWidth(),
        )
        Button(
            onClick = {
                scope.launch(Dispatchers.IO) {
                    val r = runCatching { RustBridge.loginWithCookies(text) }
                        .getOrElse { """{"ok":false,"error":"JNI: $it"}""" }
                    val j = runCatching { JSONObject(r) }.getOrElse { JSONObject() }
                    withContext(Dispatchers.Main) {
                        if (j.optBoolean("ok", false)) {
                            onLogged(r)
                        } else {
                            status = j.optString("error", "error desconocido")
                        }
                    }
                }
            },
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text("Entrar")
        }
        if (status.isNotEmpty()) {
            Text(status, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
        }
    }
}

// ---------------- Home (placeholder P1) ----------------

@Composable
private fun HomeScreen(session: JSONObject, onLogout: () -> Unit) {
    Column(
        Modifier.fillMaxSize().padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text("X Client", style = MaterialTheme.typography.headlineSmall)
        val handle = session.optString("handle").ifEmpty { "—" }
        Text("Sesión: @$handle", style = MaterialTheme.typography.titleMedium)
        Text(RustBridge.ping("sesión viva"), style = MaterialTheme.typography.bodySmall)
        Text("Feed, likes y publicaciones: siguiente fase", style = MaterialTheme.typography.bodySmall)
        Button(onClick = onLogout, modifier = Modifier.fillMaxWidth()) {
            Text("Cerrar sesión")
        }
    }
}
