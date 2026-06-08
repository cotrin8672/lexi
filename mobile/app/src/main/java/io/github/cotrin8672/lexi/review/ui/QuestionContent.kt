package io.github.cotrin8672.lexi.review.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import io.github.cotrin8672.lexi.review.InflectionDirection
import io.github.cotrin8672.lexi.review.QuestionPayload
import io.github.cotrin8672.lexi.review.ReorderBankSlot
import io.github.cotrin8672.lexi.review.ui.theme.CorrectSoft
import io.github.cotrin8672.lexi.review.ui.theme.IncorrectSoft

@Composable
fun MeaningQuestionContent(
    question: RenderedQuestion.Meaning,
    selectedOptionKey: String?,
    interactionPhase: QuestionInteractionPhase,
    onSelectOption: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier) {
        PrimaryQuestionText(text = question.headword)
        Spacer(modifier = Modifier.height(24.dp))
        ChoiceList(
            options = question.options,
            selectedOptionKey = selectedOptionKey,
            interactionPhase = interactionPhase,
            onSelectOption = onSelectOption,
        )
    }
}

@Composable
fun UsageQuestionContent(
    question: RenderedQuestion.Usage,
    selectedOptionKey: String?,
    interactionPhase: QuestionInteractionPhase,
    onSelectOption: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier) {
        PrimaryQuestionText(text = question.contextText)
        Spacer(modifier = Modifier.height(24.dp))
        ChoiceList(
            options = question.options,
            selectedOptionKey = selectedOptionKey,
            interactionPhase = interactionPhase,
            onSelectOption = onSelectOption,
        )
    }
}

@Composable
fun InflectionQuestionContent(
    question: RenderedQuestion.Inflection,
    answerText: String,
    interactionPhase: QuestionInteractionPhase,
    onAnswerTextChange: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val payload = question.candidate.payload as? QuestionPayload.Inflection
    val answering = interactionPhase == QuestionInteractionPhase.ANSWERING
    val checked = interactionPhase == QuestionInteractionPhase.CHECKED
    val answerCorrect = checked &&
        payload != null &&
        normalizeInflectionAnswer(answerText) == normalizeInflectionAnswer(payload.formText)
    val answerIncorrect = checked && !answerCorrect

    Column(modifier = modifier.fillMaxWidth()) {
        if (payload != null) {
            InflectionQuestionPrompt(payload = payload)
        } else {
            PrimaryQuestionText(text = question.primaryText)
        }
        Spacer(modifier = Modifier.height(28.dp))
        OutlinedTextField(
            value = answerText,
            onValueChange = onAnswerTextChange,
            enabled = answering,
            modifier = Modifier.fillMaxWidth(),
            label = { Text("不規則変化") },
            placeholder = { Text("英単語を入力") },
            singleLine = true,
            keyboardOptions = KeyboardOptions(
                capitalization = KeyboardCapitalization.None,
                autoCorrectEnabled = false,
            ),
            colors = OutlinedTextFieldDefaults.colors(
                focusedBorderColor = when {
                    answerCorrect -> CorrectSoft
                    answerIncorrect -> IncorrectSoft
                    else -> MaterialTheme.colorScheme.primary
                },
                unfocusedBorderColor = when {
                    answerCorrect -> CorrectSoft.copy(alpha = 0.7f)
                    answerIncorrect -> IncorrectSoft.copy(alpha = 0.7f)
                    else -> MaterialTheme.colorScheme.outline
                },
            ),
        )
        if (answerIncorrect) {
            Spacer(modifier = Modifier.height(12.dp))
            Text(
                text = "正解: ${question.expectedForm}",
                style = MaterialTheme.typography.bodyLarge,
                color = CorrectSoft,
                fontWeight = FontWeight.Medium,
            )
        }
    }
}

