package io.github.cotrin8672.lexi.review.sync

import io.github.cotrin8672.lexi.review.storage.dao.VocabularyCacheDao
import io.github.cotrin8672.lexi.review.storage.dao.VocabularySyncStateDao

/**
 * Mirrors the desktop Tauri vocabulary replica flow:
 * bootstrap canonical tables, then incrementally pull vocabulary_changes.
 */
internal class VocabularySyncEngine(
    private val config: SupabaseMobileConfig,
    private val cacheDao: VocabularyCacheDao,
    private val syncStateDao: VocabularySyncStateDao,
    private val sessionProvider: suspend () -> SupabaseSession?,
    private val restClient: SupabaseRestClient = SupabaseRestClient(config),
) : VocabularyReplicaSync {
    override suspend fun sync(userId: String) {
        val session = sessionProvider()
            ?: error("Supabase session is not available")
        VocabularyBootstrap.runIfNeeded(
            restClient = restClient,
            cacheDao = cacheDao,
            syncStateDao = syncStateDao,
            userId = userId,
            accessToken = session.accessToken,
        )
        VocabularyPull.run(
            restClient = restClient,
            cacheDao = cacheDao,
            syncStateDao = syncStateDao,
            userId = userId,
            accessToken = session.accessToken,
        )
    }
}
