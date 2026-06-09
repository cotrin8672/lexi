package io.github.cotrin8672.lexi.review.sync

internal fun resolveLexemeCreatedAt(
    existingCreatedAt: String?,
    payloadCreatedAt: String?,
    now: String,
): String {
    if (!existingCreatedAt.isNullOrBlank()) {
        return existingCreatedAt
    }
    return payloadCreatedAt?.takeIf { it.isNotBlank() } ?: now
}

internal fun bootstrapLexemeCreatedAt(
    rowCreatedAt: String?,
    fallback: String,
): String = rowCreatedAt?.takeIf { it.isNotBlank() } ?: fallback
