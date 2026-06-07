package io.github.cotrin8672.lexi.review.sync

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.core.content.edit
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.security.SecureRandom
import java.util.Base64
import org.json.JSONObject

data class SupabaseMobileConfig(
    val url: String,
    val anonKey: String,
    val redirectUri: String = "lexireview://auth/callback",
)

data class SupabaseSession(
    val accessToken: String,
    val refreshToken: String?,
    val expiresAtEpochSeconds: Long?,
    val userId: String?,
)

sealed interface SupabaseCallbackResult {
    data class Session(val session: SupabaseSession) : SupabaseCallbackResult
    data class AuthorizationCode(val code: String) : SupabaseCallbackResult
    data object Ignored : SupabaseCallbackResult
}

class SupabaseSessionStore(context: Context) {
    private val preferences = context.applicationContext.getSharedPreferences(
        "lexi_review_supabase_session",
        Context.MODE_PRIVATE,
    )

    fun readUserId(): String? = preferences.getString(KEY_USER_ID, null)?.takeIf { it.isNotBlank() }

    fun read(): SupabaseSession? {
        val accessToken = preferences.getString(KEY_ACCESS_TOKEN, null) ?: return null
        return SupabaseSession(
            accessToken = accessToken,
            refreshToken = preferences.getString(KEY_REFRESH_TOKEN, null),
            expiresAtEpochSeconds = if (preferences.contains(KEY_EXPIRES_AT)) {
                preferences.getLong(KEY_EXPIRES_AT, 0L)
            } else {
                null
            },
            userId = preferences.getString(KEY_USER_ID, null),
        )
    }

    fun write(session: SupabaseSession) {
        preferences.edit {
            putString(KEY_ACCESS_TOKEN, session.accessToken)
            if (session.refreshToken == null) remove(KEY_REFRESH_TOKEN) else putString(KEY_REFRESH_TOKEN, session.refreshToken)
            if (session.expiresAtEpochSeconds == null) remove(KEY_EXPIRES_AT) else putLong(KEY_EXPIRES_AT, session.expiresAtEpochSeconds)
            if (session.userId == null) remove(KEY_USER_ID) else putString(KEY_USER_ID, session.userId)
        }
    }

    fun clear() {
        preferences.edit { clear() }
    }

    private companion object {
        const val KEY_ACCESS_TOKEN = "access_token"
        const val KEY_REFRESH_TOKEN = "refresh_token"
        const val KEY_EXPIRES_AT = "expires_at"
        const val KEY_USER_ID = "user_id"
    }
}

