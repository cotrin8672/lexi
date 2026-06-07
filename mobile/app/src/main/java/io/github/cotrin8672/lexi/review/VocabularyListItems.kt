package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.schema.ActiveVocabularyCard
import io.github.cotrin8672.lexi.review.storage.VocabularySource

data class VocabularyListItem(
    val headword: String,
    val meanings: String,
    val partOfSpeech: String? = null,
)

fun List<ActiveVocabularyCard>.toVocabularyListItems(): List<VocabularyListItem> =
    sortedBy { it.content.headword.lowercase() }
        .map { card ->
            VocabularyListItem(
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
