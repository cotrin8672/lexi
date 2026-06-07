package io.github.cotrin8672.lexi.review.storage

import io.github.cotrin8672.lexi.review.schema.VocabularyBundle

/**
 * Read-only Supabase boundary for active vocabulary cards.
 * Mutation push is intentionally out of scope for v1.
 */
interface SupabaseVocabularyClient {
    suspend fun fetchActiveVocabulary(userId: String): VocabularyBundle
}

class UnconfiguredSupabaseVocabularyClient : SupabaseVocabularyClient {
    override suspend fun fetchActiveVocabulary(userId: String): VocabularyBundle {
        throw UnsupportedOperationException(
            "Supabase vocabulary refresh is not configured for mobile review v1",
        )
    }
}
