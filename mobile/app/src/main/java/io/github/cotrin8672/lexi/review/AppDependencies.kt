package io.github.cotrin8672.lexi.review

import android.content.Context
import androidx.room.Room
import io.github.cotrin8672.lexi.review.storage.DefaultVocabularyRepository
import io.github.cotrin8672.lexi.review.storage.LexiReviewDatabase
import io.github.cotrin8672.lexi.review.storage.ReviewStore
import io.github.cotrin8672.lexi.review.storage.RoomReviewStore
import io.github.cotrin8672.lexi.review.storage.RoomStatsStore
import io.github.cotrin8672.lexi.review.storage.StatsStore
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository
import io.github.cotrin8672.lexi.review.speech.AndroidWordSpeech
import io.github.cotrin8672.lexi.review.speech.WordSpeech
import io.github.cotrin8672.lexi.review.sync.SupabaseMobileConfig
import io.github.cotrin8672.lexi.review.sync.SupabaseSessionStore
import io.github.cotrin8672.lexi.review.sync.VocabularyReplicaSync
import io.github.cotrin8672.lexi.review.sync.VocabularySyncCoordinator
import io.github.cotrin8672.lexi.review.sync.VocabularySyncEngine
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

class AppDependencies(
    val vocabularyRepository: VocabularyRepository,
    val reviewStore: ReviewStore,
    val statsStore: StatsStore,
    val sessionStore: SupabaseSessionStore?,
    val supabaseConfigured: Boolean,
    val wordSpeech: WordSpeech,
    val vocabularySyncCoordinator: VocabularySyncCoordinator?,
    private val wordSpeechLifecycle: AndroidWordSpeech? = null,
) {
    fun shutdown() {
        wordSpeechLifecycle?.shutdown()
    }
    fun activeUserId(): String? = sessionStore?.readUserId()

    fun isSignedIn(): Boolean = !activeUserId().isNullOrBlank()

    fun canRefreshFromSupabase(): Boolean =
        supabaseConfigured && !sessionStore?.read()?.accessToken.isNullOrBlank()

    fun warmUpVocabularyCache() {
        val userId = activeUserId()?.takeIf { it.isNotBlank() } ?: return
        vocabularySyncCoordinator?.probeCache(userId)
        if (canRefreshFromSupabase()) {
            vocabularySyncCoordinator?.scheduleSync(userId)
        }
    }

    companion object {
        fun create(context: Context): AppDependencies {
            val appContext = context.applicationContext
            val database = Room.databaseBuilder(
                appContext,
                LexiReviewDatabase::class.java,
                "lexi_review.db",
            )
                .addMigrations(
                    LexiReviewDatabase.MIGRATION_2_3,
                    LexiReviewDatabase.MIGRATION_3_4,
                )
                .build()
            val supabaseConfigured = isSupabaseConfigured()
            val sessionStore = createSessionStore(supabaseConfigured)
            val syncEngine = createSyncEngine(
                database = database,
                sessionStore = sessionStore,
                configured = supabaseConfigured,
            )
            val wordSpeech = AndroidWordSpeech(appContext)
            val vocabularyRepository = DefaultVocabularyRepository(
                cacheDao = database.vocabularyCacheDao(),
                syncEngine = syncEngine,
            )
            val appScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
            val vocabularySyncCoordinator = if (syncEngine != null) {
                VocabularySyncCoordinator(
                    repository = vocabularyRepository,
                    scope = appScope,
                    canSync = {
                        supabaseConfigured && !sessionStore?.read()?.accessToken.isNullOrBlank()
                    },
                )
            } else {
                null
            }
            return AppDependencies(
                vocabularyRepository = vocabularyRepository,
                reviewStore = RoomReviewStore(database.questionStatsDao()),
                statsStore = RoomStatsStore(
                    attemptEventDao = database.reviewAttemptEventDao(),
                    studySessionDao = database.studySessionDao(),
                ),
                sessionStore = sessionStore,
                supabaseConfigured = supabaseConfigured,
                wordSpeech = wordSpeech,
                vocabularySyncCoordinator = vocabularySyncCoordinator,
                wordSpeechLifecycle = wordSpeech,
            )
        }

        private fun isSupabaseConfigured(): Boolean =
            BuildConfig.SUPABASE_URL.isNotBlank() && BuildConfig.SUPABASE_ANON_KEY.isNotBlank()

        private fun createSyncEngine(
            database: LexiReviewDatabase,
            sessionStore: SupabaseSessionStore?,
            configured: Boolean,
        ): VocabularyReplicaSync? {
            if (!configured || sessionStore == null) {
                return null
            }
            return VocabularySyncEngine(
                config = mobileConfig(),
                cacheDao = database.vocabularyCacheDao(),
                syncStateDao = database.vocabularySyncStateDao(),
                sessionProvider = { sessionStore.sessionForSync() },
            )
        }

        private fun createSessionStore(
            configured: Boolean,
        ): SupabaseSessionStore? {
            if (!configured) {
                return null
            }
            return SupabaseSessionStore.createClient(mobileConfig())
        }

        private fun mobileConfig(): SupabaseMobileConfig =
            SupabaseMobileConfig(
                url = BuildConfig.SUPABASE_URL,
                publishableKey = BuildConfig.SUPABASE_ANON_KEY,
                googleWebClientId = BuildConfig.GOOGLE_WEB_CLIENT_ID,
            )
    }
}
