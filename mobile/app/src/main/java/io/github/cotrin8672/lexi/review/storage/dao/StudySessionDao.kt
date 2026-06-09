package io.github.cotrin8672.lexi.review.storage.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import io.github.cotrin8672.lexi.review.storage.entity.StudySessionEntity

@Dao
interface StudySessionDao {
    @Insert(onConflict = OnConflictStrategy.ABORT)
    suspend fun insert(row: StudySessionEntity)

    @Query("SELECT * FROM study_sessions WHERE id = :sessionId LIMIT 1")
    suspend fun getById(sessionId: String): StudySessionEntity?

    @Query(
        """
        SELECT * FROM study_sessions
        WHERE startedAt >= :sinceInclusive
        ORDER BY startedAt ASC
        """,
    )
    suspend fun getSince(sinceInclusive: String): List<StudySessionEntity>

    @Query(
        """
        SELECT * FROM study_sessions
        WHERE startedAt >= :fromInclusive AND startedAt < :toExclusive
        ORDER BY startedAt ASC
        """,
    )
    suspend fun getBetween(
        fromInclusive: String,
        toExclusive: String,
    ): List<StudySessionEntity>

    @Query("UPDATE study_sessions SET endedAt = :endedAt WHERE id = :sessionId")
    suspend fun endSession(sessionId: String, endedAt: String)

    @Query("UPDATE study_sessions SET activeMillis = :activeMillis WHERE id = :sessionId")
    suspend fun updateActiveMillis(sessionId: String, activeMillis: Long)

    @Query(
        """
        UPDATE study_sessions
        SET answeredCount = answeredCount + 1,
            correctCount = correctCount + :correctIncrement
        WHERE id = :sessionId
        """,
    )
    suspend fun incrementAnswerCounts(sessionId: String, correctIncrement: Int)
}
