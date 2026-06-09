package io.github.cotrin8672.lexi.review.stats

import io.github.cotrin8672.lexi.review.QuestionStats
import io.github.cotrin8672.lexi.review.QuestionType
import io.github.cotrin8672.lexi.review.ReviewResult
import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import io.github.cotrin8672.lexi.review.storage.ReviewAttemptEvent
import io.github.cotrin8672.lexi.review.storage.StudySession
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId

class StatsAggregatorTest {
    private val zoneId = ZoneId.of("UTC")
    private val today = LocalDate.of(2026, 6, 10)
    private val now = today.atTime(18, 0).atZone(zoneId).toInstant()

    @Test
    fun todayStatsCombineSessionsAttemptsAndNewWords() {
        val attempts = listOf(
            attempt(
                lexemeId = "lex-a",
                questionType = QuestionType.MEANING,
                correct = true,
                answeredAt = instantAt(today, 10),
            ),
            attempt(
                lexemeId = "lex-b",
                questionType = QuestionType.USAGE,
                correct = false,
                answeredAt = instantAt(today, 11),
            ),
        )
        val sessions = listOf(
            session(
                startedAt = instantAt(today, 9),
                activeMillis = 125_000L,
            ),
        )
        val lexemes = listOf(
            lexeme("lex-a", "alpha", instantAt(today, 8)),
            lexeme("lex-old", "old", instantAt(today.minusDays(3), 8)),
        )

        val dashboard = aggregate(
            attempts = attempts,
            sessions = sessions,
            lexemes = lexemes,
        )

        assertEquals(2, dashboard.today.studyMinutes)
        assertEquals(2, dashboard.today.distinctLexemesReviewed)
        assertEquals(2, dashboard.today.attempts)
        assertEquals(0.5, dashboard.today.accuracy!!, 0.0001)
        assertEquals(1, dashboard.today.newWordsAdded)
    }

    @Test
    fun streakCountsCurrentAndLongestStudyDays() {
        val attempts = listOf(
            attempt(answeredAt = instantAt(today, 12)),
            attempt(answeredAt = instantAt(today.minusDays(1), 12)),
            attempt(answeredAt = instantAt(today.minusDays(2), 12)),
            attempt(answeredAt = instantAt(today.minusDays(4), 12)),
            attempt(answeredAt = instantAt(today.minusDays(5), 12)),
        )

        val dashboard = aggregate(attempts = attempts)

        assertEquals(3, dashboard.streaks.currentDays)
        assertEquals(3, dashboard.streaks.longestDays)
    }

    @Test
    fun longestStreakSpansAcrossGaps() {
        val attempts = listOf(
            attempt(answeredAt = instantAt(today, 12)),
            attempt(answeredAt = instantAt(today.minusDays(1), 12)),
            attempt(answeredAt = instantAt(today.minusDays(2), 12)),
            attempt(answeredAt = instantAt(today.minusDays(5), 12)),
            attempt(answeredAt = instantAt(today.minusDays(6), 12)),
            attempt(answeredAt = instantAt(today.minusDays(7), 12)),
            attempt(answeredAt = instantAt(today.minusDays(8), 12)),
        )

        val dashboard = aggregate(attempts = attempts)

        assertEquals(3, dashboard.streaks.currentDays)
        assertEquals(4, dashboard.streaks.longestDays)
    }

    @Test
    fun currentStreakIsZeroWhenTodayHasNoStudy() {
        val attempts = listOf(
            attempt(answeredAt = instantAt(today.minusDays(1), 12)),
            attempt(answeredAt = instantAt(today.minusDays(2), 12)),
        )

        val dashboard = aggregate(attempts = attempts)

        assertEquals(0, dashboard.streaks.currentDays)
        assertEquals(2, dashboard.streaks.longestDays)
    }

