package io.github.cotrin8672.lexi.review.sync

import android.content.Intent
import io.github.jan.supabase.SupabaseClient
import io.github.jan.supabase.auth.Auth
import io.github.jan.supabase.auth.ExternalAuthAction
import io.github.jan.supabase.auth.FlowType
import io.github.jan.supabase.auth.auth
import io.github.jan.supabase.auth.handleDeeplinks
import io.github.jan.supabase.auth.providers.Google
import io.github.jan.supabase.createSupabaseClient
import io.github.jan.supabase.postgrest.Postgrest

data class SupabaseMobileConfig(
    val url: String,
    val publishableKey: String,
    val deeplinkScheme: String = "lexireview",
    val deeplinkHost: String = "auth",
)

data class SupabaseSession(
    val accessToken: String,
    val refreshToken: String?,
    val expiresAtEpochSeconds: Long?,
    val userId: String?,
)

class SupabaseSessionStore(
    private val client: SupabaseClient,
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

    suspend fun signInWithGoogle() {
        client.auth.signInWith(Google)
    }

    fun handleDeeplink(intent: Intent) {
        client.handleDeeplinks(intent)
    }

    companion object {
        fun createClient(config: SupabaseMobileConfig): SupabaseClient =
            createSupabaseClient(
                supabaseUrl = config.url,
                supabaseKey = config.publishableKey,
            ) {
                install(Auth) {
                    scheme = config.deeplinkScheme
                    host = config.deeplinkHost
                    flowType = FlowType.PKCE
                    defaultExternalAuthAction = ExternalAuthAction.CustomTabs()
                }
                install(Postgrest)
            }
    }
}
