package io.github.cotrin8672.lexi.review.stats

import io.github.cotrin8672.lexi.review.QuestionType

data class LexemeStatsInput(
    val lexemeId: String,
    val headword: String,
    val createdAt: String,
)

data class TodayStats(
    val studyMinutes: Int,
    val distinctLexemesReviewed: Int,
    val attempts: Int,
    val accuracy: Double?,
    val newWordsAdded: Int,
)

data class StreakStats(
    val currentDays: Int,
    val longestDays: Int,
)

data class DailySeriesPoint(
    val dateLabel: String,
    val studyMinutes: Int,
    val reviewedLexemes: Int,
    val accuracy: Double?,
    val newWordsAdded: Int,
)

data class QuestionTypeStats(
    val questionType: QuestionType,
    val attempts: Int,
    val accuracy: Double?,
)

data class WeakWordEntry(
    val lexemeId: String,
    val headword: String,
    val questionKey: String,
    val questionType: QuestionType,
    val difficultyEma: Double,
    val recentlyWrong: Boolean,
)

data class VocabularyGrowthStats(
    val totalCount: Int,
    val addedThisWeek: Int,
    val addedThisMonth: Int,
)

data class StatsDashboardState(
    val today: TodayStats,
    val streaks: StreakStats,
    val lastSevenDays: List<DailySeriesPoint>,
    val byQuestionType: List<QuestionTypeStats>,
    val weakWords: List<WeakWordEntry>,
    val vocabularyGrowth: VocabularyGrowthStats,
)
