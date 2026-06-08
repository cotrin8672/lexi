package io.github.cotrin8672.lexi.review.storage.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import io.github.cotrin8672.lexi.review.storage.entity.VocabularySyncStateEntity

@Dao
interface VocabularySyncStateDao {
    @Query("SELECT * FROM vocabulary_sync_state WHERE userId = :userId LIMIT 1")
    suspend fun get(userId: String): VocabularySyncStateEntity?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(state: VocabularySyncStateEntity)
}
