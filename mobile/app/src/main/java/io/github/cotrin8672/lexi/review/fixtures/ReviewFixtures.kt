package io.github.cotrin8672.lexi.review.fixtures

import io.github.cotrin8672.lexi.review.schema.CardSnapshot
import io.github.cotrin8672.lexi.review.schema.ExampleSentence
import io.github.cotrin8672.lexi.review.schema.Inflection
import io.github.cotrin8672.lexi.review.schema.LEXI_RESULT_V1_SCHEMA_VERSION
import io.github.cotrin8672.lexi.review.schema.LexemeForm
import io.github.cotrin8672.lexi.review.schema.LexiResultV1
import io.github.cotrin8672.lexi.review.schema.RelatedWord
import io.github.cotrin8672.lexi.review.schema.Translation
import io.github.cotrin8672.lexi.review.schema.UserLexeme
import io.github.cotrin8672.lexi.review.schema.VocabularyBundle

object ReviewFixtures {
    const val USER_ID = "fixture-user"
    const val FIXTURE_TIMESTAMP = "2026-06-06T00:00:00Z"

    fun vocabularyBundle(): VocabularyBundle = VocabularyBundle(
        lexemes = listOf(
            lexeme("lex-adopt", "adopt", "動詞"),
            lexeme("lex-explain", "explain", "動詞"),
            lexeme("lex-reject", "reject", "動詞"),
            lexeme("lex-compare", "compare", "動詞"),
            lexeme("lex-go", "go", "動詞"),
            lexeme("lex-subtle", "subtle", "形容詞"),
        ),
        snapshots = listOf(
            snapshot("snap-adopt", "lex-adopt", adoptContent()),
            snapshot("snap-explain", "lex-explain", explainContent()),
            snapshot("snap-reject", "lex-reject", rejectContent()),
            snapshot("snap-compare", "lex-compare", compareContent()),
            snapshot("snap-go", "lex-go", goContent()),
            snapshot("snap-subtle", "lex-subtle", subtleContent()),
        ),
        forms = listOf(
            form("form-go-canonical", "lex-go", "go", "go", "canonical"),
            form("form-go-went", "lex-go", "went", "went", "irregular"),
        ),
    )

    private fun lexeme(id: String, canonical: String, pos: String): UserLexeme =
        UserLexeme(
            id = id,
            userId = USER_ID,
            language = "en",
            canonicalText = canonical,
            canonicalKey = canonical,
            partOfSpeech = pos,
        )

    private fun snapshot(id: String, lexemeId: String, content: LexiResultV1): CardSnapshot =
        CardSnapshot(
            id = id,
            userId = USER_ID,
            lexemeId = lexemeId,
            schemaVersion = LEXI_RESULT_V1_SCHEMA_VERSION,
            provider = "fixture",
            model = "fixture",
            resultLanguage = "ja",
            content = content,
            active = true,
            createdAt = FIXTURE_TIMESTAMP,
        )

    private fun form(
        id: String,
        lexemeId: String,
        formText: String,
        formKey: String,
        relation: String,
    ): LexemeForm = LexemeForm(
        id = id,
        userId = USER_ID,
        lexemeId = lexemeId,
        language = "en",
        formText = formText,
        formKey = formKey,
        relation = relation,
        source = "fixture",
    )

    private fun adoptContent(): LexiResultV1 = LexiResultV1(
        schemaVersion = LEXI_RESULT_V1_SCHEMA_VERSION,
        mode = "word-study",
        sourceLanguage = "en",
        resultLanguage = "ja",
        headword = "adopt",
        translations = listOf(
            Translation(
                text = "採用する",
                note = "動詞",
                example = ExampleSentence(
                    sentence = "The team adopted a new policy.",
                    japanese = "チームは新しい方針を採用した。",
                ),
            ),
        ),
        nuance = "Officially start using a plan or method.",
        synonyms = emptyList(),
        inflections = emptyList(),
        idioms = emptyList(),
        warnings = emptyList(),
    )

