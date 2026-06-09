package io.github.cotrin8672.lexi.review.storage

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase
import io.github.cotrin8672.lexi.review.storage.dao.QuestionStatsDao
import io.github.cotrin8672.lexi.review.storage.dao.ReviewAttemptEventDao
import io.github.cotrin8672.lexi.review.storage.dao.StudySessionDao
import io.github.cotrin8672.lexi.review.storage.dao.VocabularyCacheDao
import io.github.cotrin8672.lexi.review.storage.dao.VocabularySyncStateDao
import io.github.cotrin8672.lexi.review.storage.entity.CachedCardSnapshotEntity
import io.github.cotrin8672.lexi.review.storage.entity.CachedLexemeFormEntity
import io.github.cotrin8672.lexi.review.storage.entity.CachedUserLexemeEntity
import io.github.cotrin8672.lexi.review.storage.entity.QuestionStatsEntity
import io.github.cotrin8672.lexi.review.storage.entity.ReviewAttemptEventEntity
import io.github.cotrin8672.lexi.review.storage.entity.StudySessionEntity
import io.github.cotrin8672.lexi.review.storage.entity.VocabularySyncStateEntity

@Database(
    entities = [
        CachedUserLexemeEntity::class,
        CachedCardSnapshotEntity::class,
        CachedLexemeFormEntity::class,
        QuestionStatsEntity::class,
        VocabularySyncStateEntity::class,
        ReviewAttemptEventEntity::class,
        StudySessionEntity::class,
    ],
    version = 4,
    exportSchema = false,
)
abstract class LexiReviewDatabase : RoomDatabase() {
    abstract fun vocabularyCacheDao(): VocabularyCacheDao
    abstract fun vocabularySyncStateDao(): VocabularySyncStateDao
    abstract fun questionStatsDao(): QuestionStatsDao
    abstract fun reviewAttemptEventDao(): ReviewAttemptEventDao
    abstract fun studySessionDao(): StudySessionDao

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

        val MIGRATION_3_4 = object : Migration(3, 4) {
            override fun migrate(db: SupportSQLiteDatabase) {
                db.execSQL(
                    """
                    CREATE TABLE IF NOT EXISTS review_attempt_events (
                        id TEXT NOT NULL PRIMARY KEY,
                        sessionId TEXT NOT NULL,
                        questionKey TEXT NOT NULL,
                        questionType TEXT NOT NULL,
                        lexemeId TEXT NOT NULL,
                        correct INTEGER NOT NULL,
                        answeredAt TEXT NOT NULL,
                        elapsedActiveMillis INTEGER NOT NULL
                    )
                    """.trimIndent(),
                )
                db.execSQL(
                    "CREATE INDEX IF NOT EXISTS index_review_attempt_events_answeredAt " +
                        "ON review_attempt_events (answeredAt)",
                )
                db.execSQL(
                    """
                    CREATE TABLE IF NOT EXISTS study_sessions (
                        id TEXT NOT NULL PRIMARY KEY,
                        startedAt TEXT NOT NULL,
                        endedAt TEXT,
                        activeMillis INTEGER NOT NULL,
                        answeredCount INTEGER NOT NULL,
                        correctCount INTEGER NOT NULL
                    )
                    """.trimIndent(),
                )
                db.execSQL(
                    "CREATE INDEX IF NOT EXISTS index_study_sessions_startedAt " +
                        "ON study_sessions (startedAt)",
                )
            }
        }
    }
}
