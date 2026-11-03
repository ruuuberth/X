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
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
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
    var mode by remember { mutableStateOf(0) } // 0=contraseña (default), 1=navegador, 2=pegar

    Column(Modifier.fillMaxSize()) {
        TabRow(selectedTabIndex = mode) {
            Tab(selected = mode == 0, onClick = { mode = 0 }, text = { Text("Contraseña") })
            Tab(selected = mode == 1, onClick = { mode = 1 }, text = { Text("Navegador") })
            Tab(selected = mode == 2, onClick = { mode = 2 }, text = { Text("Pegar cookies") })
        }
        when (mode) {
            1 -> WebViewLogin(onAuth = onLogged)
            2 -> PasteLogin(onLogged = onLogged)
            else -> PasswordLogin(onLogged = onLogged)
        }
    }
}

/**
 * Login usuario+contraseña vía core Rust (flujo twscrape por subtasks).
 * Los códigos intermedios (email / código del email / 2FA) se piden en un
 * diálogo y reanudan el flow con `loginSubmitCode`. La contraseña jamás se
 * persiste: vive solo dentro de la llamada al core.
 */
@Composable
private fun PasswordLogin(onLogged: (String) -> Unit) {
    var user by remember { mutableStateOf("") }
    var pass by remember { mutableStateOf("") }
    var error by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var pending by remember { mutableStateOf<String?>(null) } // "email"|"email_code"|"two_factor"
    var code by remember { mutableStateOf("") }
    val scope = rememberCoroutineScope()

    fun startLogin() {
        if (busy) return
        busy = true
        error = ""
        scope.launch(Dispatchers.IO) {
            val r = runCatching { RustBridge.loginStart(user, pass) }
                .getOrElse { """{"ok":false,"error":"JNI: $it"}""" }
            val j = runCatching { JSONObject(r) }.getOrElse { JSONObject() }
            withContext(Dispatchers.Main) {
                busy = false
                when {
                    j.optBoolean("ok", false) && j.has("session") -> onLogged(r)
                    j.optBoolean("ok", false) -> {
                        code = ""
                        pending = j.optString("need").ifEmpty { "email_code" }
                    }
                    else -> error = j.optString("error").ifEmpty { "login falló" }
                }
            }
        }
    }

    fun submitCode() {
        if (busy) return
        busy = true
        error = ""
        scope.launch(Dispatchers.IO) {
            val r = runCatching { RustBridge.loginSubmitCode(code) }
                .getOrElse { """{"ok":false,"error":"JNI: $it"}""" }
            val j = runCatching { JSONObject(r) }.getOrElse { JSONObject() }
            withContext(Dispatchers.Main) {
                busy = false
                when {
                    j.optBoolean("ok", false) && j.has("session") -> {
                        pending = null
                        onLogged(r)
                    }
                    else -> {
                        error = j.optString("error").ifEmpty { "código rechazado" }
                        code = ""
                    }
                }
            }
        }
    }

    // código intermedio pedido por el servidor
    pending?.let { kind ->
        val (title, hint) = when (kind) {
            "email" -> "Verificación de cuenta" to "Ingresa el email de tu cuenta"
            "two_factor" -> "Autenticación en dos pasos" to "Código de 6 dígitos de tu app authenticator"
            else -> "Verificación de cuenta" to "Ingresa el código que te enviamos por email"
        }
        AlertDialog(
            onDismissRequest = { if (!busy) pending = null },
            title = { Text(title) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(hint, style = MaterialTheme.typography.bodySmall)
                    if (error.isNotEmpty()) {
                        Text(
                            error,
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                    OutlinedTextField(
                        value = code,
                        onValueChange = { code = it },
                        singleLine = true,
                        enabled = !busy,
                        label = { Text(if (kind == "two_factor") "Código" else hint) },
                    )
                }
            },
            confirmButton = {
                TextButton(
                    enabled = !busy && code.isNotBlank(),
                    onClick = { submitCode() },
                ) { Text(if (busy) "…" else "Continuar") }
            },
            dismissButton = {
                TextButton(enabled = !busy, onClick = { pending = null }) { Text("Cancelar") }
            },
        )
    }

    Column(
        Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text("Inicia sesión en X", style = MaterialTheme.typography.headlineSmall)
        Text(
            "Tu usuario y contraseña se usan solo para entrar; la app guarda únicamente las cookies de la sesión.",
            style = MaterialTheme.typography.bodySmall,
        )
        OutlinedTextField(
            value = user,
            onValueChange = { user = it },
            singleLine = true,
            enabled = !busy,
            label = { Text("Usuario, email o teléfono") },
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = pass,
            onValueChange = { pass = it },
            singleLine = true,
            enabled = !busy,
            label = { Text("Contraseña") },
            modifier = Modifier.fillMaxWidth(),
        )
        if (error.isNotEmpty()) {
            Text(
                error,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall,
            )
        }
        Button(
            onClick = { startLogin() },
            enabled = !busy && user.isNotBlank() && pass.isNotBlank(),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(if (busy) "Verificando…" else "Iniciar sesión")
        }
        Text(
            "Si tu cuenta tiene 2FA o verificación por email, se pedirá el código en el paso siguiente.",
            style = MaterialTheme.typography.bodySmall,
        )
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