    @Test
    fun lastSevenDaysSeriesOrdersChronologicallyFromWeekStartToToday() {
        val weekStart = today.minusDays(6)
        val dashboard = aggregate()

        assertEquals(7, dashboard.lastSevenDays.size)
        assertEquals(weekStart.format(java.time.format.DateTimeFormatter.ofPattern("MM/dd")), dashboard.lastSevenDays.first().dateLabel)
        assertEquals(today.format(java.time.format.DateTimeFormatter.ofPattern("MM/dd")), dashboard.lastSevenDays.last().dateLabel)
    }

    @Test
    fun lastSevenDaysSeriesIncludesDailyMetrics() {
        val attempts = listOf(
            attempt(
                lexemeId = "lex-a",
                correct = true,
                answeredAt = instantAt(today.minusDays(1), 12),
            ),
            attempt(
                lexemeId = "lex-b",
                correct = false,
                answeredAt = instantAt(today.minusDays(1), 13),
            ),
        )
        val sessions = listOf(
            session(
                startedAt = instantAt(today.minusDays(1), 11),
                activeMillis = 180_000L,
            ),
        )
        val lexemes = listOf(
            lexeme("lex-a", "alpha", instantAt(today.minusDays(1), 8)),
        )

        val dashboard = aggregate(
            attempts = attempts,
            sessions = sessions,
            lexemes = lexemes,
        )

        assertEquals(7, dashboard.lastSevenDays.size)
        val yesterday = dashboard.lastSevenDays[5]
        assertEquals(3, yesterday.studyMinutes)
        assertEquals(2, yesterday.reviewedLexemes)
        assertEquals(0.5, yesterday.accuracy!!, 0.0001)
        assertEquals(1, yesterday.newWordsAdded)
    }

    @Test
    fun questionTypeStatsGroupAttemptsAndAccuracy() {
        val attempts = listOf(
            attempt(questionType = QuestionType.MEANING, correct = true),
            attempt(questionType = QuestionType.MEANING, correct = false),
            attempt(questionType = QuestionType.REORDER, correct = true),
            attempt(questionType = QuestionType.USAGE, correct = false),
            attempt(questionType = QuestionType.USAGE, correct = true),
            attempt(questionType = QuestionType.INFLECTION, correct = true),
        )

        val dashboard = aggregate(attempts = attempts)
        val meaning = dashboard.byQuestionType.first { it.questionType == QuestionType.MEANING }
        val reorder = dashboard.byQuestionType.first { it.questionType == QuestionType.REORDER }
        val usage = dashboard.byQuestionType.first { it.questionType == QuestionType.USAGE }
        val inflection = dashboard.byQuestionType.first { it.questionType == QuestionType.INFLECTION }

        assertEquals(2, meaning.attempts)
        assertEquals(0.5, meaning.accuracy!!, 0.0001)
        assertEquals(1, reorder.attempts)
        assertEquals(1.0, reorder.accuracy!!, 0.0001)
        assertEquals(2, usage.attempts)
        assertEquals(0.5, usage.accuracy!!, 0.0001)
        assertEquals(1, inflection.attempts)
        assertEquals(1.0, inflection.accuracy!!, 0.0001)
    }

    @Test
    fun questionTypeStatsExcludeAttemptsOutsideRollingWeek() {
        val attempts = listOf(
            attempt(
                questionType = QuestionType.MEANING,
                answeredAt = instantAt(today.minusDays(7), 12),
            ),
            attempt(
                questionType = QuestionType.MEANING,
                answeredAt = instantAt(today.minusDays(6), 12),
            ),
        )

        val dashboard = aggregate(attempts = attempts)
        val meaning = dashboard.byQuestionType.first { it.questionType == QuestionType.MEANING }

        assertEquals(1, meaning.attempts)
    }

