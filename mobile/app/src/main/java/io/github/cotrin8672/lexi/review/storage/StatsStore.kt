package io.github.cotrin8672.lexi.review.storage

import io.github.cotrin8672.lexi.review.storage.dao.ReviewAttemptEventDao
import io.github.cotrin8672.lexi.review.storage.dao.StudySessionDao
import io.github.cotrin8672.lexi.review.storage.entity.ReviewAttemptEventEntity
import io.github.cotrin8672.lexi.review.storage.entity.StudySessionEntity

data class ReviewAttemptEvent(
    val id: String,
    val sessionId: String,
    val questionKey: String,
    val questionType: String,
    val lexemeId: String,
    val correct: Boolean,
    val answeredAt: String,
    val elapsedActiveMillis: Long,
)

data class StudySession(
    val id: String,
    val startedAt: String,
    val endedAt: String?,
    val activeMillis: Long,
    val answeredCount: Int,
    val correctCount: Int,
)

interface StatsStore {
    suspend fun insertAttemptEvent(event: ReviewAttemptEvent)

    suspend fun startSession(session: StudySession)

    suspend fun endSession(sessionId: String, endedAt: String)

    suspend fun updateSessionActiveMillis(sessionId: String, activeMillis: Long)

    suspend fun incrementSessionAnswer(sessionId: String, correct: Boolean)

    suspend fun getSession(sessionId: String): StudySession?

    suspend fun getAttemptsSince(sinceInclusive: String): List<ReviewAttemptEvent>

    suspend fun getAttemptsBetween(
        fromInclusive: String,
        toExclusive: String,
    ): List<ReviewAttemptEvent>

    suspend fun getAttemptsForSession(sessionId: String): List<ReviewAttemptEvent>

    suspend fun getSessionsSince(sinceInclusive: String): List<StudySession>

    suspend fun getSessionsBetween(
        fromInclusive: String,
        toExclusive: String,
    ): List<StudySession>
}

class RoomStatsStore(
    private val attemptEventDao: ReviewAttemptEventDao,
    private val studySessionDao: StudySessionDao,
) : StatsStore {
    override suspend fun insertAttemptEvent(event: ReviewAttemptEvent) {
        attemptEventDao.insert(event.toEntity())
    }

    override suspend fun startSession(session: StudySession) {
        studySessionDao.insert(session.toEntity())
    }

    override suspend fun endSession(sessionId: String, endedAt: String) {
        studySessionDao.endSession(sessionId, endedAt)
    }

    override suspend fun updateSessionActiveMillis(sessionId: String, activeMillis: Long) {
        studySessionDao.updateActiveMillis(sessionId, activeMillis)
    }

    override suspend fun incrementSessionAnswer(sessionId: String, correct: Boolean) {
        studySessionDao.incrementAnswerCounts(
            sessionId = sessionId,
            correctIncrement = if (correct) 1 else 0,
        )
    }

    override suspend fun getSession(sessionId: String): StudySession? =
        studySessionDao.getById(sessionId)?.toDomain()

    override suspend fun getAttemptsSince(sinceInclusive: String): List<ReviewAttemptEvent> =
        attemptEventDao.getSince(sinceInclusive).map { it.toDomain() }

    override suspend fun getAttemptsBetween(
        fromInclusive: String,
        toExclusive: String,
    ): List<ReviewAttemptEvent> =
        attemptEventDao.getBetween(fromInclusive, toExclusive).map { it.toDomain() }

    override suspend fun getAttemptsForSession(sessionId: String): List<ReviewAttemptEvent> =
        attemptEventDao.getBySessionId(sessionId).map { it.toDomain() }

    override suspend fun getSessionsSince(sinceInclusive: String): List<StudySession> =
        studySessionDao.getSince(sinceInclusive).map { it.toDomain() }

    override suspend fun getSessionsBetween(
        fromInclusive: String,
        toExclusive: String,
    ): List<StudySession> =
        studySessionDao.getBetween(fromInclusive, toExclusive).map { it.toDomain() }
}

private fun ReviewAttemptEventEntity.toDomain(): ReviewAttemptEvent = ReviewAttemptEvent(
    id = id,
    sessionId = sessionId,
    questionKey = questionKey,
    questionType = questionType,
    lexemeId = lexemeId,
    correct = correct,
    answeredAt = answeredAt,
    elapsedActiveMillis = elapsedActiveMillis,
)

private fun ReviewAttemptEvent.toEntity(): ReviewAttemptEventEntity = ReviewAttemptEventEntity(
    id = id,
    sessionId = sessionId,
    questionKey = questionKey,
    questionType = questionType,
    lexemeId = lexemeId,
    correct = correct,
    answeredAt = answeredAt,
    elapsedActiveMillis = elapsedActiveMillis,
)

private fun StudySessionEntity.toDomain(): StudySession = StudySession(
    id = id,
    startedAt = startedAt,
    endedAt = endedAt,
    activeMillis = activeMillis,
    answeredCount = answeredCount,
    correctCount = correctCount,
)

private fun StudySession.toEntity(): StudySessionEntity = StudySessionEntity(
    id = id,
    startedAt = startedAt,
    endedAt = endedAt,
    activeMillis = activeMillis,
    answeredCount = answeredCount,
    correctCount = correctCount,
)

class InMemoryStatsStore : StatsStore {
    private val attempts = mutableListOf<ReviewAttemptEvent>()
    private val sessions = linkedMapOf<String, StudySession>()

    override suspend fun insertAttemptEvent(event: ReviewAttemptEvent) {
        attempts += event
    }

    override suspend fun startSession(session: StudySession) {
        sessions[session.id] = session
    }

    override suspend fun endSession(sessionId: String, endedAt: String) {
        val existing = sessions[sessionId] ?: return
        sessions[sessionId] = existing.copy(endedAt = endedAt)
    }

    override suspend fun updateSessionActiveMillis(sessionId: String, activeMillis: Long) {
        val existing = sessions[sessionId] ?: return
        sessions[sessionId] = existing.copy(activeMillis = activeMillis)
    }

    override suspend fun incrementSessionAnswer(sessionId: String, correct: Boolean) {
        val existing = sessions[sessionId] ?: return
        sessions[sessionId] = existing.copy(
            answeredCount = existing.answeredCount + 1,
            correctCount = existing.correctCount + if (correct) 1 else 0,
        )
    }

    override suspend fun getSession(sessionId: String): StudySession? = sessions[sessionId]

    override suspend fun getAttemptsSince(sinceInclusive: String): List<ReviewAttemptEvent> =
        attempts.filter { it.answeredAt >= sinceInclusive }

    override suspend fun getAttemptsBetween(
        fromInclusive: String,
        toExclusive: String,
    ): List<ReviewAttemptEvent> =
        attempts.filter { it.answeredAt >= fromInclusive && it.answeredAt < toExclusive }

    override suspend fun getAttemptsForSession(sessionId: String): List<ReviewAttemptEvent> =
        attempts.filter { it.sessionId == sessionId }

    override suspend fun getSessionsSince(sinceInclusive: String): List<StudySession> =
        sessions.values.filter { it.startedAt >= sinceInclusive }

    override suspend fun getSessionsBetween(
        fromInclusive: String,
        toExclusive: String,
    ): List<StudySession> =
        sessions.values.filter { it.startedAt >= fromInclusive && it.startedAt < toExclusive }
}