    private fun explainContent(): LexiResultV1 = LexiResultV1(
        schemaVersion = LEXI_RESULT_V1_SCHEMA_VERSION,
        mode = "word-study",
        sourceLanguage = "en",
        resultLanguage = "ja",
        headword = "explain",
        translations = listOf(
            Translation(
                text = "説明する",
                note = "動詞",
                example = ExampleSentence(
                    sentence = "She explained the rules clearly.",
                    japanese = "彼女は規則をはっきりと説明した。",
                ),
            ),
        ),
        nuance = "Make something clear in words.",
        synonyms = emptyList(),
        inflections = emptyList(),
        idioms = emptyList(),
        warnings = emptyList(),
    )

    private fun rejectContent(): LexiResultV1 = LexiResultV1(
        schemaVersion = LEXI_RESULT_V1_SCHEMA_VERSION,
        mode = "word-study",
        sourceLanguage = "en",
        resultLanguage = "ja",
        headword = "reject",
        translations = listOf(
            Translation(
                text = "拒否する",
                note = "動詞",
                example = ExampleSentence(
                    sentence = "The board rejected the proposal.",
                    japanese = "理事会は提案を拒否した。",
                ),
            ),
        ),
        nuance = "Refuse to accept something.",
        synonyms = emptyList(),
        inflections = emptyList(),
        idioms = emptyList(),
        warnings = emptyList(),
    )

    private fun compareContent(): LexiResultV1 = LexiResultV1(
        schemaVersion = LEXI_RESULT_V1_SCHEMA_VERSION,
        mode = "word-study",
        sourceLanguage = "en",
        resultLanguage = "ja",
        headword = "compare",
        translations = listOf(
            Translation(
                text = "比較する",
                note = "動詞",
                example = ExampleSentence(
                    sentence = "We compared the two options carefully.",
                    japanese = "私たちは二つの選択肢を慎重に比較した。",
                ),
            ),
        ),
        nuance = "Examine similarities and differences.",
        synonyms = emptyList(),
        inflections = emptyList(),
        idioms = emptyList(),
        warnings = emptyList(),
    )

    private fun goContent(): LexiResultV1 = LexiResultV1(
        schemaVersion = LEXI_RESULT_V1_SCHEMA_VERSION,
        mode = "word-study",
        sourceLanguage = "en",
        resultLanguage = "ja",
        headword = "go",
        inflections = listOf(Inflection(kind = "past", form = "went")),
        translations = listOf(
            Translation(
                text = "行く",
                note = "動詞",
                example = ExampleSentence(
                    sentence = "I go to school every day.",
                    japanese = "私は毎日学校へ行く。",
                ),
            ),
        ),
        nuance = "Move from one place to another.",
        synonyms = emptyList(),
        idioms = emptyList(),
        warnings = emptyList(),
    )

    private fun subtleContent(): LexiResultV1 = LexiResultV1(
        schemaVersion = LEXI_RESULT_V1_SCHEMA_VERSION,
        mode = "word-study",
        sourceLanguage = "en",
        resultLanguage = "ja",
        headword = "subtle",
        translations = listOf(
            Translation(
                text = "微妙な",
                note = "形容詞",
                example = ExampleSentence(
                    sentence = "She noticed a subtle change in his voice.",
                    japanese = "彼女は彼の声の微妙な変化に気づいた。",
                ),
            ),
        ),
        nuance = "Hard to notice but meaningful.",
        synonyms = listOf(
            RelatedWord(
                term = "delicate",
                japanese = "繊細な",
                usageComparison =
                    "Choose subtle for hard-to-notice differences; choose delicate for fine detail.",
            ),
            RelatedWord(
                term = "slight",
                japanese = "わずかな",
                usageComparison =
                    "Choose subtle for understated meaning; choose slight for a small degree.",
            ),
        ),
        inflections = emptyList(),
        idioms = emptyList(),
        warnings = emptyList(),
    )
}
