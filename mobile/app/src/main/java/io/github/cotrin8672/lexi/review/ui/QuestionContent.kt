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
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
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

@OptIn(ExperimentalLayoutApi::class)
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
        val underlineRows = remember(slotWidthsPx, maxWidthPx, tokenSpacingPx) {
            layoutReorderAnswerRows(
                slotWidthsPx = slotWidthsPx,
                maxWidthPx = maxWidthPx,
                tokenSpacingPx = tokenSpacingPx,
            )
        }

        Box(modifier = Modifier.fillMaxWidth()) {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                underlineRows.forEach { row ->
                    ReorderAnswerUnderlineRow(
                        row = row,
                        chipHeight = chipHeight,
                    )
                }
            }
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(tokenSpacing),
                verticalArrangement = Arrangement.spacedBy(tokenSpacing),
                modifier = Modifier.fillMaxWidth(),
            ) {
                selectedTokens.forEachIndexed { index, token ->
                    TokenChip(
                        text = token,
                        enabled = answering,
                        onClick = { onRemoveTokenAtIndex(index) },
                    )
                }
            }
        }
    }
}

@Composable
private fun ReorderAnswerUnderlineRow(
    row: ReorderAnswerRowLayout,
    chipHeight: Dp,
) {
    val density = LocalDensity.current
    val underlineWidth = with(density) { row.underlineWidthPx.toDp() }

    Column {
        Spacer(modifier = Modifier.height(chipHeight))
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
