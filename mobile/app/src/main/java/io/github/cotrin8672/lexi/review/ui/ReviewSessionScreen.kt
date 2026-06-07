package io.github.cotrin8672.lexi.review.ui

import android.view.HapticFeedbackConstants
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import io.github.cotrin8672.lexi.review.AppDependencies
import io.github.cotrin8672.lexi.review.MeaningOption
import io.github.cotrin8672.lexi.review.QuestionCandidate
import io.github.cotrin8672.lexi.review.QuestionPayload
import io.github.cotrin8672.lexi.review.QuestionType
import io.github.cotrin8672.lexi.review.ReviewMode
import io.github.cotrin8672.lexi.review.VocabularyListItem
import io.github.cotrin8672.lexi.review.displayLabel
import io.github.cotrin8672.lexi.review.storage.VocabularySource
import io.github.cotrin8672.lexi.review.ui.theme.CorrectSoft
import io.github.cotrin8672.lexi.review.ui.theme.FeedbackCorrectPanel
import io.github.cotrin8672.lexi.review.ui.theme.FeedbackIncorrectPanel
import io.github.cotrin8672.lexi.review.ui.theme.IncorrectSoft
import io.github.cotrin8672.lexi.review.ui.theme.LexiReviewTheme
import kotlinx.coroutines.delay

private const val CORRECT_AUTO_ADVANCE_MS = 2500L

@Composable
fun ReviewSessionScreen(
    dependencies: AppDependencies,
    onSignIn: () -> Unit,
    modifier: Modifier = Modifier,
    viewModel: ReviewViewModel = viewModel(
        factory = ReviewViewModel.factory(dependencies),
    ),
) {
    val uiState by viewModel.uiState.collectAsStateWithLifecycle()
    ReviewSessionContent(
        modifier = modifier,
        uiState = uiState,
        onSelectMode = viewModel::startSession,
        onOpenVocabularyList = viewModel::loadVocabularyList,
        onSignIn = onSignIn,
        onSubmitOption = viewModel::submitOption,
        onAddReorderToken = viewModel::addReorderToken,
        onRemoveReorderToken = viewModel::removeReorderToken,
        onCheck = viewModel::checkAnswer,
        onSkip = viewModel::skipQuestion,
        onNext = viewModel::nextQuestion,
        onRetryLoad = viewModel::retryLastLoad,
        onReturnToModeSelect = viewModel::returnToModeSelect,
    )
}

@Composable
private fun ReviewSessionContent(
    uiState: ReviewUiState,
    onSelectMode: (ReviewMode) -> Unit,
    onOpenVocabularyList: () -> Unit,
    onSignIn: () -> Unit,
    onSubmitOption: (String) -> Unit,
    onAddReorderToken: (Int) -> Unit,
    onRemoveReorderToken: (Int) -> Unit,
    onCheck: () -> Unit,
    onSkip: () -> Unit,
    onNext: () -> Unit,
    onRetryLoad: () -> Unit,
    onReturnToModeSelect: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier,
        color = MaterialTheme.colorScheme.surface,
    ) {
        when (uiState.loadPhase) {
            SessionLoadPhase.MODE_SELECT -> ModeSelectState(
                onSelectMode = onSelectMode,
                onOpenVocabularyList = onOpenVocabularyList,
                onSignIn = onSignIn,
                modifier = Modifier.fillMaxSize(),
            )
            SessionLoadPhase.LOADING -> LoadingState(modifier = Modifier.fillMaxSize())
            SessionLoadPhase.ERROR -> ErrorState(
                message = uiState.errorMessage ?: "Could not start review.",
                onRetry = onRetryLoad,
                onBack = onReturnToModeSelect,
                modifier = Modifier.fillMaxSize(),
            )
            SessionLoadPhase.VOCABULARY_LIST -> VocabularyListState(
                uiState = uiState,
                onBack = onReturnToModeSelect,
                modifier = Modifier.fillMaxSize(),
            )
            SessionLoadPhase.READY -> ReadyState(
                uiState = uiState,
                onSubmitOption = onSubmitOption,
                onAddReorderToken = onAddReorderToken,
                onRemoveReorderToken = onRemoveReorderToken,
                onCheck = onCheck,
                onSkip = onSkip,
                onNext = onNext,
                modifier = Modifier.fillMaxSize(),
            )
        }
    }
}