    @Test
    fun weakWordsRankByDifficultyAndRecentWrong() {
        val questionStats = listOf(
            stats(
                questionKey = "easy",
                lexemeId = "lex-easy",
                difficultyEma = 0.2,
                lastResult = ReviewResult.CORRECT,
            ),
            stats(
                questionKey = "hard-wrong",
                lexemeId = "lex-hard",
                difficultyEma = 0.9,
                lastResult = ReviewResult.WRONG,
            ),
        )
        val lexemes = listOf(
            lexeme("lex-easy", "easy"),
            lexeme("lex-hard", "hard"),
        )

        val dashboard = aggregate(
            questionStats = questionStats,
            lexemes = lexemes,
        )

        assertEquals("hard", dashboard.weakWords.first().headword)
        assertEquals(true, dashboard.weakWords.first().recentlyWrong)
    }

    @Test
    fun weakWordsBreakTiesWithWrongStreakAndLimitToTen() {
        val questionStats = (1..12).map { index ->
            stats(
                questionKey = "key-$index",
                lexemeId = "lex-$index",
                difficultyEma = 0.5,
                lastResult = if (index % 2 == 0) ReviewResult.WRONG else ReviewResult.CORRECT,
                wrongStreak = index,
            )
        }
        val lexemes = questionStats.map { lexeme(it.lexemeId, it.lexemeId) }

        val dashboard = aggregate(
            questionStats = questionStats,
            lexemes = lexemes,
        )

        assertEquals(10, dashboard.weakWords.size)
        assertEquals("lex-12", dashboard.weakWords.first().lexemeId)
        assertEquals(true, dashboard.weakWords.first().recentlyWrong)
    }

    @Test
    fun weakWordsExcludeEntriesWithZeroAttempts() {
        val questionStats = listOf(
            stats(
                questionKey = "unused",
                lexemeId = "lex-unused",
                difficultyEma = 0.99,
                lastResult = ReviewResult.WRONG,
                attempts = 0,
            ),
            stats(
                questionKey = "used",
                lexemeId = "lex-used",
                difficultyEma = 0.4,
                lastResult = ReviewResult.CORRECT,
            ),
        )

        val dashboard = aggregate(
            questionStats = questionStats,
            lexemes = listOf(lexeme("lex-used", "used")),
        )

        assertEquals(1, dashboard.weakWords.size)
        assertEquals("used", dashboard.weakWords.first().headword)
    }

    @Test
    fun vocabularyGrowthCountsTotalWeeklyAndMonthlyAdditions() {
        val lexemes = listOf(
            lexeme("lex-today", "today", instantAt(today, 8)),
            lexeme("lex-week", "week", instantAt(today.minusDays(2), 8)),
            lexeme("lex-month", "month", instantAt(today.minusDays(8), 8)),
            lexeme("lex-old", "old", instantAt(today.minusDays(40), 8)),
        )

        val dashboard = aggregate(lexemes = lexemes)

        assertEquals(4, dashboard.vocabularyGrowth.totalCount)
        assertEquals(2, dashboard.vocabularyGrowth.addedThisWeek)
        assertEquals(3, dashboard.vocabularyGrowth.addedThisMonth)
    }

    @Test
    fun accuracyReturnsNullWhenNoAttempts() {
        assertNull(StatsAggregator.accuracy(correct = 0, total = 0))
    }

    @Test
    fun timezoneBoundaryAssignsAttemptsToCorrectLocalDay() {
        val laZone = ZoneId.of("America/Los_Angeles")
        val laToday = LocalDate.of(2026, 6, 10)
        val laNow = laToday.atTime(12, 0).atZone(laZone).toInstant()
        val latePreviousDayUtc = "2026-06-10T06:30:00Z"
        val earlyTodayUtc = "2026-06-10T08:30:00Z"

        val dashboard = StatsAggregator.aggregateDashboard(
            attempts = listOf(
                attempt(answeredAt = latePreviousDayUtc),
                attempt(answeredAt = earlyTodayUtc),
            ),
            sessions = emptyList(),
            questionStats = emptyList(),
            lexemes = emptyList(),
            now = laNow,
            zoneId = laZone,
        )

        assertEquals(1, dashboard.today.attempts)
        assertEquals(1, dashboard.today.distinctLexemesReviewed)
        val yesterdayPoint = dashboard.lastSevenDays[5]
        assertEquals(1, yesterdayPoint.reviewedLexemes)
        assertEquals(1, dashboard.lastSevenDays.last().reviewedLexemes)
    }

