package io.github.cotrin8672.lexi.review.stats

/**
 * Tracks foreground study time during a review session.
 * Pauses on background and stops accumulating after [idleCapMillis] without interaction.
 */
class StudySessionTracker(
    private val clock: () -> Long = { System.currentTimeMillis() },
    private val idleCapMillis: Long = DEFAULT_IDLE_CAP_MILLIS,
) {
    private var running = false
    private var paused = false
    private var accumulatedMillis = 0L
    private var segmentStartMillis: Long? = null
    private var lastInteractionMillis = 0L

    fun start(atMillis: Long = clock()) {
        running = true
        paused = false
        accumulatedMillis = 0L
        lastInteractionMillis = atMillis
        segmentStartMillis = atMillis
    }

    fun recordInteraction(atMillis: Long = clock()) {
        if (!running || paused) {
            return
        }
        if (segmentStartMillis != null && atMillis > lastInteractionMillis + idleCapMillis) {
            accumulatedMillis = 0L
            segmentStartMillis = atMillis
        } else if (segmentStartMillis == null) {
            segmentStartMillis = atMillis
        }
        lastInteractionMillis = atMillis
    }

    fun pause(atMillis: Long = clock()) {
        if (!running || paused) {
            return
        }
        accumulatedMillis += activeSegmentMillis(atMillis)
        segmentStartMillis = null
        paused = true
    }

    fun resume(atMillis: Long = clock()) {
        if (!running || !paused) {
            return
        }
        paused = false
        lastInteractionMillis = atMillis
        segmentStartMillis = atMillis
    }

    fun stop(atMillis: Long = clock()): Long {
        if (!running) {
            return accumulatedMillis
        }
        if (!paused) {
            accumulatedMillis += activeSegmentMillis(atMillis)
        }
        running = false
        paused = false
        segmentStartMillis = null
        return accumulatedMillis
    }

    fun currentActiveMillis(atMillis: Long = clock()): Long {
        val segmentMillis = if (!paused && segmentStartMillis != null) {
            activeSegmentMillis(atMillis)
        } else {
            0L
        }
        return accumulatedMillis + segmentMillis
    }

    val isRunning: Boolean
        get() = running

    private fun activeSegmentMillis(atMillis: Long): Long {
        val start = segmentStartMillis ?: return 0L
        val idleDeadline = lastInteractionMillis + idleCapMillis
        val effectiveEnd = minOf(atMillis, idleDeadline)
        return maxOf(0L, effectiveEnd - start)
    }

    companion object {
        const val DEFAULT_IDLE_CAP_MILLIS = 5 * 60 * 1000L
    }
}
