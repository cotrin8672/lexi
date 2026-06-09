package io.github.cotrin8672.lexi.review.sync

fun syncErrorUserMessage(raw: String): String {
    val normalized = raw.lowercase()
    return when {
        normalized.contains("http 401") ||
            normalized.contains("http 403") ||
            normalized.contains("session") && normalized.contains("not available") ->
            "Session expired. Sign in again to sync vocabulary."
        normalized.contains("http 429") ||
            normalized.contains("temporarily unavailable") ->
            "Vocabulary sync is temporarily unavailable. Try again shortly."
        normalized.contains("not configured") ->
            "Supabase sync is not configured on this build."
        else -> raw
    }
}
