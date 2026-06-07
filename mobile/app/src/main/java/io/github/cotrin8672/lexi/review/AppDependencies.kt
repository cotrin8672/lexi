package io.github.cotrin8672.lexi.review

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.room.Room
import io.github.cotrin8672.lexi.review.storage.DefaultVocabularyRepository
import io.github.cotrin8672.lexi.review.storage.LexiReviewDatabase
import io.github.cotrin8672.lexi.review.storage.ReviewStore
import io.github.cotrin8672.lexi.review.storage.RoomReviewStore
import io.github.cotrin8672.lexi.review.storage.SupabasePostgrestVocabularyClient
import io.github.cotrin8672.lexi.review.storage.SupabaseVocabularyClient
import io.github.cotrin8672.lexi.review.storage.UnconfiguredSupabaseVocabularyClient
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository
import io.github.cotrin8672.lexi.review.sync.SupabaseCallbackResult
import io.github.cotrin8672.lexi.review.sync.SupabaseMobileConfig
import io.github.cotrin8672.lexi.review.sync.SupabasePkceAuth
import io.github.cotrin8672.lexi.review.sync.SupabasePkceStateStore
import io.github.cotrin8672.lexi.review.sync.SupabaseSessionStore

class AppDependencies(
    val vocabularyRepository: VocabularyRepository,
    val reviewStore: ReviewStore,
    private val sessionStore: SupabaseSessionStore,
    val supabaseConfigured: Boolean,
    private val supabaseAuth: SupabasePkceAuth?,
) {
    fun activeUserId(): String? = sessionStore.readUserId()

    fun isSignedIn(): Boolean = !activeUserId().isNullOrBlank()

    fun canRefreshFromSupabase(): Boolean =
        supabaseConfigured && !sessionStore.read()?.accessToken.isNullOrBlank()

    fun createGoogleSignInIntent(): Intent? = supabaseAuth?.createGoogleSignInIntent()

    fun handleAuthCallback(uri: Uri): AuthCallbackStatus {
        val auth = supabaseAuth ?: return AuthCallbackStatus.Ignored
        return when (val result = auth.parseCallback(uri)) {
            is SupabaseCallbackResult.AuthorizationCode -> {
                val session = auth.exchangeCodeForSession(result.code)
                sessionStore.write(session)
                AuthCallbackStatus.SignedIn
            }
            is SupabaseCallbackResult.Session -> {
                sessionStore.write(result.session)
                AuthCallbackStatus.SignedIn
            }
            SupabaseCallbackResult.Ignored -> AuthCallbackStatus.Ignored
        }
    }

    companion object {
        fun create(context: Context): AppDependencies {
            val appContext = context.applicationContext
            val database = Room.databaseBuilder(
                appContext,
                LexiReviewDatabase::class.java,
                "lexi_review.db",
            ).build()
            val sessionStore = SupabaseSessionStore(appContext)
            val supabaseConfigured = isSupabaseConfigured()
            val supabaseAuth = createSupabaseAuth(appContext, supabaseConfigured)
            val supabaseClient = createSupabaseClient(sessionStore, supabaseConfigured)
            return AppDependencies(
                vocabularyRepository = DefaultVocabularyRepository(
                    cacheDao = database.vocabularyCacheDao(),
                    supabaseClient = supabaseClient,
                ),
                reviewStore = RoomReviewStore(database.questionStatsDao()),
                sessionStore = sessionStore,
                supabaseConfigured = supabaseConfigured,
                supabaseAuth = supabaseAuth,
            )
        }

        private fun isSupabaseConfigured(): Boolean =
            BuildConfig.SUPABASE_URL.isNotBlank() && BuildConfig.SUPABASE_ANON_KEY.isNotBlank()

        private fun createSupabaseClient(
            sessionStore: SupabaseSessionStore,
            configured: Boolean,
        ): SupabaseVocabularyClient {
            if (!configured) {
                return UnconfiguredSupabaseVocabularyClient()
            }
            val config = SupabaseMobileConfig(
                url = BuildConfig.SUPABASE_URL,
                anonKey = BuildConfig.SUPABASE_ANON_KEY,
            )
            return SupabasePostgrestVocabularyClient(
                config = config,
                sessionProvider = { sessionStore.read() },
            )
        }

        private fun createSupabaseAuth(
            context: Context,
            configured: Boolean,
        ): SupabasePkceAuth? {
            if (!configured) {
                return null
            }
            val config = SupabaseMobileConfig(
                url = BuildConfig.SUPABASE_URL,
                anonKey = BuildConfig.SUPABASE_ANON_KEY,
            )
            return SupabasePkceAuth(
                config = config,
                stateStore = SupabasePkceStateStore(context),
            )
        }
    }
}

enum class AuthCallbackStatus {
    Ignored,
    SignedIn,
}
