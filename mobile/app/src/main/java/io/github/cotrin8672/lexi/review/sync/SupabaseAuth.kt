package io.github.cotrin8672.lexi.review.sync

import io.github.jan.supabase.SupabaseClient
import io.github.jan.supabase.auth.Auth
import io.github.jan.supabase.auth.auth
import io.github.jan.supabase.compose.auth.ComposeAuth
import io.github.jan.supabase.compose.auth.googleNativeLogin
import io.github.jan.supabase.createSupabaseClient
import io.github.jan.supabase.postgrest.Postgrest

data class SupabaseMobileConfig(
    val url: String,
    val publishableKey: String,
    val googleWebClientId: String = "",
)

data class SupabaseSession(
    val accessToken: String,
    val refreshToken: String?,
    val expiresAtEpochSeconds: Long?,
    val userId: String?,
)

class SupabaseSessionStore(
    val client: SupabaseClient,
    val nativeGoogleSignInEnabled: Boolean,
) {
    fun readUserId(): String? =
        client.auth.currentUserOrNull()?.id
            ?: client.auth.currentSessionOrNull()?.user?.id

    fun read(): SupabaseSession? {
        val session = client.auth.currentSessionOrNull() ?: return null
        return SupabaseSession(
            accessToken = session.accessToken,
            refreshToken = session.refreshToken,
            expiresAtEpochSeconds = null,
            userId = session.user?.id ?: client.auth.currentUserOrNull()?.id,
        )
    }

    companion object {
        fun createClient(config: SupabaseMobileConfig): SupabaseSessionStore {
            val nativeGoogleSignInEnabled = config.googleWebClientId.isNotBlank()
            val client = createSupabaseClient(
                supabaseUrl = config.url,
                supabaseKey = config.publishableKey,
            ) {
                install(Auth)
                if (nativeGoogleSignInEnabled) {
                    install(ComposeAuth) {
                        googleNativeLogin(serverClientId = config.googleWebClientId)
                    }
                }
                install(Postgrest)
            }
            return SupabaseSessionStore(
                client = client,
                nativeGoogleSignInEnabled = nativeGoogleSignInEnabled,
            )
        }
    }
}
