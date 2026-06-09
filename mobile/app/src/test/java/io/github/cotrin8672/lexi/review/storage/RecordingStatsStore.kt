package io.github.cotrin8672.lexi.review.storage

/**
 * In-memory [StatsStore] that records calls for ViewModel and integration tests.
 */
class RecordingStatsStore(
    private val delegate: StatsStore = InMemoryStatsStore(),
) : StatsStore {
    val sessionsStarted = mutableListOf<StudySession>()
    val attemptEventsInserted = mutableListOf<ReviewAttemptEvent>()
    val sessionActiveMillisUpdates = mutableListOf<Pair<String, Long>>()
    val sessionAnswersIncremented = mutableListOf<Pair<String, Boolean>>()
    val sessionsEnded = mutableListOf<Pair<String, String>>()
    val attemptsSinceQueries = mutableListOf<String>()
    val sessionsSinceQueries = mutableListOf<String>()

    suspend fun seedAttempt(event: ReviewAttemptEvent) {
        delegate.insertAttemptEvent(event)
    }

    suspend fun seedSession(session: StudySession) {
        delegate.startSession(session)
    }

    override suspend fun insertAttemptEvent(event: ReviewAttemptEvent) {
        attemptEventsInserted += event
        delegate.insertAttemptEvent(event)
    }

    override suspend fun startSession(session: StudySession) {
        sessionsStarted += session
        delegate.startSession(session)
    }

    override suspend fun endSession(sessionId: String, endedAt: String) {
        sessionsEnded += sessionId to endedAt
        delegate.endSession(sessionId, endedAt)
    }

    override suspend fun updateSessionActiveMillis(sessionId: String, activeMillis: Long) {
        sessionActiveMillisUpdates += sessionId to activeMillis
        delegate.updateSessionActiveMillis(sessionId, activeMillis)
    }

    override suspend fun incrementSessionAnswer(sessionId: String, correct: Boolean) {
        sessionAnswersIncremented += sessionId to correct
        delegate.incrementSessionAnswer(sessionId, correct)
    }

    override suspend fun getSession(sessionId: String): StudySession? =
        delegate.getSession(sessionId)

    override suspend fun getAttemptsSince(sinceInclusive: String): List<ReviewAttemptEvent> {
        attemptsSinceQueries += sinceInclusive
        return delegate.getAttemptsSince(sinceInclusive)
    }

    override suspend fun getAttemptsBetween(
        fromInclusive: String,
        toExclusive: String,
    ): List<ReviewAttemptEvent> = delegate.getAttemptsBetween(fromInclusive, toExclusive)

    override suspend fun getAttemptsForSession(sessionId: String): List<ReviewAttemptEvent> =
        delegate.getAttemptsForSession(sessionId)

    override suspend fun getSessionsSince(sinceInclusive: String): List<StudySession> {
        sessionsSinceQueries += sinceInclusive
        return delegate.getSessionsSince(sinceInclusive)
    }

    override suspend fun getSessionsBetween(
        fromInclusive: String,
        toExclusive: String,
    ): List<StudySession> = delegate.getSessionsBetween(fromInclusive, toExclusive)
}
