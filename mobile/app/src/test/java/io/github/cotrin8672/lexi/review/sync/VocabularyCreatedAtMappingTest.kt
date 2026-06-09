package io.github.cotrin8672.lexi.review.sync

import org.junit.Assert.assertEquals
import org.junit.Test

class VocabularyCreatedAtMappingTest {
    private val fallbackNow = "2026-06-10T12:00:00Z"
    private val rowCreatedAt = "2026-05-01T08:30:00Z"
    private val existingCreatedAt = "2026-04-15T10:00:00Z"
    private val payloadCreatedAt = "2026-05-20T14:00:00Z"

    @Test
    fun bootstrapLexemeRowCarriesCreatedAt() {
        val row = BootstrapLexemeRow(
            id = "lex-1",
            language = "en",
            canonicalText = "adopt",
            canonicalKey = "adopt",
            createdAt = rowCreatedAt,
        )

        assertEquals(rowCreatedAt, row.createdAt)
    }

    @Test
    fun bootstrapRowMapsCreatedAtToEntity() {
        assertEquals(rowCreatedAt, bootstrapLexemeCreatedAt(rowCreatedAt, fallbackNow))
    }

    @Test
    fun bootstrapRowFallsBackWhenCreatedAtMissing() {
        assertEquals(fallbackNow, bootstrapLexemeCreatedAt(null, fallbackNow))
        assertEquals(fallbackNow, bootstrapLexemeCreatedAt("  ", fallbackNow))
    }

    @Test
    fun pullPreservesExistingCreatedAtOnLexemeUpdate() {
        assertEquals(
            existingCreatedAt,
            resolveLexemeCreatedAt(
                existingCreatedAt = existingCreatedAt,
                payloadCreatedAt = payloadCreatedAt,
                now = fallbackNow,
            ),
        )
    }

    @Test
    fun newLexemeFromPullUsesPayloadCreatedAtWhenPresent() {
        assertEquals(
            payloadCreatedAt,
            resolveLexemeCreatedAt(
                existingCreatedAt = null,
                payloadCreatedAt = payloadCreatedAt,
                now = fallbackNow,
            ),
        )
    }

    @Test
    fun newLexemeFromPullFallsBackToNowWhenPayloadMissing() {
        assertEquals(
            fallbackNow,
            resolveLexemeCreatedAt(
                existingCreatedAt = null,
                payloadCreatedAt = null,
                now = fallbackNow,
            ),
        )
        assertEquals(
            fallbackNow,
            resolveLexemeCreatedAt(
                existingCreatedAt = "",
                payloadCreatedAt = " ",
                now = fallbackNow,
            ),
        )
    }

    @Test
    fun bootstrapSnapshotCreatedAtUsesRowOrFallback() {
        assertEquals(rowCreatedAt, bootstrapSnapshotCreatedAt(rowCreatedAt, fallbackNow))
        assertEquals(fallbackNow, bootstrapSnapshotCreatedAt(null, fallbackNow))
    }

    private fun bootstrapSnapshotCreatedAt(rowCreatedAt: String?, updatedAt: String): String =
        rowCreatedAt ?: updatedAt
}
