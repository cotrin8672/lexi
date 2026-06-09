package io.github.cotrin8672.lexi.review.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import io.github.cotrin8672.lexi.review.stats.DailySeriesPoint
import io.github.cotrin8672.lexi.review.ui.theme.Accent
import io.github.cotrin8672.lexi.review.ui.theme.CorrectSoft
import io.github.cotrin8672.lexi.review.ui.theme.MutedLabel
import kotlin.math.max
import kotlin.math.roundToInt

@Composable
fun SevenDayStudyMinutesChart(
    series: List<DailySeriesPoint>,
    modifier: Modifier = Modifier,
    barColor: Color = Accent,
) {
    CompactBarChart(
        title = "学習時間（分）",
        series = series,
        modifier = modifier,
        valueLabel = { point -> "${point.studyMinutes}" },
        barValue = { point -> point.studyMinutes.toFloat() },
        barColor = barColor,
    )
}

@Composable
fun SevenDayAccuracyChart(
    series: List<DailySeriesPoint>,
    modifier: Modifier = Modifier,
    barColor: Color = CorrectSoft,
) {
    CompactBarChart(
        title = "正答率（%）",
        series = series,
        modifier = modifier,
        valueLabel = { point ->
            point.accuracy?.let { "${(it * 100).roundToInt()}%" } ?: "—"
        },
        barValue = { point -> ((point.accuracy ?: 0.0) * 100).toFloat() },
        barColor = barColor,
        maxValue = 100f,
    )
}

@Composable
private fun CompactBarChart(
    title: String,
    series: List<DailySeriesPoint>,
    modifier: Modifier = Modifier,
    valueLabel: (DailySeriesPoint) -> String,
    barValue: (DailySeriesPoint) -> Float,
    barColor: Color,
    maxValue: Float? = null,
) {
    Column(modifier = modifier.fillMaxWidth()) {
        Text(
            text = title,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (series.isEmpty()) {
            Text(
                text = "データがありません",
                modifier = Modifier.padding(top = 12.dp),
                style = MaterialTheme.typography.bodySmall,
                color = MutedLabel,
            )
            return
        }

        val computedMax = max(maxValue ?: 0f, series.maxOf { barValue(it) }.coerceAtLeast(1f))
        Canvas(
            modifier = Modifier
                .fillMaxWidth()
                .height(120.dp)
                .padding(top = 12.dp, bottom = 4.dp),
        ) {
            val barCount = series.size
            val gap = size.width * 0.04f
            val barWidth = ((size.width - gap * (barCount + 1)) / barCount).coerceAtLeast(8f)
            val chartHeight = size.height

            series.forEachIndexed { index, point ->
                val value = barValue(point)
                val normalized = if (computedMax <= 0f) 0f else value / computedMax
                val barHeight = chartHeight * normalized
                val left = gap + index * (barWidth + gap)
                val top = chartHeight - barHeight

                drawRoundRect(
                    color = barColor.copy(alpha = if (value <= 0f) 0.25f else 0.9f),
                    topLeft = Offset(left, top),
                    size = Size(barWidth, barHeight.coerceAtLeast(if (value > 0f) 4f else 0f)),
                    cornerRadius = CornerRadius(6f, 6f),
                )
            }
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            series.forEach { point ->
                Column(
                    modifier = Modifier.weight(1f),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(
                        text = valueLabel(point),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                        maxLines = 1,
                    )
                    Text(
                        text = point.dateLabel,
                        style = MaterialTheme.typography.labelSmall,
                        color = MutedLabel,
                        textAlign = TextAlign.Center,
                        maxLines = 1,
                    )
                }
            }
        }
    }
}
