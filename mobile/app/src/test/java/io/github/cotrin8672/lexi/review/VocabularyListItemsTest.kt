package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class VocabularyListItemsTest {
    @Test
    fun mapsHeadwordsAndMeaningsInSortedOrder() {
        val cards = ReviewFixtures.vocabularyBundle().activeCards()

        val items = cards.toVocabularyListItems()

        assertEquals(cards.size, items.size)
        assertEquals("adopt", items.first().headword)
        assertTrue(items.zip(items.drop(1)).all { (left, right) ->
            left.headword.lowercase() <= right.headword.lowercase()
        })
        val adoptCard = cards.first { it.content.headword == "adopt" }
        val adopt = items.first { it.headword == "adopt" }
        val expectedMeaning = adoptCard.content.translations.first().text
        assertTrue(adopt.meanings.contains(expectedMeaning))
        assertEquals(adoptCard.lexeme.partOfSpeech, adopt.partOfSpeech)
    }
}
