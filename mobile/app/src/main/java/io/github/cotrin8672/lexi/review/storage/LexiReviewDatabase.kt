package io.github.cotrin8672.lexi.review.storage

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase
import io.github.cotrin8672.lexi.review.storage.dao.QuestionStatsDao
import io.github.cotrin8672.lexi.review.storage.dao.VocabularyCacheDao
import io.github.cotrin8672.lexi.review.storage.dao.VocabularySyncStateDao
import io.github.cotrin8672.lexi.review.storage.entity.CachedCardSnapshotEntity
import io.github.cotrin8672.lexi.review.storage.entity.CachedLexemeFormEntity
import io.github.cotrin8672.lexi.review.storage.entity.CachedUserLexemeEntity
import io.github.cotrin8672.lexi.review.storage.entity.QuestionStatsEntity
import io.github.cotrin8672.lexi.review.storage.entity.VocabularySyncStateEntity

@Database(
    entities = [
        CachedUserLexemeEntity::class,
        CachedCardSnapshotEntity::class,
        CachedLexemeFormEntity::class,
        QuestionStatsEntity::class,
        VocabularySyncStateEntity::class,
    ],
    version = 3,
    exportSchema = false,
)
abstract class LexiReviewDatabase : RoomDatabase() {
    abstract fun vocabularyCacheDao(): VocabularyCacheDao
    abstract fun vocabularySyncStateDao(): VocabularySyncStateDao
    abstract fun questionStatsDao(): QuestionStatsDao

    companion object {
        val MIGRATION_2_3 = object : Migration(2, 3) {
            override fun migrate(db: SupportSQLiteDatabase) {
                db.execSQL(
                    "ALTER TABLE cached_user_lexemes ADD COLUMN createdAt TEXT NOT NULL DEFAULT ''",
                )
                db.execSQL(
                    "UPDATE cached_user_lexemes SET createdAt = updatedAt WHERE createdAt = ''",
                )
            }
        }
    }
}
