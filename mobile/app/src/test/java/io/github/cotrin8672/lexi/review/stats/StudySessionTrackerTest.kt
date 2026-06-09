package io.github.cotrin8672.lexi.review.stats

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class StudySessionTrackerTest {
    @Test
    fun accumulatesActiveTimeAcrossInteractions() {
        val tracker = StudySessionTracker()

        tracker.start(atMillis = 0L)
        tracker.recordInteraction(atMillis = 0L)
        tracker.recordInteraction(atMillis = 30_000L)

        assertEquals(30_000L, tracker.currentActiveMillis(atMillis = 30_000L))
        assertEquals(60_000L, tracker.stop(atMillis = 60_000L))
    }

    @Test
    fun pauseStopsAccumulationUntilResume() {
        val tracker = StudySessionTracker()

        tracker.start(atMillis = 0L)
        tracker.recordInteraction(atMillis = 0L)
        tracker.pause(atMillis = 30_000L)
        assertEquals(30_000L, tracker.currentActiveMillis(atMillis = 120_000L))

        tracker.resume(atMillis = 120_000L)
        tracker.recordInteraction(atMillis = 120_000L)
        assertEquals(60_000L, tracker.stop(atMillis = 150_000L))
    }

    @Test
    fun idleCapStopsCountingAfterFiveMinutesWithoutInteraction() {
        val tracker = StudySessionTracker(
            idleCapMillis = StudySessionTracker.DEFAULT_IDLE_CAP_MILLIS,
        )

        tracker.start(atMillis = 0L)
        tracker.recordInteraction(atMillis = 0L)

        val idleDeadline = StudySessionTracker.DEFAULT_IDLE_CAP_MILLIS
        assertEquals(idleDeadline, tracker.currentActiveMillis(atMillis = idleDeadline + 60_000L))
        assertEquals(idleDeadline, tracker.stop(atMillis = idleDeadline + 120_000L))
    }

    @Test
    fun stopReturnsFinalMillisAndEndsTracking() {
        val tracker = StudySessionTracker()

        tracker.start(atMillis = 1_000L)
        tracker.recordInteraction(atMillis = 1_000L)
        val finalMillis = tracker.stop(atMillis = 31_000L)

        assertEquals(30_000L, finalMillis)
        assertFalse(tracker.isRunning)
        assertEquals(30_000L, tracker.stop(atMillis = 60_000L))
    }

    @Test
    fun interactionAfterIdleResetsCountingWindow() {
        val tracker = StudySessionTracker()

        tracker.start(atMillis = 0L)
        tracker.recordInteraction(atMillis = 0L)
        val afterIdle = StudySessionTracker.DEFAULT_IDLE_CAP_MILLIS + 30_000L
        tracker.recordInteraction(atMillis = afterIdle)

        assertEquals(30_000L, tracker.stop(atMillis = afterIdle + 30_000L))
    }

    @Test
    fun pauseWhileNotRunningIsNoOp() {
        val tracker = StudySessionTracker()

        tracker.pause(atMillis = 10_000L)
        assertFalse(tracker.isRunning)
    }

    @Test
    fun resumeWhileNotPausedIsNoOp() {
        val tracker = StudySessionTracker()

        tracker.start(atMillis = 0L)
        tracker.resume(atMillis = 10_000L)
        assertTrue(tracker.isRunning)
        assertEquals(10_000L, tracker.stop(atMillis = 10_000L))
    }
}
