package com.dibillsion.app.ui

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.spring
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import kotlin.math.abs

/**
 * An elastic edge at the bottom of a list: dragging past the end pulls the content up with
 * growing resistance, and letting go springs it back. Unlike the platform stretch, it also
 * works when the list is too short to scroll. The top edge is left to pull-to-refresh.
 */
@Composable
fun Modifier.bottomBounce(): Modifier {
    val max = with(LocalDensity.current) { 48.dp.toPx() }
    var offset by remember { mutableFloatStateOf(0f) }
    val connection = remember(max) {
        object : NestedScrollConnection {
            override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset {
                // Dragging back down while stretched undoes the stretch before scrolling.
                if (offset < 0f && available.y > 0f && source == NestedScrollSource.UserInput) {
                    val used = minOf(available.y, -offset)
                    offset += used
                    return Offset(0f, used)
                }
                return Offset.Zero
            }

            override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset {
                if (available.y < 0f && source == NestedScrollSource.UserInput) {
                    val resistance = 0.35f * (1f - abs(offset) / max)
                    offset = (offset + available.y * resistance).coerceAtLeast(-max)
                    return Offset(0f, available.y)
                }
                return Offset.Zero
            }

            override suspend fun onPreFling(available: Velocity): Velocity {
                if (offset == 0f) return Velocity.Zero
                animate(offset, 0f, animationSpec = spring(dampingRatio = 0.55f, stiffness = Spring.StiffnessMediumLow)) { v, _ ->
                    offset = v
                }
                return available
            }
        }
    }
    return nestedScroll(connection).graphicsLayer { translationY = offset }
}
