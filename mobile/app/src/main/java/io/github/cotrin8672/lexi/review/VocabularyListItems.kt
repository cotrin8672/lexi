package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.schema.ActiveVocabularyCard
import io.github.cotrin8672.lexi.review.storage.VocabularySource

data class VocabularyListItem(
    val lexemeId: String,
    val snapshotId: String,
    val headword: String,
    val meanings: String,
    val partOfSpeech: String? = null,
)

fun List<ActiveVocabularyCard>.toVocabularyListItems(): List<VocabularyListItem> =
    groupBy { it.lexemeId }
        .values
        .map { cards -> cards.maxBy { card -> card.snapshot.createdAt } }
        .sortedBy { it.content.headword.lowercase() }
        .map { card ->
            VocabularyListItem(
                lexemeId = card.lexemeId,
                snapshotId = card.snapshot.id,
                headword = card.content.headword,
                meanings = card.content.translations
                    .take(3)
                    .joinToString(separator = " / ") { translation -> translation.text },
                partOfSpeech = card.lexeme.partOfSpeech,
            )
        }

fun VocabularySource.displayLabel(): String = when (this) {
    VocabularySource.FIXTURE -> "Fixture preview"
    VocabularySource.LOCAL_CACHE -> "Local cache"
    VocabularySource.SUPABASE_REFRESH -> "Supabase sync"
}
