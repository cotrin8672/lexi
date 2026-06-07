package io.github.cotrin8672.lexi.review.ui.theme

import android.app.Activity
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalView
import androidx.core.view.ViewCompat

private val DarkColorScheme = darkColorScheme(
    surface = Color(0xFF1E252D),
    onSurface = Color(0xFFE4E7EB),
    primary = AccentSoft,
    onPrimary = Ink,
    secondary = Color(0xFF3A454F),
    onSecondary = Color(0xFFD8DEE5),
    tertiary = CorrectSoft,
    onTertiary = Color(0xFF1E2A24),
    error = IncorrectSoft,
    onError = Color(0xFF2A2020),
    surfaceVariant = Color(0xFF2C343D),
    onSurfaceVariant = Color(0xFFB8C0C8),
    outline = Color(0xFF4A5560),
)

private val LightColorScheme = lightColorScheme(
    surface = Paper,
    onSurface = Ink,
    primary = Accent,
    onPrimary = Color.White,
    secondary = ChoiceSurface,
    onSecondary = Ink,
    tertiary = CorrectSoft,
    onTertiary = Color(0xFF1E2A24),
    error = IncorrectSoft,
    onError = Color(0xFF2A2020),
    surfaceVariant = ChoiceSurface,
    onSurfaceVariant = MutedLabel,
    outline = ChoiceBorder,
)

@Suppress("DEPRECATION")
@Composable
fun LexiReviewTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    val colorScheme = if (darkTheme) DarkColorScheme else LightColorScheme
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            (view.context as Activity).window.statusBarColor = colorScheme.surface.toArgb()
            ViewCompat.getWindowInsetsController(view)?.isAppearanceLightStatusBars = !darkTheme
        }
    }

    MaterialTheme(
        colorScheme = colorScheme,
        typography = Typography,
        content = content,
    )
}