private fun normalizeInflectionAnswer(text: String): String =
    text.trim().lowercase()

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ReorderQuestionContent(
    question: RenderedQuestion.Reorder,
    bankSlots: List<ReorderBankSlot>,
    selectedTokens: List<String>,
    interactionPhase: QuestionInteractionPhase,
    onAddTokenAtSlot: (Int) -> Unit,
    onRemoveTokenAtIndex: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    val answering = interactionPhase == QuestionInteractionPhase.ANSWERING

    Column(modifier = modifier) {
        PrimaryQuestionText(text = question.promptJapanese)
        Spacer(modifier = Modifier.height(28.dp))
        ReorderAnswerArea(
            selectedTokens = selectedTokens,
            answering = answering,
            onRemoveTokenAtIndex = onRemoveTokenAtIndex,
            modifier = Modifier
                .fillMaxWidth()
                .defaultMinSize(minHeight = 56.dp),
        )
        Spacer(modifier = Modifier.height(20.dp))
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            bankSlots.forEachIndexed { index, slot ->
                if (!slot.selected) {
                    TokenChip(
                        text = slot.token,
                        enabled = answering,
                        onClick = { onAddTokenAtSlot(index) },
                    )
                }
            }
        }
    }
}

@Composable
fun WrongAnswerLearningCard(
    context: WrongAnswerLearningContext,
    modifier: Modifier = Modifier,
) {
    Card(
        modifier = modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surface.copy(alpha = 0.75f),
        ),
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.35f)),
        shape = RoundedCornerShape(12.dp),
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text(
                text = context.headword,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
            )
            Spacer(modifier = Modifier.height(6.dp))
            Text(
                text = context.correctAnswer,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface,
            )
            context.nuance?.takeIf { it.isNotBlank() }?.let { nuance ->
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    text = nuance,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            context.detail?.takeIf { it.isNotBlank() }?.let { detail ->
                Spacer(modifier = Modifier.height(6.dp))
                Text(
                    text = detail,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun InflectionQuestionPrompt(
    payload: QuestionPayload.Inflection,
    modifier: Modifier = Modifier,
) {
    val (prominentWord, cue) = inflectionDisplayParts(payload)
    Column(modifier = modifier.fillMaxWidth()) {
        PrimaryQuestionText(text = prominentWord)
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = cue,
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

private fun inflectionDisplayParts(payload: QuestionPayload.Inflection): Pair<String, String> {
    val relationLabel = formatInflectionRelationLabel(payload.relation)
    return when (payload.direction) {
        InflectionDirection.BASE_TO_FORM ->
            payload.headword to relationLabel
        InflectionDirection.FORM_TO_BASE ->
            payload.formText to "原形を入力"
        InflectionDirection.KIND_RECOGNITION ->
            payload.formText to relationLabel
    }
}

private fun formatInflectionRelationLabel(relation: String): String =
    when (relation.lowercase()) {
        "past" -> "過去形を入力"
        "pastparticiple", "past_participle" -> "過去分詞を入力"
        "plural" -> "複数形を入力"
        "irregular" -> "不規則変化を入力"
        else -> "${relation.replace('_', ' ').trim()}を入力"
    }

@Composable
private fun PrimaryQuestionText(
    text: String,
    modifier: Modifier = Modifier,
) {
    Text(
        text = text,
        modifier = modifier.fillMaxWidth(),
        style = MaterialTheme.typography.headlineMedium,
        fontWeight = FontWeight.Medium,
        color = MaterialTheme.colorScheme.onSurface,
    )
}

@Composable
private fun ChoiceList(
    options: List<io.github.cotrin8672.lexi.review.MeaningOption>,
    selectedOptionKey: String?,
    interactionPhase: QuestionInteractionPhase,
    onSelectOption: (String) -> Unit,
) {
    options.forEach { option ->
        val selected = selectedOptionKey == option.answerKey
        val revealCorrectness = interactionPhase == QuestionInteractionPhase.CHECKED
        AnswerChoice(
            text = option.label,
            selected = selected,
            revealCorrectness = revealCorrectness,
            isCorrect = option.isCorrect,
            enabled = interactionPhase == QuestionInteractionPhase.ANSWERING,
            onClick = { onSelectOption(option.answerKey) },
        )
        Spacer(modifier = Modifier.height(12.dp))
    }
}

@Composable
private fun AnswerChoice(
    text: String,
    selected: Boolean,
    revealCorrectness: Boolean,
    isCorrect: Boolean,
    enabled: Boolean,
    onClick: () -> Unit,
) {
    val showCorrectMark = revealCorrectness && isCorrect
    val showIncorrectMark = revealCorrectness && selected && !isCorrect
    val containerColor = when {
        showCorrectMark -> CorrectSoft.copy(alpha = 0.12f)
        showIncorrectMark -> IncorrectSoft.copy(alpha = 0.12f)
        selected -> MaterialTheme.colorScheme.primary.copy(alpha = 0.08f)
        else -> MaterialTheme.colorScheme.surfaceVariant
    }
    val borderColor = when {
        showCorrectMark -> CorrectSoft.copy(alpha = 0.55f)
        showIncorrectMark -> IncorrectSoft.copy(alpha = 0.55f)
        selected -> MaterialTheme.colorScheme.primary.copy(alpha = 0.35f)
        else -> MaterialTheme.colorScheme.outline.copy(alpha = 0.7f)
    }
    val textColor = MaterialTheme.colorScheme.onSurface

    Surface(
        onClick = onClick,
        enabled = enabled,
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        color = containerColor,
        border = BorderStroke(1.dp, borderColor),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 18.dp, vertical = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Text(
                text = text,
                modifier = Modifier.weight(1f),
                style = MaterialTheme.typography.bodyLarge,
                color = textColor,
            )
            if (showCorrectMark) {
                Icon(
                    imageVector = Icons.Filled.Check,
                    contentDescription = null,
                    modifier = Modifier.size(20.dp),
                    tint = CorrectSoft,
                )
            } else if (showIncorrectMark) {
                Icon(
                    imageVector = Icons.Filled.Close,
                    contentDescription = null,
                    modifier = Modifier.size(20.dp),
                    tint = IncorrectSoft,
                )
            }
        }
    }
}

private fun reorderTokenMotionSpec() = spring<Float>(
    dampingRatio = Spring.DampingRatioMediumBouncy,
    stiffness = Spring.StiffnessMedium,
)

private fun reorderTokenOffsetMotionSpec() = spring<IntOffset>(
    dampingRatio = Spring.DampingRatioMediumBouncy,
    stiffness = Spring.StiffnessMedium,
)

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ReorderAnswerArea(
    selectedTokens: List<String>,
    answering: Boolean,
    onRemoveTokenAtIndex: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier) {
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            modifier = Modifier
                .fillMaxWidth()
                .defaultMinSize(minHeight = 48.dp)
                .padding(bottom = 8.dp),
        ) {
            selectedTokens.forEachIndexed { index, token ->
                key("answer-$index-$token") {
                    AnimatedVisibility(
                        visible = true,
                        enter = slideInVertically(
                            animationSpec = reorderTokenOffsetMotionSpec(),
                            initialOffsetY = { it / 2 },
                        ) + fadeIn(animationSpec = reorderTokenMotionSpec()) +
                            scaleIn(
                                initialScale = 0.9f,
                                animationSpec = reorderTokenMotionSpec(),
                            ),
                        exit = slideOutVertically(
                            animationSpec = reorderTokenOffsetMotionSpec(),
                            targetOffsetY = { it / 2 },
                        ) + fadeOut(animationSpec = reorderTokenMotionSpec()) +
                            scaleOut(
                                targetScale = 0.9f,
                                animationSpec = reorderTokenMotionSpec(),
                            ),
                    ) {
                        TokenChip(
                            text = token,
                            enabled = answering,
                            onClick = { onRemoveTokenAtIndex(index) },
                        )
                    }
                }
            }
        }
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(2.dp)
                .background(
                    MaterialTheme.colorScheme.onSurface.copy(alpha = 0.22f),
                    RoundedCornerShape(1.dp),
                ),
        )
    }
}

@Composable
private fun TokenChip(
    text: String,
    enabled: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Surface(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier,
        shape = RoundedCornerShape(10.dp),
        color = MaterialTheme.colorScheme.surfaceVariant,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.65f)),
    ) {
        Text(
            text = text,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
            style = MaterialTheme.typography.titleMedium,
        )
    }
}
