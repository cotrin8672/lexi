package io.github.cotrin8672.lexi.review.storage.entity

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "vocabulary_sync_state")
data class VocabularySyncStateEntity(
    @PrimaryKey val userId: String,
    val bootstrapComplete: Boolean,
    val lastServerRevision: Long,
    val updatedAt: String,
)
