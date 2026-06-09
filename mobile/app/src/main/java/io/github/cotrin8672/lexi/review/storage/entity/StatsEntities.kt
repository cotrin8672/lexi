package io.github.cotrin8672.lexi.review.storage.entity

import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

@Entity(
    tableName = "review_attempt_events",
    indices = [Index(value = ["answeredAt"])],
)
data class ReviewAttemptEventEntity(
    @PrimaryKey val id: String,
    val sessionId: String,
    val questionKey: String,
    val questionType: String,
    val lexemeId: String,
    val correct: Boolean,
    val answeredAt: String,
    val elapsedActiveMillis: Long,
)

@Entity(
    tableName = "study_sessions",
    indices = [Index(value = ["startedAt"])],
)
data class StudySessionEntity(
    @PrimaryKey val id: String,
    val startedAt: String,
    val endedAt: String?,
    val activeMillis: Long,
    val answeredCount: Int,
    val correctCount: Int,
)