class SupabasePkceAuth(
    private val config: SupabaseMobileConfig,
    private val stateStore: SupabasePkceStateStore,
) {
    fun createGoogleSignInIntent(): Intent {
        val verifier = newCodeVerifier()
        val state = newCodeVerifier()
        stateStore.write(verifier = verifier, state = state)
        val challenge = codeChallenge(verifier)
        val uri = Uri.parse(config.url)
            .buildUpon()
            .appendEncodedPath("auth/v1/authorize")
            .appendQueryParameter("provider", "google")
            .appendQueryParameter("redirect_to", config.redirectUri)
            .appendQueryParameter("code_challenge", challenge)
            .appendQueryParameter("code_challenge_method", "S256")
            .appendQueryParameter("state", state)
            .build()
        return Intent(Intent.ACTION_VIEW, uri)
    }

    fun parseCallback(uri: Uri): SupabaseCallbackResult {
        if (uri.toString().startsWith(config.redirectUri).not()) {
            return SupabaseCallbackResult.Ignored
        }
        val expectedState = stateStore.readState()
        val actualState = uri.getQueryParameter("state")
        if (expectedState != null && actualState != null && expectedState != actualState) {
            return SupabaseCallbackResult.Ignored
        }
        uri.getQueryParameter("code")?.let { code ->
            return SupabaseCallbackResult.AuthorizationCode(code)
        }

        val fragmentParams = uri.fragment
            ?.split("&")
            ?.mapNotNull { part ->
                val split = part.split("=", limit = 2)
                if (split.size == 2) {
                    Uri.decode(split[0]) to Uri.decode(split[1])
                } else {
                    null
                }
            }
            ?.toMap()
            .orEmpty()

        val accessToken = fragmentParams["access_token"] ?: return SupabaseCallbackResult.Ignored
        val expiresIn = fragmentParams["expires_in"]?.toLongOrNull()
        return SupabaseCallbackResult.Session(
            SupabaseSession(
                accessToken = accessToken,
                refreshToken = fragmentParams["refresh_token"],
                expiresAtEpochSeconds = expiresIn?.let { seconds ->
                    System.currentTimeMillis() / 1000L + seconds
                },
                userId = fragmentParams["user_id"],
            ),
        )
    }

    fun exchangeCodeForSession(code: String): SupabaseSession {
        val verifier = stateStore.readVerifier()
            ?: error("Missing Supabase PKCE verifier. Start sign-in again.")
        val base = config.url.trimEnd('/')
        val connection = URL("$base/auth/v1/token?grant_type=pkce")
            .openConnection() as HttpURLConnection
        val body = JSONObject()
            .put("auth_code", code)
            .put("code_verifier", verifier)
            .toString()

        connection.requestMethod = "POST"
        connection.doOutput = true
        connection.setRequestProperty("apikey", config.anonKey)
        connection.setRequestProperty("Content-Type", "application/json")
        connection.setRequestProperty("Accept", "application/json")
        connection.connectTimeout = 15_000
        connection.readTimeout = 20_000
        OutputStreamWriter(connection.outputStream, Charsets.UTF_8).use { writer ->
            writer.write(body)
        }

        val status = connection.responseCode
        val responseBody = if (status in 200..299) {
            connection.inputStream.bufferedReader().use { it.readText() }
        } else {
            connection.errorStream?.bufferedReader()?.use { it.readText() }.orEmpty()
        }
        if (status !in 200..299) {
            error("Supabase sign-in failed with HTTP $status")
        }

        stateStore.clear()
        val json = JSONObject(responseBody)
        val expiresIn = json.optLong("expires_in").takeIf { it > 0L }
        val userId = json.optJSONObject("user")?.optString("id")?.takeIf { it.isNotBlank() }
            ?: json.optString("user_id").takeIf { it.isNotBlank() }
        return SupabaseSession(
            accessToken = json.getString("access_token"),
            refreshToken = json.optString("refresh_token").takeIf { it.isNotBlank() },
            expiresAtEpochSeconds = expiresIn?.let { seconds ->
                System.currentTimeMillis() / 1000L + seconds
            },
            userId = userId,
        )
    }

    private fun newCodeVerifier(): String {
        val bytes = ByteArray(32)
        SecureRandom().nextBytes(bytes)
        return Base64.getUrlEncoder().withoutPadding().encodeToString(bytes)
    }

    private fun codeChallenge(verifier: String): String {
        val digest = MessageDigest.getInstance("SHA-256")
            .digest(verifier.toByteArray(Charsets.US_ASCII))
        return Base64.getUrlEncoder().withoutPadding().encodeToString(digest)
    }
}

class SupabasePkceStateStore(context: Context) {
    private val preferences = context.applicationContext.getSharedPreferences(
        "lexi_review_supabase_pkce",
        Context.MODE_PRIVATE,
    )

    fun write(verifier: String, state: String) {
        preferences.edit {
            putString(KEY_VERIFIER, verifier)
            putString(KEY_STATE, state)
        }
    }

    fun readVerifier(): String? = preferences.getString(KEY_VERIFIER, null)

    fun readState(): String? = preferences.getString(KEY_STATE, null)

    fun clear() {
        preferences.edit { clear() }
    }

    private companion object {
        const val KEY_VERIFIER = "code_verifier"
        const val KEY_STATE = "state"
    }
}
