package io.github.cotrin8672.lexi.review.storage.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import io.github.cotrin8672.lexi.review.storage.entity.QuestionStatsEntity

@Dao
interface QuestionStatsDao {
    @Query("SELECT * FROM question_stats WHERE questionKey = :questionKey LIMIT 1")
    suspend fun getByKey(questionKey: String): QuestionStatsEntity?

    @Query("SELECT * FROM question_stats")
    suspend fun getAll(): List<QuestionStatsEntity>

    @Query("SELECT * FROM question_stats WHERE questionKey IN (:questionKeys)")
    suspend fun getByKeys(questionKeys: List<String>): List<QuestionStatsEntity>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(row: QuestionStatsEntity)
}
