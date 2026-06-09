package io.github.cotrin8672.lexi.review.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import io.github.cotrin8672.lexi.review.schema.Idiom
import io.github.cotrin8672.lexi.review.schema.RelatedWord
import io.github.cotrin8672.lexi.review.schema.Translation
import io.github.cotrin8672.lexi.review.ui.theme.CorrectSoft
import io.github.cotrin8672.lexi.review.ui.theme.IncorrectSoft

@Composable
fun WordDetailScreen(
    context: WordDetailContext,
    wasSkipped: Boolean,
    modifier: Modifier = Modifier,
) {
    val scrollState = rememberScrollState()
    Column(
        modifier = modifier
            .fillMaxWidth()
            .verticalScroll(scrollState),
    ) {
        WordDetailStatusBanner(wasSkipped = wasSkipped)
        Spacer(modifier = Modifier.height(16.dp))
        WordDetailHeader(
            headword = context.content.headword,
            partOfSpeech = context.partOfSpeech,
            irregularForms = context.irregularForms,
        )
        Spacer(modifier = Modifier.height(12.dp))
        ReviewAnswerSummary(
            headline = context.reviewHeadline,
            subline = context.reviewSubline,
        )
        Spacer(modifier = Modifier.height(20.dp))
        if (context.content.nuance.isNotBlank()) {
            WordDetailSection(title = "ニュアンス") {
                Text(
                    text = context.content.nuance,
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
        }
        if (context.content.translations.isNotEmpty()) {
            Spacer(modifier = Modifier.height(20.dp))
            WordDetailSection(title = "訳") {
                context.content.translations.forEach { translation ->
                    val highlighted = translation.text == context.highlightTranslation ||
                        translation.example.sentence == context.highlightExampleSentence
                    TranslationRow(
                        translation = translation,
                        headword = context.content.headword,
                        highlighted = highlighted,
                    )
                    Spacer(modifier = Modifier.height(12.dp))
                }
            }
        }
        if (context.content.synonyms.isNotEmpty()) {
            Spacer(modifier = Modifier.height(20.dp))
            WordDetailSection(title = "類義語") {
                context.content.synonyms.forEach { synonym ->
                    SynonymRow(
                        synonym = synonym,
                        highlighted = synonym.term == context.highlightSynonymTerm,
                    )
                    Spacer(modifier = Modifier.height(12.dp))
                }
            }
        }
        if (context.content.idioms.isNotEmpty()) {
            Spacer(modifier = Modifier.height(20.dp))
            WordDetailSection(title = "慣用句") {
                context.content.idioms.forEach { idiom ->
                    IdiomRow(idiom = idiom)
                    Spacer(modifier = Modifier.height(12.dp))
                }
            }
        }
        if (context.content.warnings.isNotEmpty()) {
            Spacer(modifier = Modifier.height(16.dp))
            Text(
                text = context.content.warnings.first(),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
            )
        }
        Spacer(modifier = Modifier.height(24.dp))
        Text(
            text = "Tap to continue",
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.8f),
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(modifier = Modifier.height(8.dp))
    }
}

@Composable
private fun WordDetailStatusBanner(wasSkipped: Boolean) {
    val backgroundColor = if (wasSkipped) {
        MaterialTheme.colorScheme.surfaceVariant
    } else {
        IncorrectSoft.copy(alpha = 0.12f)
    }
    val textColor = if (wasSkipped) {
        MaterialTheme.colorScheme.onSurfaceVariant
    } else {
        IncorrectSoft
    }
    val label = if (wasSkipped) "Skipped" else "Incorrect"

    Surface(
        modifier = Modifier.fillMaxWidth(),
        color = backgroundColor,
        shape = RoundedCornerShape(10.dp),
    ) {
        Text(
            text = label,
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 10.dp),
            style = MaterialTheme.typography.titleSmall,
            fontWeight = FontWeight.Medium,
            color = textColor,
        )
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun WordDetailHeader(
    headword: String,
    partOfSpeech: String?,
    irregularForms: List<String>,
) {
    Column(modifier = Modifier.fillMaxWidth()) {
        Text(
            text = headword,
            style = MaterialTheme.typography.displaySmall,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onSurface,
        )
        if (!partOfSpeech.isNullOrBlank() || irregularForms.isNotEmpty()) {
            Spacer(modifier = Modifier.height(6.dp))
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                partOfSpeech?.takeIf { it.isNotBlank() }?.let { pos ->
                    PartOfSpeechChip(label = pos)
                }
                irregularForms.forEach { form ->
                    PartOfSpeechChip(label = form)
                }
            }
        }
    }
}

@Composable
private fun PartOfSpeechChip(label: String) {
    Surface(
        shape = RoundedCornerShape(6.dp),
        color = MaterialTheme.colorScheme.surfaceVariant,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.45f)),
    ) {
        Text(
            text = label,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun ReviewAnswerSummary(
    headline: String,
    subline: String?,
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = CorrectSoft.copy(alpha = 0.10f),
        ),
        border = BorderStroke(1.dp, CorrectSoft.copy(alpha = 0.35f)),
        shape = RoundedCornerShape(12.dp),
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text(
                text = headline,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onSurface,
            )
            subline?.takeIf { it.isNotBlank() }?.let { detail ->
                Spacer(modifier = Modifier.height(6.dp))
                Text(
                    text = detail,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun WordDetailSection(
    title: String,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    Column(modifier = modifier.fillMaxWidth()) {
        Text(
            text = title,
            style = MaterialTheme.typography.titleSmall,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.height(10.dp))
        content()
    }
}

@Composable
private fun TranslationRow(
    translation: Translation,
    headword: String,
    highlighted: Boolean,
) {
    val containerColor = if (highlighted) {
        CorrectSoft.copy(alpha = 0.08f)
    } else {
        MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.55f)
    }
    val borderColor = if (highlighted) {
        CorrectSoft.copy(alpha = 0.40f)
    } else {
        MaterialTheme.colorScheme.outline.copy(alpha = 0.30f)
    }

    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        color = containerColor,
        border = BorderStroke(1.dp, borderColor),
    ) {
        Row(
            modifier = Modifier.padding(14.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalAlignment = Alignment.Top,
        ) {
            translation.note?.takeIf { it.isNotBlank() }?.let { note ->
                PartOfSpeechChip(label = note)
            }
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = translation.text,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Medium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    text = highlightHeadwordInSentence(
                        sentence = translation.example.sentence,
                        headword = headword,
                    ),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = translation.example.japanese,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun SynonymRow(
    synonym: RelatedWord,
    highlighted: Boolean,
) {
    val containerColor = if (highlighted) {
        CorrectSoft.copy(alpha = 0.08f)
    } else {
        MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.55f)
    }
    val borderColor = if (highlighted) {
        CorrectSoft.copy(alpha = 0.40f)
    } else {
        MaterialTheme.colorScheme.outline.copy(alpha = 0.30f)
    }

    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        color = containerColor,
        border = BorderStroke(1.dp, borderColor),
    ) {
        Column(modifier = Modifier.padding(14.dp)) {
            Row(
                horizontalArrangement = Arrangement.spacedBy(10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = synonym.term,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Medium,
                )
                Text(
                    text = synonym.japanese,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = synonym.usageComparison,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }
    }
}

@Composable
private fun IdiomRow(idiom: Idiom) {
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.55f),
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.30f)),
    ) {
        Column(modifier = Modifier.padding(14.dp)) {
            Text(
                text = idiom.idiom,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Medium,
            )
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = idiom.japanese,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (idiom.example.isNotBlank()) {
                Spacer(modifier = Modifier.height(6.dp))
                Text(
                    text = idiom.example,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
        }
    }
}

private fun highlightHeadwordInSentence(sentence: String, headword: String) =
    buildAnnotatedString {
        val target = headword.trim()
        if (target.isEmpty()) {
            append(sentence)
            return@buildAnnotatedString
        }
        val pattern = Regex("\\b${Regex.escape(target)}\\b", RegexOption.IGNORE_CASE)
        var lastIndex = 0
        pattern.findAll(sentence).forEach { match ->
            append(sentence.substring(lastIndex, match.range.first))
            withStyle(SpanStyle(fontWeight = FontWeight.SemiBold)) {
                append(match.value)
            }
            lastIndex = match.range.last + 1
        }
        append(sentence.substring(lastIndex))
    }