    @Test
    fun studyDayCountsSessionsWithActiveMillisOrAnswers() {
        val dashboard = aggregate(
            sessions = listOf(
                session(
                    startedAt = instantAt(today.minusDays(3), 9),
                    activeMillis = 0L,
                    answeredCount = 0,
                ),
                session(
                    startedAt = instantAt(today.minusDays(2), 9),
                    activeMillis = 1_000L,
                    answeredCount = 0,
                ),
            ),
        )

        assertEquals(0, dashboard.streaks.currentDays)
        assertEquals(1, dashboard.streaks.longestDays)
    }

    @Test
    fun millisToMinutesRoundsToNearestMinute() {
        assertEquals(2, StatsAggregator.millisToMinutes(125_000L))
        assertEquals(3, StatsAggregator.millisToMinutes(150_000L))
    }

    private fun aggregate(
        attempts: List<ReviewAttemptEvent> = emptyList(),
        sessions: List<StudySession> = emptyList(),
        questionStats: List<QuestionStats> = emptyList(),
        lexemes: List<LexemeStatsInput> = emptyList(),
    ) = StatsAggregator.aggregateDashboard(
        attempts = attempts,
        sessions = sessions,
        questionStats = questionStats,
        lexemes = lexemes,
        now = now,
        zoneId = zoneId,
    )

    private fun attempt(
        lexemeId: String = "lex-a",
        questionType: QuestionType = QuestionType.MEANING,
        correct: Boolean = true,
        answeredAt: String = instantAt(today, 12),
    ) = ReviewAttemptEvent(
        id = "attempt-$answeredAt-$lexemeId",
        sessionId = "session-1",
        questionKey = "key-$lexemeId",
        questionType = questionType.name,
        lexemeId = lexemeId,
        correct = correct,
        answeredAt = answeredAt,
        elapsedActiveMillis = 1_000L,
    )

    private fun session(
        startedAt: String = instantAt(today, 9),
        activeMillis: Long = 60_000L,
        answeredCount: Int = 1,
        correctCount: Int = 1,
    ) = StudySession(
        id = "session-$startedAt",
        startedAt = startedAt,
        endedAt = startedAt,
        activeMillis = activeMillis,
        answeredCount = answeredCount,
        correctCount = correctCount,
    )

    private fun lexeme(
        lexemeId: String,
        headword: String,
        createdAt: String = ReviewFixtures.FIXTURE_TIMESTAMP,
    ) = LexemeStatsInput(
        lexemeId = lexemeId,
        headword = headword,
        createdAt = createdAt,
    )

    private fun stats(
        questionKey: String,
        lexemeId: String,
        difficultyEma: Double,
        lastResult: ReviewResult,
        attempts: Int = 3,
        wrongStreak: Int = if (lastResult == ReviewResult.WRONG) 1 else 0,
    ) = QuestionStats(
        questionKey = questionKey,
        questionType = QuestionType.MEANING,
        lexemeId = lexemeId,
        attempts = attempts,
        wrongCount = if (lastResult == ReviewResult.WRONG) 1 else 0,
        wrongStreak = wrongStreak,
        difficultyEma = difficultyEma,
        lastResult = lastResult,
        createdAt = ReviewFixtures.FIXTURE_TIMESTAMP,
        updatedAt = ReviewFixtures.FIXTURE_TIMESTAMP,
    )

    private fun instantAt(day: LocalDate, hour: Int): String =
        day.atTime(hour, 0).atZone(zoneId).toInstant().toString()
}
