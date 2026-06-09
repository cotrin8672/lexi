package io.github.cotrin8672.lexi.review.stats

import io.github.cotrin8672.lexi.review.QuestionStats
import io.github.cotrin8672.lexi.review.QuestionType
import io.github.cotrin8672.lexi.review.ReviewResult
import io.github.cotrin8672.lexi.review.storage.ReviewAttemptEvent
import io.github.cotrin8672.lexi.review.storage.StudySession
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import kotlin.math.roundToInt

object StatsAggregator {
    private const val WEAK_WORD_LIMIT = 10
    private val dayLabelFormatter = DateTimeFormatter.ofPattern("MM/dd")

    fun aggregateDashboard(
        attempts: List<ReviewAttemptEvent>,
        sessions: List<StudySession>,
        questionStats: List<QuestionStats>,
        lexemes: List<LexemeStatsInput>,
        now: Instant = Instant.now(),
        zoneId: ZoneId = ZoneId.systemDefault(),
    ): StatsDashboardState {
        val today = localDate(now, zoneId)
        val weekStart = today.minusDays(6)
        val monthStart = today.withDayOfMonth(1)
        val weekInstantStart = weekStart.atStartOfDay(zoneId).toInstant()
        val historyStart = weekStart.minusDays(365).atStartOfDay(zoneId).toInstant()

        val attemptsByDay = attempts.groupBy { localDate(parseInstant(it.answeredAt), zoneId) }
        val sessionsByDay = sessions.groupBy { localDate(parseInstant(it.startedAt), zoneId) }
        val lexemesByDay = lexemes.groupBy { localDate(parseInstant(it.createdAt), zoneId) }

        val studyDays = buildStudyDaySet(
            attempts = attempts.filter { parseInstant(it.answeredAt) >= historyStart },
            sessions = sessions.filter { parseInstant(it.startedAt) >= historyStart },
            zoneId = zoneId,
        )

        return StatsDashboardState(
            today = buildTodayStats(
                today = today,
                attempts = attemptsByDay[today].orEmpty(),
                sessions = sessionsByDay[today].orEmpty(),
                lexemesAdded = lexemesByDay[today].orEmpty(),
            ),
            streaks = buildStreakStats(studyDays, today),
            lastSevenDays = buildDailySeries(
                start = weekStart,
                endInclusive = today,
                attemptsByDay = attemptsByDay,
                sessionsByDay = sessionsByDay,
                lexemesByDay = lexemesByDay,
            ),
            byQuestionType = buildQuestionTypeStats(attempts.filter {
                parseInstant(it.answeredAt) >= weekInstantStart
            }),
            weakWords = buildWeakWords(
                questionStats = questionStats,
                headwordsByLexemeId = lexemes.associate { it.lexemeId to it.headword },
            ),
            vocabularyGrowth = buildVocabularyGrowth(
                lexemes = lexemes,
                today = today,
                weekStart = weekStart,
                monthStart = monthStart,
                zoneId = zoneId,
            ),
        )
    }

    fun accuracy(correct: Int, total: Int): Double? =
        if (total == 0) null else correct.toDouble() / total.toDouble()

    fun millisToMinutes(millis: Long): Int =
        (millis / 60_000.0).roundToInt()

    private fun buildTodayStats(
        today: LocalDate,
        attempts: List<ReviewAttemptEvent>,
        sessions: List<StudySession>,
        lexemesAdded: List<LexemeStatsInput>,
    ): TodayStats {
        val correct = attempts.count { it.correct }
        return TodayStats(
            studyMinutes = millisToMinutes(sessions.sumOf { it.activeMillis }),
            distinctLexemesReviewed = attempts.map { it.lexemeId }.distinct().size,
            attempts = attempts.size,
            accuracy = accuracy(correct, attempts.size),
            newWordsAdded = lexemesAdded.size,
        )
    }

    private fun buildStreakStats(
        studyDays: Set<LocalDate>,
        today: LocalDate,
    ): StreakStats {
        val current = consecutiveDaysEndingAt(studyDays, today)
        val longest = longestStudyStreak(studyDays)
        return StreakStats(currentDays = current, longestDays = longest)
    }

