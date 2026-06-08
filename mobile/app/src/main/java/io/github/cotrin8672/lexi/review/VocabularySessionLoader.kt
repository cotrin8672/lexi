package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.storage.VocabularyLoadResult
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository

suspend fun loadSessionVocabulary(
    repository: VocabularyRepository,
    userId: String?,
): VocabularyLoadResult {
    val resolvedUserId = userId?.takeIf { it.isNotBlank() }
        ?: return VocabularyLoadResult.Failure(
            "No account on this device. Sign in to sync vocabulary, or use preview fixtures in tests.",
        )

    return repository.loadCachedCards(resolvedUserId)
}
