package io.github.cotrin8672.lexi.review.storage.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import io.github.cotrin8672.lexi.review.storage.entity.CachedCardSnapshotEntity
import io.github.cotrin8672.lexi.review.storage.entity.CachedLexemeFormEntity
import io.github.cotrin8672.lexi.review.storage.entity.CachedUserLexemeEntity

@Dao
interface VocabularyCacheDao {
    @Query("SELECT * FROM cached_user_lexemes WHERE userId = :userId")
    suspend fun getLexemes(userId: String): List<CachedUserLexemeEntity>

    @Query("SELECT * FROM cached_card_snapshots WHERE userId = :userId AND active = 1")
    suspend fun getActiveSnapshots(userId: String): List<CachedCardSnapshotEntity>

    @Query("SELECT * FROM cached_lexeme_forms WHERE userId = :userId")
    suspend fun getForms(userId: String): List<CachedLexemeFormEntity>

    @Query(
        """
        SELECT EXISTS(
            SELECT 1 FROM cached_card_snapshots
            WHERE userId = :userId
              AND remoteOperationId = :operationId
              AND remoteServerRevision = :serverRevision
        )
        """,
    )
    suspend fun hasAppliedPullChange(
        userId: String,
        operationId: String,
        serverRevision: Long,
    ): Boolean

    @Query(
        """
        UPDATE cached_card_snapshots
        SET active = 0
        WHERE userId = :userId AND lexemeId = :lexemeId AND resultLanguage = :resultLanguage
        """,
    )
    suspend fun deactivateSnapshots(
        userId: String,
        lexemeId: String,
        resultLanguage: String,
    )

    @Query(
        """
        SELECT id FROM cached_user_lexemes
        WHERE userId = :userId AND language = :language AND canonicalKey = :canonicalKey
        LIMIT 1
        """,
    )
    suspend fun findLexemeId(
        userId: String,
        language: String,
        canonicalKey: String,
    ): String?

    @Query(
        """
        SELECT * FROM cached_user_lexemes
        WHERE userId = :userId AND id = :lexemeId
        LIMIT 1
        """,
    )
    suspend fun findLexemeById(
        userId: String,
        lexemeId: String,
    ): CachedUserLexemeEntity?

    @Query(
        """
        SELECT * FROM cached_user_lexemes
        WHERE userId = :userId AND language = :language AND canonicalKey = :canonicalKey
        LIMIT 1
        """,
    )
    suspend fun findLexemeByKey(
        userId: String,
        language: String,
        canonicalKey: String,
    ): CachedUserLexemeEntity?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertLexemes(rows: List<CachedUserLexemeEntity>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertLexeme(row: CachedUserLexemeEntity)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertSnapshot(row: CachedCardSnapshotEntity)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertForm(row: CachedLexemeFormEntity)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertSnapshots(rows: List<CachedCardSnapshotEntity>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertForms(rows: List<CachedLexemeFormEntity>)

    @Query("DELETE FROM cached_user_lexemes WHERE userId = :userId")
    suspend fun deleteLexemes(userId: String)

    @Query("DELETE FROM cached_card_snapshots WHERE userId = :userId")
    suspend fun deleteSnapshots(userId: String)

    @Query("DELETE FROM cached_lexeme_forms WHERE userId = :userId")
    suspend fun deleteForms(userId: String)

    @Transaction
    suspend fun replaceUserCache(
        userId: String,
        lexemes: List<CachedUserLexemeEntity>,
        snapshots: List<CachedCardSnapshotEntity>,
        forms: List<CachedLexemeFormEntity>,
    ) {
        deleteLexemes(userId)
        deleteSnapshots(userId)
        deleteForms(userId)
        if (lexemes.isNotEmpty()) {
            upsertLexemes(lexemes)
        }
        if (snapshots.isNotEmpty()) {
            upsertSnapshots(snapshots)
        }
        if (forms.isNotEmpty()) {
            upsertForms(forms)
        }
    }
}
