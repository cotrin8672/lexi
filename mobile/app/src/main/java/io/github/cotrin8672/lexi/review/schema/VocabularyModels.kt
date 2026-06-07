package io.github.cotrin8672.lexi.review.schema

data class UserLexeme(
    val id: String,
    val userId: String,
    val language: String,
    val canonicalText: String,
    val canonicalKey: String,
    val partOfSpeech: String? = null,
)

data class CardSnapshot(
    val id: String,
    val userId: String,
    val lexemeId: String,
    val schemaVersion: String,
    val provider: String,
    val model: String,
    val resultLanguage: String,
    val content: LexiResultV1,
    val active: Boolean,
    val createdAt: String,
)

data class LexemeForm(
    val id: String,
    val userId: String,
    val lexemeId: String,
    val language: String,
    val formText: String,
    val formKey: String,
    val relation: String,
    val source: String,
)

data class ActiveVocabularyCard(
    val lexeme: UserLexeme,
    val snapshot: CardSnapshot,
    val forms: List<LexemeForm>,
) {
    val content: LexiResultV1 get() = snapshot.content
    val lexemeId: String get() = lexeme.id
}

data class VocabularyBundle(
    val lexemes: List<UserLexeme>,
    val snapshots: List<CardSnapshot>,
    val forms: List<LexemeForm>,
) {
    fun activeCards(): List<ActiveVocabularyCard> {
        val lexemeById = lexemes.associateBy { it.id }
        val formsByLexeme = forms.groupBy { it.lexemeId }
        return snapshots
            .filter { it.active && it.content.mode == "word-study" }
            .mapNotNull { snapshot ->
                val lexeme = lexemeById[snapshot.lexemeId] ?: return@mapNotNull null
                ActiveVocabularyCard(
                    lexeme = lexeme,
                    snapshot = snapshot,
                    forms = formsByLexeme[snapshot.lexemeId].orEmpty(),
                )
            }
    }
}
