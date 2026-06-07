package io.github.cotrin8672.lexi.review.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
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
    selectedOptionKey: String?,
    interactionPhase: QuestionInteractionPhase,
    onSelectOption: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier) {
        val payload = question.candidate.payload as? QuestionPayload.Inflection
        if (payload != null) {
            InflectionQuestionPrompt(payload = payload)
        } else {
            PrimaryQuestionText(text = question.primaryText)
        }
        Spacer(modifier = Modifier.height(24.dp))
        ChoiceList(
            options = question.options,
            selectedOptionKey = selectedOptionKey,
            interactionPhase = interactionPhase,
            onSelectOption = onSelectOption,
        )
    }
}

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

    val correctTokens = (question.candidate.payload as? QuestionPayload.Reorder)?.tokens.orEmpty()

    Column(modifier = modifier) {
        PrimaryQuestionText(text = question.promptJapanese)
        Spacer(modifier = Modifier.height(28.dp))
        ReorderAnswerArea(
            correctTokens = correctTokens,
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
                if (slot.selected) {
                    TokenPlaceholder(referenceToken = slot.token)
                } else {
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
    val relationLabel = formatInflectionRelation(payload.relation)
    return when (payload.direction) {
        InflectionDirection.BASE_TO_FORM ->
            payload.headword to "$relationLabel form"
        InflectionDirection.FORM_TO_BASE ->
            payload.formText to "base form"
        InflectionDirection.KIND_RECOGNITION ->
            payload.formText to "form: $relationLabel"
    }
}

private fun formatInflectionRelation(relation: String): String =
    relation.replace('_', ' ').trim().ifBlank { relation }

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

private data class ReorderAnswerRowLayout(
    val answerIndices: List<Int>,
    val underlineWidthPx: Float,
)

private fun layoutReorderAnswerRows(
    slotWidthsPx: List<Int>,
    maxWidthPx: Int,
    tokenSpacingPx: Int,
): List<ReorderAnswerRowLayout> {
    if (slotWidthsPx.isEmpty() || maxWidthPx <= 0) {
        return emptyList()
    }

    val rows = mutableListOf<ReorderAnswerRowLayout>()
    var currentIndices = mutableListOf<Int>()
    var currentWidthPx = 0

    fun flushRow() {
        if (currentIndices.isEmpty()) return
        val underlineWidthPx = currentIndices.fold(0) { width, index ->
            width + slotWidthsPx[index].coerceAtMost(maxWidthPx)
        } + tokenSpacingPx * (currentIndices.size - 1).coerceAtLeast(0)
        rows.add(
            ReorderAnswerRowLayout(
                answerIndices = currentIndices.toList(),
                underlineWidthPx = underlineWidthPx.toFloat().coerceAtMost(maxWidthPx.toFloat()),
            ),
        )
        currentIndices = mutableListOf()
        currentWidthPx = 0
    }

    slotWidthsPx.forEachIndexed { index, rawSlotWidthPx ->
        val slotWidthPx = rawSlotWidthPx.coerceAtMost(maxWidthPx)
        val spacingPx = if (currentIndices.isEmpty()) 0 else tokenSpacingPx
        val neededWidthPx = currentWidthPx + spacingPx + slotWidthPx

        if (currentIndices.isNotEmpty() && neededWidthPx > maxWidthPx) {
            flushRow()
            currentIndices.add(index)
            currentWidthPx = slotWidthPx
        } else {
            currentIndices.add(index)
            currentWidthPx = neededWidthPx
        }
    }
    flushRow()

    return rows
}

private fun measureTokenSlotWidthsPx(
    tokens: List<String>,
    textMeasurer: TextMeasurer,
    style: TextStyle,
    chipHorizontalPaddingPx: Int,
): List<Int> =
    tokens.map { token ->
        textMeasurer.measure(token, style).size.width + chipHorizontalPaddingPx
    }

@Composable
private fun ReorderAnswerArea(
    correctTokens: List<String>,
    selectedTokens: List<String>,
    answering: Boolean,
    onRemoveTokenAtIndex: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    val textMeasurer = rememberTextMeasurer()
    val tokenStyle = MaterialTheme.typography.titleMedium
    val density = LocalDensity.current
    val tokenSpacing = 10.dp
    val chipHorizontalPadding = 32.dp
    val chipVerticalPadding = 24.dp

    BoxWithConstraints(modifier = modifier) {
        val maxWidthPx = constraints.maxWidth
        val chipHorizontalPaddingPx = with(density) { chipHorizontalPadding.roundToPx() }
        val tokenSpacingPx = with(density) { tokenSpacing.roundToPx() }
        val slotWidthsPx = remember(correctTokens, tokenStyle, chipHorizontalPaddingPx) {
            measureTokenSlotWidthsPx(
                tokens = correctTokens,
                textMeasurer = textMeasurer,
                style = tokenStyle,
                chipHorizontalPaddingPx = chipHorizontalPaddingPx,
            )
        }
        val chipHeight = remember(tokenStyle, chipVerticalPadding) {
            with(density) {
                (
                    textMeasurer.measure("Mg", tokenStyle).size.height +
                        chipVerticalPadding.roundToPx()
                    ).toDp()
            }
        }
        val rows = remember(slotWidthsPx, maxWidthPx, tokenSpacingPx) {
            layoutReorderAnswerRows(
                slotWidthsPx = slotWidthsPx,
                maxWidthPx = maxWidthPx,
                tokenSpacingPx = tokenSpacingPx,
            )
        }

        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            rows.forEach { row ->
                ReorderMeasuredAnswerRow(
                    row = row,
                    selectedTokens = selectedTokens,
                    slotWidthsPx = slotWidthsPx,
                    maxWidthPx = maxWidthPx,
                    tokenSpacing = tokenSpacing,
                    chipHeight = chipHeight,
                    answering = answering,
                    onRemoveTokenAtIndex = onRemoveTokenAtIndex,
                )
            }
        }
    }
}

@Composable
private fun ReorderMeasuredAnswerRow(
    row: ReorderAnswerRowLayout,
    selectedTokens: List<String>,
    slotWidthsPx: List<Int>,
    maxWidthPx: Int,
    tokenSpacing: Dp,
    chipHeight: Dp,
    answering: Boolean,
    onRemoveTokenAtIndex: (Int) -> Unit,
) {
    val density = LocalDensity.current
    val underlineWidth = with(density) { row.underlineWidthPx.toDp() }

    Column {
        Row(horizontalArrangement = Arrangement.spacedBy(tokenSpacing)) {
            row.answerIndices.forEach { answerIndex ->
                val slotWidth = with(density) {
                    slotWidthsPx[answerIndex].coerceAtMost(maxWidthPx).toDp()
                }
                val selectedToken = selectedTokens.getOrNull(answerIndex)
                Box(
                    modifier = Modifier
                        .width(slotWidth)
                        .height(chipHeight),
                    contentAlignment = Alignment.Center,
                ) {
                    if (selectedToken != null) {
                        TokenChip(
                            text = selectedToken,
                            enabled = answering,
                            onClick = { onRemoveTokenAtIndex(answerIndex) },
                            modifier = Modifier.fillMaxWidth(),
                            fillSlotWidth = true,
                        )
                    }
                }
            }
        }
        Box(
            modifier = Modifier
                .width(underlineWidth)
                .padding(top = 2.dp)
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
    fillSlotWidth: Boolean = false,
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
            modifier = Modifier
                .then(if (fillSlotWidth) Modifier.fillMaxWidth() else Modifier)
                .padding(horizontal = 16.dp, vertical = 12.dp),
            style = MaterialTheme.typography.titleMedium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun TokenPlaceholder(
    referenceToken: String? = null,
) {
    Surface(
        enabled = false,
        onClick = {},
        shape = RoundedCornerShape(10.dp),
        color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.35f),
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.35f)),
    ) {
        Text(
            text = referenceToken ?: "\u00A0",
            modifier = Modifier
                .padding(horizontal = 16.dp, vertical = 12.dp)
                .then(
                    if (referenceToken == null) {
                        Modifier.defaultMinSize(minWidth = 40.dp)
                    } else {
                        Modifier
                    },
                ),
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0f),
        )
    }
}
