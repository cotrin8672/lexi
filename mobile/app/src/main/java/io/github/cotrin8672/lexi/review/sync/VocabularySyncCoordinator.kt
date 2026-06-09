package io.github.cotrin8672.lexi.review.sync

import io.github.cotrin8672.lexi.review.storage.VocabularyLoadResult
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

data class VocabularySyncStatus(
    val isSyncing: Boolean = false,
    /** Local Room cache exists (may be stale until a sync succeeds). */
    val hasLocalCache: Boolean = false,
    /** Last Supabase sync attempt completed successfully. */
    val cacheReady: Boolean = false,
    val lastError: String? = null,
)

/**
 * App-scoped vocabulary sync. Deduplicates concurrent refresh requests and keeps
 * sync off the ViewModel lifecycle so session start can stay cache-first.
 */
class VocabularySyncCoordinator(
    private val repository: VocabularyRepository,
    private val scope: CoroutineScope,
    private val canSync: () -> Boolean,
) {
    private val _status = MutableStateFlow(VocabularySyncStatus())
    val status: StateFlow<VocabularySyncStatus> = _status.asStateFlow()

    private val mutex = Mutex()
    private var activeSync: Deferred<Unit>? = null

    fun probeCache(userId: String) {
        scope.launch {
            when (repository.loadCachedCards(userId)) {
                is VocabularyLoadResult.Success ->
                    _status.update { it.copy(hasLocalCache = true) }
                is VocabularyLoadResult.Failure ->
                    _status.update { it.copy(hasLocalCache = false) }
            }
        }
    }

    fun scheduleSync(userId: String) {
        scope.launch { syncNow(userId) }
    }

    suspend fun syncNow(userId: String) {
        if (!canSync()) {
            return
        }
        val job = mutex.withLock {
            activeSync?.takeIf { it.isActive } ?: scope.async {
                runSync(userId)
            }.also { activeSync = it }
        }
        job.await()
    }

    suspend fun awaitIdle() {
        mutex.withLock { activeSync }?.await()
    }

    private suspend fun runSync(userId: String) {
        _status.update { it.copy(isSyncing = true, lastError = null) }
        try {
            when (val result = repository.refreshFromSupabase(userId)) {
                is VocabularyLoadResult.Success ->
                    _status.update {
                        it.copy(
                            isSyncing = false,
                            hasLocalCache = true,
                            cacheReady = true,
                            lastError = null,
                        )
                    }
                is VocabularyLoadResult.Failure ->
                    _status.update {
                        it.copy(
                            isSyncing = false,
                            lastError = result.message,
                        )
                    }
            }
        } catch (error: Exception) {
            _status.update {
                it.copy(
                    isSyncing = false,
                    lastError = error.message ?: "Vocabulary sync failed",
                )
            }
        }
    }
}
