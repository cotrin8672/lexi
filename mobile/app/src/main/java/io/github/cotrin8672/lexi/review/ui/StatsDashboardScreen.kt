package io.github.cotrin8672.lexi.review.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import io.github.cotrin8672.lexi.review.QuestionType
import io.github.cotrin8672.lexi.review.stats.DailySeriesPoint
import io.github.cotrin8672.lexi.review.stats.QuestionTypeStats
import io.github.cotrin8672.lexi.review.stats.StatsDashboardState
import io.github.cotrin8672.lexi.review.stats.StreakStats
import io.github.cotrin8672.lexi.review.stats.TodayStats
import io.github.cotrin8672.lexi.review.stats.VocabularyGrowthStats
import io.github.cotrin8672.lexi.review.stats.WeakWordEntry
import io.github.cotrin8672.lexi.review.ui.components.SevenDayAccuracyChart
import io.github.cotrin8672.lexi.review.ui.components.SevenDayStudyMinutesChart
import io.github.cotrin8672.lexi.review.ui.components.SummaryStatCard
import io.github.cotrin8672.lexi.review.ui.theme.LexiReviewTheme
import kotlin.math.roundToInt

@Composable
fun StatsDashboardScreen(
    uiState: StatsUiState,
    onBack: () -> Unit,
    onRefresh: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp, vertical = 16.dp),
    ) {
        StatsDashboardHeader(onBack = onBack, onRefresh = onRefresh)
        Spacer(modifier = Modifier.height(12.dp))

        when (uiState) {
            StatsUiState.Loading -> {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f),
                    verticalArrangement = Arrangement.Center,
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    CircularProgressIndicator()
                    Spacer(modifier = Modifier.height(12.dp))
                    Text(
                        text = "統計を読み込み中…",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            is StatsUiState.Error -> {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f),
                    verticalArrangement = Arrangement.Center,
                ) {
                    Text(
                        text = uiState.message,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    OutlinedButton(onClick = onRefresh) {
                        Text("再読み込み")
                    }
                }
            }
            is StatsUiState.Ready -> {
                StatsDashboardBody(
                    dashboard = uiState.dashboard,
                    modifier = Modifier.weight(1f),
                )
            }
        }
    }
}

@Composable
private fun StatsDashboardHeader(
    onBack: () -> Unit,
    onRefresh: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = "統計",
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.SemiBold,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = onRefresh) {
                Text("更新")
            }
            OutlinedButton(onClick = onBack) {
                Text("戻る")
            }
        }
    }
}

@Composable
private fun StatsDashboardBody(
    dashboard: StatsDashboardState,
    modifier: Modifier = Modifier,
) {
    val studyDaysThisWeek = dashboard.lastSevenDays.count { day ->
        day.studyMinutes > 0 || day.reviewedLexemes > 0
    }

    Column(
        modifier = modifier.verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        StatsSection(title = "今日のサマリー") {
            TodaySummaryGrid(today = dashboard.today)
        }

        StatsSection(title = "継続") {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                SummaryStatCard(
                    label = "連続学習日",
                    value = "${dashboard.streaks.currentDays}日",
                    modifier = Modifier.weight(1f),
                )
                SummaryStatCard(
                    label = "今週の学習日",
                    value = "${studyDaysThisWeek}日",
                    modifier = Modifier.weight(1f),
                )
            }
        }

        StatsSection(title = "7日推移") {
            SevenDayStudyMinutesChart(
                series = dashboard.lastSevenDays,
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(modifier = Modifier.height(16.dp))
            SevenDayAccuracyChart(
                series = dashboard.lastSevenDays,
                modifier = Modifier.fillMaxWidth(),
            )
        }

        StatsSection(title = "問題種別別成績") {
            if (dashboard.byQuestionType.isEmpty()) {
                EmptySectionMessage()
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    dashboard.byQuestionType.forEach { row ->
                        QuestionTypeStatsRowItem(row = row)
                    }
                }
            }
        }

        StatsSection(title = "弱点語") {
            if (dashboard.weakWords.isEmpty()) {
                EmptySectionMessage()
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    dashboard.weakWords.forEach { word ->
                        WeakWordRowItem(row = word)
                    }
                }
            }
        }

        StatsSection(title = "語彙成長") {
            VocabularyGrowthRow(growth = dashboard.vocabularyGrowth)
        }
    }
}

