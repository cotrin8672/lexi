package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import io.github.cotrin8672.lexi.review.schema.ActiveVocabularyCard
import io.github.cotrin8672.lexi.review.schema.CardSnapshot
import io.github.cotrin8672.lexi.review.schema.LEXI_RESULT_V1_SCHEMA_VERSION
import io.github.cotrin8672.lexi.review.schema.LexiResultV1
import io.github.cotrin8672.lexi.review.schema.Translation
import io.github.cotrin8672.lexi.review.schema.UserLexeme
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
        assertEquals(adoptCard.lexemeId, adopt.lexemeId)
        assertEquals(adoptCard.snapshot.id, adopt.snapshotId)
    }

    @Test
    fun keepsDistinctLexemesThatShareHeadword() {
        val cards = listOf(
            card(
                lexemeId = "lex-gray-us",
                snapshotId = "snap-gray-us",
                canonicalText = "gray",
                headword = "gray",
                meaning = "灰色",
            ),
            card(
                lexemeId = "lex-gray-uk",
                snapshotId = "snap-gray-uk",
                canonicalText = "grey",
                headword = "gray",
                meaning = "灰色の",
            ),
        )

        val items = cards.toVocabularyListItems()

        assertEquals(2, items.size)
        assertEquals(items.size, items.map { it.lexemeId }.distinct().size)
        assertEquals(2, items.count { it.headword == "gray" })
    }

    @Test
    fun dedupesMultipleActiveSnapshotsForSameLexeme() {
        val cards = listOf(
            card(
                lexemeId = "lex-run",
                snapshotId = "snap-run-old",
                canonicalText = "run",
                headword = "run",
                meaning = "走る",
                createdAt = "2026-01-01T00:00:00Z",
            ),
            card(
                lexemeId = "lex-run",
                snapshotId = "snap-run-new",
                canonicalText = "run",
                headword = "run",
                meaning = "走る / 運営する",
                createdAt = "2026-06-01T00:00:00Z",
            ),
        )

        val items = cards.toVocabularyListItems()

        assertEquals(1, items.size)
        assertEquals("snap-run-new", items.single().snapshotId)
        assertTrue(items.single().meanings.contains("運営する"))
    }

    private fun card(
        lexemeId: String,
        snapshotId: String,
        canonicalText: String,
        headword: String,
        meaning: String,
        createdAt: String = ReviewFixtures.FIXTURE_TIMESTAMP,
    ): ActiveVocabularyCard {
        val content = LexiResultV1(
            schemaVersion = LEXI_RESULT_V1_SCHEMA_VERSION,
            mode = "word-study",
            sourceLanguage = "en",
            resultLanguage = "ja",
            headword = headword,
            translations = listOf(
                Translation(
                    text = meaning,
                    note = "動詞",
                    example = io.github.cotrin8672.lexi.review.schema.ExampleSentence(
                        sentence = "Example.",
                        japanese = "例文。",
                    ),
                ),
            ),
            nuance = "Test nuance.",
        )
        return ActiveVocabularyCard(
            lexeme = UserLexeme(
                id = lexemeId,
                userId = ReviewFixtures.USER_ID,
                language = "en",
                canonicalText = canonicalText,
                canonicalKey = canonicalText,
                partOfSpeech = "動詞",
            ),
            snapshot = CardSnapshot(
                id = snapshotId,
                userId = ReviewFixtures.USER_ID,
                lexemeId = lexemeId,
                schemaVersion = LEXI_RESULT_V1_SCHEMA_VERSION,
                provider = "test",
                model = "test",
                resultLanguage = "ja",
                content = content,
                active = true,
                createdAt = createdAt,
            ),
            forms = emptyList(),
        )
    }
}