    private fun buildDailySeries(
        start: LocalDate,
        endInclusive: LocalDate,
        attemptsByDay: Map<LocalDate, List<ReviewAttemptEvent>>,
        sessionsByDay: Map<LocalDate, List<StudySession>>,
        lexemesByDay: Map<LocalDate, List<LexemeStatsInput>>,
    ): List<DailySeriesPoint> {
        val dayCount = ChronoUnit.DAYS.between(start, endInclusive).toInt()
        val days = (0..dayCount).map { start.plusDays(it.toLong()) }

        return days.map { day ->
            val dayAttempts = attemptsByDay[day].orEmpty()
            val correct = dayAttempts.count { it.correct }
            DailySeriesPoint(
                dateLabel = day.format(dayLabelFormatter),
                studyMinutes = millisToMinutes(sessionsByDay[day].orEmpty().sumOf { it.activeMillis }),
                reviewedLexemes = dayAttempts.map { it.lexemeId }.distinct().size,
                accuracy = accuracy(correct, dayAttempts.size),
                newWordsAdded = lexemesByDay[day].orEmpty().size,
            )
        }
    }

    private fun buildQuestionTypeStats(
        attempts: List<ReviewAttemptEvent>,
    ): List<QuestionTypeStats> =
        QuestionType.entries.map { questionType ->
            val typeAttempts = attempts.filter { it.questionType == questionType.name }
            val correct = typeAttempts.count { it.correct }
            QuestionTypeStats(
                questionType = questionType,
                attempts = typeAttempts.size,
                accuracy = accuracy(correct, typeAttempts.size),
            )
        }

    private fun buildWeakWords(
        questionStats: List<QuestionStats>,
        headwordsByLexemeId: Map<String, String>,
        limit: Int = WEAK_WORD_LIMIT,
    ): List<WeakWordEntry> =
        questionStats
            .filter { it.attempts > 0 }
            .sortedWith(
                compareByDescending<QuestionStats> { it.difficultyEma }
                    .thenByDescending { if (it.lastResult == ReviewResult.WRONG) 1 else 0 }
                    .thenByDescending { it.wrongStreak },
            )
            .take(limit)
            .map { stats ->
                WeakWordEntry(
                    lexemeId = stats.lexemeId,
                    headword = headwordsByLexemeId[stats.lexemeId] ?: stats.lexemeId,
                    questionKey = stats.questionKey,
                    questionType = stats.questionType,
                    difficultyEma = stats.difficultyEma,
                    recentlyWrong = stats.lastResult == ReviewResult.WRONG,
                )
            }

    private fun buildVocabularyGrowth(
        lexemes: List<LexemeStatsInput>,
        today: LocalDate,
        weekStart: LocalDate,
        monthStart: LocalDate,
        zoneId: ZoneId,
    ): VocabularyGrowthStats {
        val createdDates = lexemes.map { localDate(parseInstant(it.createdAt), zoneId) }
        return VocabularyGrowthStats(
            totalCount = lexemes.size,
            addedThisWeek = createdDates.count { !it.isBefore(weekStart) && !it.isAfter(today) },
            addedThisMonth = createdDates.count { !it.isBefore(monthStart) && !it.isAfter(today) },
        )
    }

    private fun buildStudyDaySet(
        attempts: List<ReviewAttemptEvent>,
        sessions: List<StudySession>,
        zoneId: ZoneId,
    ): Set<LocalDate> {
        val days = mutableSetOf<LocalDate>()
        attempts.forEach { attempt ->
            days += localDate(parseInstant(attempt.answeredAt), zoneId)
        }
        sessions.filter { it.activeMillis > 0L || it.answeredCount > 0 }.forEach { session ->
            days += localDate(parseInstant(session.startedAt), zoneId)
        }
        return days
    }

    private fun consecutiveDaysEndingAt(
        studyDays: Set<LocalDate>,
        end: LocalDate,
    ): Int {
        var streak = 0
        var cursor = end
        while (studyDays.contains(cursor)) {
            streak += 1
            cursor = cursor.minusDays(1)
        }
        return streak
    }

    private fun longestStudyStreak(studyDays: Set<LocalDate>): Int {
        if (studyDays.isEmpty()) {
            return 0
        }
        val sorted = studyDays.sorted()
        var longest = 1
        var current = 1
        for (index in 1 until sorted.size) {
            val gap = ChronoUnit.DAYS.between(sorted[index - 1], sorted[index])
            if (gap == 1L) {
                current += 1
                longest = maxOf(longest, current)
            } else {
                current = 1
            }
        }
        return longest
    }

    private fun parseInstant(value: String): Instant =
        runCatching { Instant.parse(value) }.getOrElse { Instant.EPOCH }

    private fun localDate(instant: Instant, zoneId: ZoneId): LocalDate =
        instant.atZone(zoneId).toLocalDate()
}
