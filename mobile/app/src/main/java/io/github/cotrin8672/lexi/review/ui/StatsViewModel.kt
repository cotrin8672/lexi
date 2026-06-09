package io.github.cotrin8672.lexi.review.ui

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import io.github.cotrin8672.lexi.review.AppDependencies
import io.github.cotrin8672.lexi.review.schema.ActiveVocabularyCard
import io.github.cotrin8672.lexi.review.stats.LexemeStatsInput
import io.github.cotrin8672.lexi.review.stats.StatsAggregator
import io.github.cotrin8672.lexi.review.stats.StatsDashboardState
import io.github.cotrin8672.lexi.review.storage.ReviewStore
import io.github.cotrin8672.lexi.review.storage.StatsStore
import io.github.cotrin8672.lexi.review.storage.VocabularyLoadResult
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.Instant
import java.time.ZoneId
class StatsViewModel(
    private val statsStore: StatsStore,
    private val vocabularyRepository: VocabularyRepository,
    private val reviewStore: ReviewStore,
    private val sessionUserId: () -> String? = { null },
    private val now: () -> Instant = { Instant.now() },
    private val zoneId: ZoneId = ZoneId.systemDefault(),
) : ViewModel() {
    private val _uiState = MutableStateFlow<StatsUiState>(StatsUiState.Loading)
    val uiState: StateFlow<StatsUiState> = _uiState.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        viewModelScope.launch {
            _uiState.value = StatsUiState.Loading
            val userId = sessionUserId()?.takeIf { it.isNotBlank() }
            if (userId == null) {
                _uiState.value = StatsUiState.Error("Sign in to view study stats.")
                return@launch
            }

            val currentInstant = now()
            val historyStart = currentInstant
                .atZone(zoneId)
                .toLocalDate()
                .minusDays(365)
                .atStartOfDay(zoneId)
                .toInstant()
                .toString()

            val attempts = statsStore.getAttemptsSince(historyStart)
            val sessions = statsStore.getSessionsSince(historyStart)
            val questionStats = reviewStore.getAllStats()
            val lexemes = when (val result = vocabularyRepository.loadCachedCards(userId)) {
                is VocabularyLoadResult.Success -> result.cards.toLexemeStatsInputs()
                is VocabularyLoadResult.Failure -> emptyList()
            }

            _uiState.value = StatsUiState.Ready(
                dashboard = StatsAggregator.aggregateDashboard(
                    attempts = attempts,
                    sessions = sessions,
                    questionStats = questionStats,
                    lexemes = lexemes,
                    now = currentInstant,
                    zoneId = zoneId,
                ),
            )
        }
    }

    companion object {
        fun factory(dependencies: AppDependencies): ViewModelProvider.Factory =
            viewModelFactory {
                initializer {
                    StatsViewModel(
                        statsStore = dependencies.statsStore,
                        vocabularyRepository = dependencies.vocabularyRepository,
                        reviewStore = dependencies.reviewStore,
                        sessionUserId = dependencies::activeUserId,
                    )
                }
            }
    }
}

sealed interface StatsUiState {
    data object Loading : StatsUiState

    data class Ready(
        val dashboard: StatsDashboardState,
    ) : StatsUiState

    data class Error(
        val message: String,
    ) : StatsUiState
}

private fun List<ActiveVocabularyCard>.toLexemeStatsInputs(): List<LexemeStatsInput> =
    groupBy { it.lexemeId }
        .map { (_, cards) ->
            val card = cards.first()
            val createdAt = card.lexeme.createdAt.takeIf { it.isNotBlank() }
                ?: card.snapshot.createdAt
            LexemeStatsInput(
                lexemeId = card.lexemeId,
                headword = card.lexeme.canonicalText,
                createdAt = createdAt,
            )
        }
