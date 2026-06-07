package io.github.cotrin8672.lexi.review

import android.content.Context
import androidx.room.Room
import io.github.cotrin8672.lexi.review.storage.DefaultVocabularyRepository
import io.github.cotrin8672.lexi.review.storage.LexiReviewDatabase
import io.github.cotrin8672.lexi.review.storage.ReviewStore
import io.github.cotrin8672.lexi.review.storage.RoomReviewStore
import io.github.cotrin8672.lexi.review.storage.SupabasePostgrestVocabularyClient
import io.github.cotrin8672.lexi.review.storage.SupabaseVocabularyClient
import io.github.cotrin8672.lexi.review.storage.UnconfiguredSupabaseVocabularyClient
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository
import io.github.cotrin8672.lexi.review.sync.SupabaseMobileConfig
import io.github.cotrin8672.lexi.review.sync.SupabaseSessionStore

class AppDependencies(
    val vocabularyRepository: VocabularyRepository,
    val reviewStore: ReviewStore,
    val sessionStore: SupabaseSessionStore?,
    val supabaseConfigured: Boolean,
) {
    fun activeUserId(): String? = sessionStore?.readUserId()

    fun isSignedIn(): Boolean = !activeUserId().isNullOrBlank()

    fun canRefreshFromSupabase(): Boolean =
        supabaseConfigured && !sessionStore?.read()?.accessToken.isNullOrBlank()

    suspend fun signInWithGoogle() {
        val store = sessionStore ?: error("Supabase is not configured.")
        store.signInWithGoogle()
    }

    companion object {
        fun create(context: Context): AppDependencies {
            val appContext = context.applicationContext
            val database = Room.databaseBuilder(
                appContext,
                LexiReviewDatabase::class.java,
                "lexi_review.db",
            ).build()
            val supabaseConfigured = isSupabaseConfigured()
            val sessionStore = createSessionStore(supabaseConfigured)
            val supabaseClient = createSupabaseClient(sessionStore, supabaseConfigured)
            return AppDependencies(
                vocabularyRepository = DefaultVocabularyRepository(
                    cacheDao = database.vocabularyCacheDao(),
                    supabaseClient = supabaseClient,
                ),
                reviewStore = RoomReviewStore(database.questionStatsDao()),
                sessionStore = sessionStore,
                supabaseConfigured = supabaseConfigured,
            )
        }

        private fun isSupabaseConfigured(): Boolean =
            BuildConfig.SUPABASE_URL.isNotBlank() && BuildConfig.SUPABASE_ANON_KEY.isNotBlank()

        private fun createSupabaseClient(
            sessionStore: SupabaseSessionStore?,
            configured: Boolean,
        ): SupabaseVocabularyClient {
            if (!configured || sessionStore == null) {
                return UnconfiguredSupabaseVocabularyClient()
            }
            return SupabasePostgrestVocabularyClient(
                config = mobileConfig(),
                sessionProvider = { sessionStore.read() },
            )
        }

        private fun createSessionStore(
            configured: Boolean,
        ): SupabaseSessionStore? {
            if (!configured) {
                return null
            }
            return SupabaseSessionStore(
                SupabaseSessionStore.createClient(mobileConfig()),
            )
        }

        private fun mobileConfig(): SupabaseMobileConfig =
            SupabaseMobileConfig(
                url = BuildConfig.SUPABASE_URL,
                publishableKey = BuildConfig.SUPABASE_ANON_KEY,
            )
    }
}