@Composable
private fun ModeSelectState(
    onSelectMode: (ReviewMode) -> Unit,
    onOpenVocabularyList: () -> Unit,
    onSignIn: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier.padding(horizontal = 24.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(
                text = "Lexi",
                style = MaterialTheme.typography.displaySmall,
                fontWeight = FontWeight.SemiBold,
                textAlign = TextAlign.Center,
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = "Choose a review mode",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
            Spacer(modifier = Modifier.height(32.dp))
            ReviewMode.entries.forEach { mode ->
                Button(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 5.dp),
                    onClick = { onSelectMode(mode) },
                ) {
                    Text(mode.label)
                }
            }
            Spacer(modifier = Modifier.height(12.dp))
            OutlinedButton(
                modifier = Modifier.fillMaxWidth(),
                onClick = onOpenVocabularyList,
            ) {
                Text("Word list")
            }
            Spacer(modifier = Modifier.height(8.dp))
            OutlinedButton(
                modifier = Modifier.fillMaxWidth(),
                onClick = onSignIn,
            ) {
                Text("Sign in with Google")
            }
        }
    }
}

@Composable
private fun VocabularyListState(
    uiState: ReviewUiState,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier.padding(horizontal = 20.dp, vertical = 16.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = "Vocabulary",
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.SemiBold,
            )
            OutlinedButton(onClick = onBack) {
                Text("Back")
            }
        }
        Spacer(modifier = Modifier.height(8.dp))
        val sourceLabel = uiState.vocabularySource?.displayLabel()
        Text(
            text = buildString {
                append("${uiState.vocabularyCount} words")
                if (sourceLabel != null) {
                    append(" / ")
                    append(sourceLabel)
                }
            },
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.height(16.dp))
        if (uiState.vocabularyList.isEmpty()) {
            Text(
                text = "No vocabulary loaded.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            LazyColumn(
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                items(uiState.vocabularyList, key = { it.headword }) { item ->
                    VocabularyListRow(item = item)
                }
            }
        }
    }
}

