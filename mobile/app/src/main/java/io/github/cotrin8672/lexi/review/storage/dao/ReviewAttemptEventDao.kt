package io.github.cotrin8672.lexi.review.storage.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import io.github.cotrin8672.lexi.review.storage.entity.ReviewAttemptEventEntity

@Dao
interface ReviewAttemptEventDao {
    @Insert(onConflict = OnConflictStrategy.ABORT)
    suspend fun insert(row: ReviewAttemptEventEntity)

    @Query(
        """
        SELECT * FROM review_attempt_events
        WHERE answeredAt >= :sinceInclusive
        ORDER BY answeredAt ASC
        """,
    )
    suspend fun getSince(sinceInclusive: String): List<ReviewAttemptEventEntity>

    @Query(
        """
        SELECT * FROM review_attempt_events
        WHERE answeredAt >= :fromInclusive AND answeredAt < :toExclusive
        ORDER BY answeredAt ASC
        """,
    )
    suspend fun getBetween(
        fromInclusive: String,
        toExclusive: String,
    ): List<ReviewAttemptEventEntity>

    @Query(
        """
        SELECT * FROM review_attempt_events
        WHERE sessionId = :sessionId
        ORDER BY answeredAt ASC
        """,
    )
    suspend fun getBySessionId(sessionId: String): List<ReviewAttemptEventEntity>
}
