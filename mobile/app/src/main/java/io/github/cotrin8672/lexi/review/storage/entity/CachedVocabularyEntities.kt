package io.github.cotrin8672.lexi.review.storage.entity

import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

@Entity(tableName = "cached_user_lexemes")
data class CachedUserLexemeEntity(
    @PrimaryKey val id: String,
    val userId: String,
    val language: String,
    val canonicalText: String,
    val canonicalKey: String,
    val partOfSpeech: String?,
    val createdAt: String = "",
    val updatedAt: String,
)

@Entity(
    tableName = "cached_card_snapshots",
    indices = [Index(value = ["lexemeId", "active"])],
)
data class CachedCardSnapshotEntity(
    @PrimaryKey val id: String,
    val userId: String,
    val lexemeId: String,
    val schemaVersion: String,
    val provider: String,
    val model: String,
    val resultLanguage: String,
    val contentJson: String,
    val active: Boolean,
    val createdAt: String,
    val updatedAt: String,
    val remoteOperationId: String? = null,
    val remoteServerRevision: Long? = null,
)

@Entity(
    tableName = "cached_lexeme_forms",
    indices = [Index(value = ["lexemeId"])],
)
data class CachedLexemeFormEntity(
    @PrimaryKey val id: String,
    val userId: String,
    val lexemeId: String,
    val language: String,
    val formText: String,
    val formKey: String,
    val relation: String,
    val source: String,
    val updatedAt: String,
)

@Entity(tableName = "question_stats")
data class QuestionStatsEntity(
    @PrimaryKey val questionKey: String,
    val questionType: String,
    val lexemeId: String,
    val attempts: Int,
    val correctCount: Int,
    val wrongCount: Int,
    val correctStreak: Int,
    val wrongStreak: Int,
    val difficultyEma: Double,
    val lastResult: String?,
    val lastReviewedAt: String?,
    val lastSeenSequence: Long?,
    val createdAt: String,
    val updatedAt: String,
)
