package io.github.cotrin8672.lexi.review

object QuestionKey {
    private const val VERSION = "v1"

    fun meaning(lexemeId: String, japaneseMeaning: String): String {
        val normalized = normalizeJapaneseMeaning(japaneseMeaning)
        return "meaning:$VERSION:$lexemeId:${hashForQuestionKey(normalized)}"
    }

    fun reorder(lexemeId: String, englishSentence: String): String {
        val normalized = normalizeEnglish(englishSentence)
        return "reorder:$VERSION:$lexemeId:${hashForQuestionKey(normalized)}"
    }

    fun usage(
        lexemeId: String,
        headword: String,
        otherTerm: String,
        comparison: String,
    ): String {
        val normalized = listOf(headword, otherTerm, comparison)
            .joinToString("|") { normalizeEnglish(it) }
        return "usage:$VERSION:$lexemeId:${hashForQuestionKey(normalized)}"
    }

    fun inflection(lexemeId: String, relation: String, formKey: String): String {
        val normalizedRelation = normalizeEnglish(relation)
        val normalizedFormKey = normalizeLookupKey(formKey)
        return "inflection:$VERSION:$lexemeId:$normalizedRelation:$normalizedFormKey"
    }
}