@Composable
private fun VocabularyListRow(
    item: VocabularyListItem,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(10.dp),
        color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.45f),
    ) {
        Column(modifier = Modifier.padding(horizontal = 14.dp, vertical = 12.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = item.headword,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Medium,
                )
                item.partOfSpeech?.let { pos ->
                    Text(
                        text = pos,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            if (item.meanings.isNotBlank()) {
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = item.meanings,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun LoadingState(modifier: Modifier = Modifier) {
    Box(modifier = modifier, contentAlignment = Alignment.Center) {
        CircularProgressIndicator()
    }
}

@Composable
private fun ErrorState(
    message: String,
    onRetry: () -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier.padding(20.dp),
        verticalArrangement = Arrangement.Center,
    ) {
        Text(text = message, style = MaterialTheme.typography.bodyMedium)
        Spacer(modifier = Modifier.height(16.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedButton(onClick = onBack) {
                Text("Back")
            }
            Button(onClick = onRetry) {
                Text("Retry")
            }
        }
    }
}

@Composable
private fun ReadyState(
    uiState: ReviewUiState,
    onSubmitOption: (String) -> Unit,
    onAddReorderToken: (Int) -> Unit,
    onRemoveReorderToken: (Int) -> Unit,
    onCheck: () -> Unit,
    onSkip: () -> Unit,
    onNext: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val question = uiState.currentQuestion
    val questionKey = question?.candidate?.questionKey
    val canTapToContinue = uiState.interactionPhase == QuestionInteractionPhase.CHECKED ||
        uiState.interactionPhase == QuestionInteractionPhase.SKIPPED
    val canReorderTapToCheck = uiState.interactionPhase == QuestionInteractionPhase.ANSWERING &&
        question is RenderedQuestion.Reorder &&
        uiState.canCheckAnswer()
    val isWrong = uiState.interactionPhase == QuestionInteractionPhase.CHECKED &&
        uiState.lastCheckCorrect == false
    val isSkipped = uiState.interactionPhase == QuestionInteractionPhase.SKIPPED
    val tapContinueInteraction = remember { MutableInteractionSource() }
    val reorderTapCheckInteraction = remember { MutableInteractionSource() }
    var advanceConsumed by remember(questionKey) { mutableStateOf(false) }
    var reorderCheckConsumed by remember(questionKey) { mutableStateOf(false) }
    val view = LocalView.current
    val isReorder = question is RenderedQuestion.Reorder

    val advanceOnce = {
        if (!advanceConsumed) {
            advanceConsumed = true
            onNext()
        }
    }

    val checkReorderFromEmptyTap = {
        if (!reorderCheckConsumed) {
            reorderCheckConsumed = true
            onCheck()
        }
    }

    LaunchedEffect(questionKey, uiState.interactionPhase, uiState.lastCheckCorrect) {
        if (
            uiState.interactionPhase == QuestionInteractionPhase.CHECKED &&
            uiState.lastCheckCorrect == true
        ) {
            delay(CORRECT_AUTO_ADVANCE_MS)
            advanceOnce()
        }
    }

    LaunchedEffect(uiState.interactionPhase, uiState.lastCheckCorrect) {
        if (uiState.interactionPhase == QuestionInteractionPhase.CHECKED) {
            view.performHapticFeedback(
                if (uiState.lastCheckCorrect == true) {
                    HapticFeedbackConstants.CONFIRM
                } else {
                    HapticFeedbackConstants.REJECT
                },
            )
        }
    }

    Column(
        modifier = modifier.padding(horizontal = 20.dp, vertical = 16.dp),
    ) {
        VocabularySyncStatus(
            source = uiState.vocabularySource,
            wordCount = uiState.vocabularyCount,
            questionCount = uiState.totalCandidates,
        )
        Spacer(modifier = Modifier.height(8.dp))
        Box(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .then(
                    when {
                        canTapToContinue -> Modifier.clickable(
                            interactionSource = tapContinueInteraction,
                            indication = null,
                            onClick = advanceOnce,
                        )
                        canReorderTapToCheck -> Modifier.clickable(
                            interactionSource = reorderTapCheckInteraction,
                            indication = null,
                            onClick = checkReorderFromEmptyTap,
                        )
                        else -> Modifier
                    },
                ),
            contentAlignment = Alignment.Center,
        ) {
            Column(
                modifier = Modifier.fillMaxWidth(),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                AnimatedVisibility(
                    visible = canTapToContinue && uiState.interactionPhase == QuestionInteractionPhase.CHECKED,
                    enter = fadeIn(),
                    exit = fadeOut(),
                ) {
                    FeedbackResultBanner(
                        isCorrect = uiState.lastCheckCorrect == true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                if (canTapToContinue && (isWrong || isSkipped)) {
                    uiState.wrongAnswerContext?.let { context ->
                        Spacer(modifier = Modifier.height(12.dp))
                        WrongAnswerLearningCard(context = context)
                    }
                }
                if (canTapToContinue) {
                    Spacer(modifier = Modifier.height(16.dp))
                }
                when (question) {
                    is RenderedQuestion.Meaning -> MeaningQuestionContent(
                        question = question,
                        selectedOptionKey = uiState.selectedOptionKey,
                        interactionPhase = uiState.interactionPhase,
                        onSelectOption = onSubmitOption,
                    )
                    is RenderedQuestion.Reorder -> ReorderQuestionContent(
                        question = question,
                        bankSlots = uiState.reorderBankSlots(),
                        selectedTokens = uiState.reorderSelectedTokens,
                        interactionPhase = uiState.interactionPhase,
                        onAddTokenAtSlot = onAddReorderToken,
                        onRemoveTokenAtIndex = onRemoveReorderToken,
                    )
                    is RenderedQuestion.Usage -> UsageQuestionContent(
                        question = question,
                        selectedOptionKey = uiState.selectedOptionKey,
                        interactionPhase = uiState.interactionPhase,
                        onSelectOption = onSubmitOption,
                    )
                    is RenderedQuestion.Inflection -> InflectionQuestionContent(
                        question = question,
                        selectedOptionKey = uiState.selectedOptionKey,
                        interactionPhase = uiState.interactionPhase,
                        onSelectOption = onSubmitOption,
                    )
                    null -> Text("No question loaded.")
                }
                if (canTapToContinue) {
                    Spacer(modifier = Modifier.height(24.dp))
                    Text(
                        text = "Tap to continue",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.8f),
                        modifier = Modifier.fillMaxWidth(),
                        textAlign = TextAlign.Center,
                    )
                }
            }
        }

        if (uiState.interactionPhase == QuestionInteractionPhase.ANSWERING) {
            Spacer(modifier = Modifier.height(16.dp))
            if (isReorder) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    OutlinedButton(
                        modifier = Modifier.weight(1f),
                        onClick = onSkip,
                    ) {
                        Text("Skip")
                    }
                    Button(
                        modifier = Modifier.weight(1f),
                        enabled = uiState.canCheckAnswer(),
                        onClick = onCheck,
                    ) {
                        Text("Check")
                    }
                }
            } else {
                OutlinedButton(
                    modifier = Modifier.fillMaxWidth(),
                    onClick = onSkip,
                ) {
                    Text("Skip")
                }
            }
        }
    }
}

@Composable
private fun VocabularySyncStatus(
    source: VocabularySource?,
    wordCount: Int,
    questionCount: Int,
    modifier: Modifier = Modifier,
) {
    if (source == null && wordCount == 0) {
        return
    }
    val sourceLabel = source?.displayLabel()
    Text(
        text = buildString {
            if (wordCount > 0) {
                append("$wordCount words")
            }
            if (questionCount > 0) {
                if (isNotEmpty()) append(" / ")
                append("$questionCount questions")
            }
            if (sourceLabel != null) {
                if (isNotEmpty()) append(" / ")
                append(sourceLabel)
            }
        },
        modifier = modifier.fillMaxWidth(),
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
    )
}

@Composable
private fun FeedbackResultBanner(
    isCorrect: Boolean,
    modifier: Modifier = Modifier,
) {
    val backgroundColor = if (isCorrect) FeedbackCorrectPanel else FeedbackIncorrectPanel
    val textColor = if (isCorrect) CorrectSoft else IncorrectSoft
    val label = if (isCorrect) "Correct" else "Incorrect"

    Surface(
        modifier = modifier,
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
            textAlign = TextAlign.Center,
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun ReviewSessionPreview() {
    LexiReviewTheme {
        ReviewSessionContent(
            uiState = ReviewUiState(
                loadPhase = SessionLoadPhase.READY,
                sessionQuestionNumber = 1,
                totalCandidates = 12,
                currentQuestion = RenderedQuestion.Meaning(
                    candidate = QuestionCandidate(
                        questionKey = "meaning:v1:lex-adopt:preview",
                        questionType = QuestionType.MEANING,
                        lexemeId = "lex-adopt",
                        sourceHash = "preview",
                        renderSeedHint = "preview",
                        payload = QuestionPayload.Meaning(
                            headword = "adopt",
                            correctMeaning = "\u63A1\u7528\u3059\u308B",
                            partOfSpeechNote = "\u52D5\u8A5E",
                        ),
                    ),
                    headword = "adopt",
                    options = listOf(
                        MeaningOption("a", "\u63A1\u7528\u3059\u308B", true),
                        MeaningOption("b", "\u8AAC\u660E\u3059\u308B", false),
                        MeaningOption("c", "\u62D2\u5426\u3059\u308B", false),
                        MeaningOption("d", "\u6BD4\u8F03\u3059\u308B", false),
                    ),
                ),
            ),
            onSelectMode = {},
            onOpenVocabularyList = {},
            onSignIn = {},
            onSubmitOption = {},
            onAddReorderToken = {},
            onRemoveReorderToken = {},
            onCheck = {},
            onSkip = {},
            onNext = {},
            onRetryLoad = {},
            onReturnToModeSelect = {},
        )
    }
}