@Composable
private fun StatsSection(
    title: String,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    Column(modifier = modifier.fillMaxWidth()) {
        Text(
            text = title,
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.Medium,
        )
        Spacer(modifier = Modifier.height(10.dp))
        content()
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun TodaySummaryGrid(
    today: TodayStats,
    modifier: Modifier = Modifier,
) {
    FlowRow(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
        maxItemsInEachRow = 2,
    ) {
        SummaryStatCard(
            label = "学習時間",
            value = "${today.studyMinutes}分",
            modifier = Modifier.weight(1f, fill = true),
        )
        SummaryStatCard(
            label = "復習した語",
            value = "${today.distinctLexemesReviewed}語",
            modifier = Modifier.weight(1f, fill = true),
        )
        SummaryStatCard(
            label = "回答数",
            value = "${today.attempts}問",
            modifier = Modifier.weight(1f, fill = true),
        )
        SummaryStatCard(
            label = "正答率",
            value = today.accuracy?.let { "${(it * 100).roundToInt()}%" } ?: "—",
            modifier = Modifier.weight(1f, fill = true),
        )
        SummaryStatCard(
            label = "新規追加語",
            value = "${today.newWordsAdded}語",
            modifier = Modifier.fillMaxWidth(0.48f),
        )
    }
}

@Composable
private fun VocabularyGrowthRow(
    growth: VocabularyGrowthStats,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        SummaryStatCard(
            label = "累計",
            value = "${growth.totalCount}語",
            modifier = Modifier.weight(1f),
        )
        SummaryStatCard(
            label = "今週追加",
            value = "${growth.addedThisWeek}語",
            modifier = Modifier.weight(1f),
        )
        SummaryStatCard(
            label = "今月追加",
            value = "${growth.addedThisMonth}語",
            modifier = Modifier.weight(1f),
        )
    }
}

@Composable
private fun QuestionTypeStatsRowItem(
    row: QuestionTypeStats,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(10.dp),
        color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.35f),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 14.dp, vertical = 12.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = row.questionType.statsLabel(),
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
            )
            Text(
                text = buildString {
                    append("${row.attempts}問")
                    append(" / ")
                    append(row.accuracy?.let { "${(it * 100).roundToInt()}%" } ?: "—")
                },
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun WeakWordRowItem(
    row: WeakWordEntry,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(10.dp),
        color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.35f),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 14.dp, vertical = 12.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = row.headword,
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.Medium,
            )
            Text(
                text = buildString {
                    append("難度 ")
                    append("${(row.difficultyEma * 100).roundToInt()}%")
                    if (row.recentlyWrong) {
                        append(" / 直近ミス")
                    }
                },
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun EmptySectionMessage(
    modifier: Modifier = Modifier,
) {
    Text(
        text = "まだデータがありません",
        modifier = modifier,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

private fun QuestionType.statsLabel(): String = when (this) {
    QuestionType.MEANING -> "意味選択"
    QuestionType.REORDER -> "並べ替え"
    QuestionType.USAGE -> "用法"
    QuestionType.INFLECTION -> "活用"
}

@Preview(showBackground = true)
@Composable
private fun StatsDashboardPreview() {
    LexiReviewTheme {
        StatsDashboardScreen(
            uiState = StatsUiState.Ready(
                dashboard = StatsDashboardState(
                    today = TodayStats(
                        studyMinutes = 18,
                        distinctLexemesReviewed = 24,
                        attempts = 32,
                        accuracy = 0.78,
                        newWordsAdded = 3,
                    ),
                    streaks = StreakStats(currentDays = 5, longestDays = 12),
                    lastSevenDays = listOf(
                        DailySeriesPoint("06/04", 12, 8, 0.70, 1),
                        DailySeriesPoint("06/05", 18, 10, 0.75, 0),
                        DailySeriesPoint("06/06", 0, 0, null, 0),
                        DailySeriesPoint("06/07", 22, 14, 0.80, 2),
                        DailySeriesPoint("06/08", 15, 9, 0.68, 0),
                        DailySeriesPoint("06/09", 30, 16, 0.82, 1),
                        DailySeriesPoint("06/10", 18, 12, 0.78, 3),
                    ),
                    byQuestionType = QuestionType.entries.map { type ->
                        QuestionTypeStats(
                            questionType = type,
                            attempts = 10,
                            accuracy = 0.75,
                        )
                    },
                    weakWords = listOf(
                        WeakWordEntry(
                            lexemeId = "lex-1",
                            headword = "ambiguous",
                            questionKey = "meaning:v1:lex-1:abc",
                            questionType = QuestionType.MEANING,
                            difficultyEma = 0.78,
                            recentlyWrong = true,
                        ),
                    ),
                    vocabularyGrowth = VocabularyGrowthStats(
                        totalCount = 128,
                        addedThisWeek = 6,
                        addedThisMonth = 18,
                    ),
                ),
            ),
            onBack = {},
            onRefresh = {},
        )
    }
}
