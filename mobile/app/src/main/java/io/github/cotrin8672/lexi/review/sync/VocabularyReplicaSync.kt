package io.github.cotrin8672.lexi.review.sync

fun interface VocabularyReplicaSync {
    suspend fun sync(userId: String)
}
