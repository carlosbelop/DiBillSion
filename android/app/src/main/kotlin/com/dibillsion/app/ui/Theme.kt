package com.dibillsion.app.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

// DiBillSion palette: deep teal for actions, warm amber as accent, coral for debts.
private val Light = lightColorScheme(
    primary = Color(0xFF00676A),
    onPrimary = Color(0xFFFFFFFF),
    primaryContainer = Color(0xFFA6EFF0),
    onPrimaryContainer = Color(0xFF002021),
    secondary = Color(0xFF4A6364),
    onSecondary = Color(0xFFFFFFFF),
    secondaryContainer = Color(0xFFCCE8E8),
    onSecondaryContainer = Color(0xFF051F20),
    tertiary = Color(0xFF8A5100),
    onTertiary = Color(0xFFFFFFFF),
    tertiaryContainer = Color(0xFFFFDCBC),
    onTertiaryContainer = Color(0xFF2C1600),
    error = Color(0xFFB3261E),
    errorContainer = Color(0xFFFFDAD6),
    onErrorContainer = Color(0xFF410002),
    background = Color(0xFFF7FAFA),
    onBackground = Color(0xFF191C1C),
    surface = Color(0xFFF7FAFA),
    onSurface = Color(0xFF191C1C),
    surfaceVariant = Color(0xFFDAE4E4),
    onSurfaceVariant = Color(0xFF3F4949),
    surfaceContainerLowest = Color(0xFFFFFFFF),
    surfaceContainerLow = Color(0xFFF1F5F5),
    surfaceContainer = Color(0xFFEBEFEF),
    surfaceContainerHigh = Color(0xFFE5E9E9),
    surfaceContainerHighest = Color(0xFFE0E3E3),
    outline = Color(0xFF6F7979),
    outlineVariant = Color(0xFFBEC8C8),
)

private val Dark = darkColorScheme(
    primary = Color(0xFF8AD3D4),
    onPrimary = Color(0xFF003738),
    primaryContainer = Color(0xFF004F51),
    onPrimaryContainer = Color(0xFFA6EFF0),
    secondary = Color(0xFFB0CCCC),
    onSecondary = Color(0xFF1B3435),
    secondaryContainer = Color(0xFF324B4C),
    onSecondaryContainer = Color(0xFFCCE8E8),
    tertiary = Color(0xFFFFB86E),
    onTertiary = Color(0xFF4A2800),
    tertiaryContainer = Color(0xFF693C00),
    onTertiaryContainer = Color(0xFFFFDCBC),
    error = Color(0xFFFFB4AB),
    errorContainer = Color(0xFF93000A),
    onErrorContainer = Color(0xFFFFDAD6),
    background = Color(0xFF0F1414),
    onBackground = Color(0xFFDEE4E3),
    surface = Color(0xFF0F1414),
    onSurface = Color(0xFFDEE4E3),
    surfaceVariant = Color(0xFF3F4949),
    onSurfaceVariant = Color(0xFFBEC8C8),
    surfaceContainerLowest = Color(0xFF0A0F0F),
    surfaceContainerLow = Color(0xFF171D1D),
    surfaceContainer = Color(0xFF1B2121),
    surfaceContainerHigh = Color(0xFF252B2B),
    surfaceContainerHighest = Color(0xFF303636),
    outline = Color(0xFF889393),
    outlineVariant = Color(0xFF3F4949),
)

private val AppShapes = Shapes(
    small = RoundedCornerShape(10.dp),
    medium = RoundedCornerShape(16.dp),
    large = RoundedCornerShape(24.dp),
)

@Composable
fun DiBillTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (isSystemInDarkTheme()) Dark else Light,
        shapes = AppShapes,
        content = content,
    )
}

/** Colour for money a member is owed (positive) or owes (negative). */
@Composable
@ReadOnlyComposable
fun balanceColor(amountMinor: Long): Color = when {
    amountMinor > 0 -> MaterialTheme.colorScheme.primary
    amountMinor < 0 -> MaterialTheme.colorScheme.error
    else -> MaterialTheme.colorScheme.onSurfaceVariant
}

// Avatar tints, picked per member so the same person keeps the same colour.
private val AvatarLight = listOf(0xFFA6EFF0, 0xFFFFDCBC, 0xFFD6E3FF, 0xFFF4D9FF, 0xFFC9EFC1, 0xFFFFD9E0)
private val AvatarDark = listOf(0xFF004F51, 0xFF693C00, 0xFF2B4678, 0xFF5B3A6B, 0xFF2E5128, 0xFF6E2D3B)

@Composable
@ReadOnlyComposable
fun avatarColor(key: String): Color {
    val palette = if (isSystemInDarkTheme()) AvatarDark else AvatarLight
    // String.hashCode() mod a small number barely varies between UUIDs, so mix it first.
    var h = key.hashCode()
    h = (h xor (h ushr 16)) * 0x45d9f3b
    h = h xor (h ushr 16)
    return Color(palette[Math.floorMod(h, palette.size)])
}
