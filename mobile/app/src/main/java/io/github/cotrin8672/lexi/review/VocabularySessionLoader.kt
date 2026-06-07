package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.storage.VocabularyLoadResult
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository

suspend fun loadSessionVocabulary(
    repository: VocabularyRepository,
    userId: String?,
    canRefreshFromSupabase: Boolean,
): VocabularyLoadResult {
    val resolvedUserId = userId?.takeIf { it.isNotBlank() }
    if (resolvedUserId == null) {
        return if (canRefreshFromSupabase) {
            VocabularyLoadResult.Failure("Signed-in user id is missing. Sign in again to sync vocabulary.")
        } else {
            VocabularyLoadResult.Failure(
                "No account on this device. Sign in to sync vocabulary, or use preview fixtures in tests.",
            )
        }
    }

    val cacheLoad = repository.loadCachedCards(resolvedUserId)
    if (cacheLoad is VocabularyLoadResult.Success) {
        return cacheLoad
    }
    val cacheError = (cacheLoad as VocabularyLoadResult.Failure).message

    if (canRefreshFromSupabase) {
        return when (val refresh = repository.refreshFromSupabase(resolvedUserId)) {
            is VocabularyLoadResult.Success -> refresh
            is VocabularyLoadResult.Failure -> VocabularyLoadResult.Failure(
                buildString {
                    append("No local vocabulary loaded")
                    append(" ($cacheError)")
                    append(". Supabase sync failed: ")
                    append(refresh.message)
                },
            )
        }
    }

    return VocabularyLoadResult.Failure(
        cacheError,
    )
}
