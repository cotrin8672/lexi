package io.github.cotrin8672.lexi.review.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import io.github.cotrin8672.lexi.review.stats.StatsDashboardState
import io.github.cotrin8672.lexi.review.stats.TodayStats
import io.github.cotrin8672.lexi.review.ui.StatsUiState
import kotlin.math.roundToInt

@Composable
fun HomeStatsOverview(
    statsUiState: StatsUiState,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(16.dp),
        color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.35f),
    ) {
        Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 14.dp)) {
            Text(
                text = "今日の学習",
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.SemiBold,
            )
            Spacer(modifier = Modifier.height(10.dp))
            when (statsUiState) {
                StatsUiState.Loading -> {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.Center,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        CircularProgressIndicator(modifier = Modifier.size(22.dp), strokeWidth = 2.dp)
                    }
                }
                is StatsUiState.Error -> {
                    Text(
                        text = "サインインすると今日の統計を表示できます",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                is StatsUiState.Ready -> {
                    HomeStatsOverviewGrid(
                        today = statsUiState.dashboard.today,
                        currentStreakDays = statsUiState.dashboard.streaks.currentDays,
                    )
                }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun HomeStatsOverviewGrid(
    today: TodayStats,
    currentStreakDays: Int,
    modifier: Modifier = Modifier,
) {
    FlowRow(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
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
            label = "連続学習",
            value = "${currentStreakDays}日",
            modifier = Modifier.weight(1f, fill = true),
        )
        SummaryStatCard(
            label = "正答率",
            value = today.accuracy?.let { "${(it * 100).roundToInt()}%" } ?: "—",
            modifier = Modifier.weight(1f, fill = true),
        )
    }
}

@Composable
fun SummaryStatCard(
    label: String,
    value: String,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(12.dp),
        color = MaterialTheme.colorScheme.surface.copy(alpha = 0.72f),
    ) {
        Column(modifier = Modifier.padding(horizontal = 12.dp, vertical = 10.dp)) {
            Text(
                text = label,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(modifier = Modifier.height(2.dp))
            Text(
                text = value,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
            )
        }
    }
}
