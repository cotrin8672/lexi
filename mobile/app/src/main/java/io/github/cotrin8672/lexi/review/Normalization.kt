package io.github.cotrin8672.lexi.review

import java.security.MessageDigest

fun normalizeWhitespace(text: String): String =
    text.trim().replace(Regex("\\s+"), " ")

fun normalizeEnglish(text: String): String =
    normalizeWhitespace(text).lowercase()

fun normalizeJapaneseMeaning(text: String): String =
    normalizeWhitespace(text)

fun normalizeLookupKey(text: String): String =
    normalizeEnglish(text)

fun hashForQuestionKey(normalized: String): String {
    val digest = MessageDigest.getInstance("SHA-256")
    val bytes = digest.digest(normalized.toByteArray(Charsets.UTF_8))
    return bytes.take(8).joinToString("") { "%02x".format(it) }
}

fun tokenizeEnglishSentence(sentence: String): List<String>? {
    val trimmed = normalizeWhitespace(sentence)
    if (trimmed.isEmpty() || hasAwkwardQuotationPatterns(trimmed)) {
        return null
    }
    val tokens = trimmed.split(Regex("\\s+")).filter { it.isNotBlank() }
    if (tokens.size !in 4..12) {
        return null
    }
    return tokens
}

fun hasAwkwardQuotationPatterns(sentence: String): Boolean {
    val quoteCount = sentence.count { it == '"' || it == '\u201c' || it == '\u201d' }
    if (quoteCount % 2 != 0) {
        return true
    }
    val openBracket = sentence.count { it == '[' || it == '(' || it == '{' }
    val closeBracket = sentence.count { it == ']' || it == ')' || it == '}' }
    return openBracket != closeBracket
}

fun isUsageComparisonConcrete(
    comparison: String,
    headword: String,
    otherTerm: String,
): Boolean {
    val normalized = normalizeWhitespace(comparison)
    if (normalized.length < 24) {
        return false
    }
    val lower = normalized.lowercase()
    val head = normalizeEnglish(headword)
    val other = normalizeEnglish(otherTerm)
    val mentionsHead = head.isNotBlank() && lower.contains(head)
    val mentionsOther = other.isNotBlank() && lower.contains(other)
    val hasChoosePattern = Regex("""\bchoose\b|\buse\b|\bpick\b""", RegexOption.IGNORE_CASE)
        .containsMatchIn(normalized)
    return mentionsHead && mentionsOther && hasChoosePattern
}
