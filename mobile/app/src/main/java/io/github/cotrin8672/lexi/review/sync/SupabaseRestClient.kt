package io.github.cotrin8672.lexi.review.sync

import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

internal class SupabaseRestClient(
    private val config: SupabaseMobileConfig,
    private val json: Json = Json {
        ignoreUnknownKeys = true
        isLenient = true
    },
) {
    suspend inline fun <reified T> getAllRows(
        accessToken: String,
        table: String,
        select: String,
        filters: List<Pair<String, String>> = emptyList(),
        pageSize: Int = BOOTSTRAP_PAGE_SIZE,
    ): List<T> {
        val rows = mutableListOf<T>()
        var offset = 0
        while (true) {
            val query = buildString {
                append("select=$select")
                append("&order=id.asc")
                append("&limit=$pageSize")
                append("&offset=$offset")
                filters.forEach { (key, value) ->
                    append("&$key=${encode(value)}")
                }
            }
            val page = get<List<T>>(
                path = "rest/v1/$table",
                query = query,
                accessToken = accessToken,
            )
            rows += page
            if (page.size < pageSize) {
                break
            }
            offset += pageSize
        }
        return rows
    }

    suspend fun fetchMaxServerRevision(accessToken: String): Long {
        val rows = get<List<MaxRevisionRow>>(
            path = "rest/v1/vocabulary_changes",
            query = "select=server_revision&order=server_revision.desc&limit=1",
            accessToken = accessToken,
        )
        return rows.firstOrNull()?.serverRevision ?: 0L
    }

    suspend fun pullVocabularyChanges(
        accessToken: String,
        sinceRevision: Long,
        batchLimit: Int = PULL_BATCH_LIMIT,
    ): PullResponse {
        val body = buildJsonObject {
            put("since_revision", sinceRevision)
            put("batch_limit", batchLimit)
        }
        return post("rest/v1/rpc/pull_vocabulary_changes", accessToken, body)
    }

    suspend inline fun <reified T> get(
        path: String,
        query: String,
        accessToken: String,
    ): T = request(
        method = "GET",
        path = path,
        query = query,
        accessToken = accessToken,
    )

    suspend inline fun <reified T> post(
        path: String,
        accessToken: String,
        body: JsonObject,
    ): T = request(
        method = "POST",
        path = path,
        query = null,
        accessToken = accessToken,
        body = json.encodeToString(body),
    )

    private inline fun <reified T> request(
        method: String,
        path: String,
        query: String?,
        accessToken: String,
        body: String? = null,
    ): T {
        val base = config.url.trimEnd('/')
        val url = if (query.isNullOrBlank()) {
            "$base/$path"
        } else {
            "$base/$path?$query"
        }
        val connection = URL(url).openConnection() as HttpURLConnection
        connection.requestMethod = method
        connection.setRequestProperty("apikey", config.publishableKey)
        connection.setRequestProperty("Authorization", "Bearer $accessToken")
        connection.setRequestProperty("Accept", "application/json")
        connection.connectTimeout = 15_000
        connection.readTimeout = 20_000
        if (body != null) {
            connection.doOutput = true
            connection.setRequestProperty("Content-Type", "application/json")
            connection.outputStream.bufferedWriter().use { it.write(body) }
        }
        val status = connection.responseCode
        val responseBody = if (status in 200..299) {
            connection.inputStream.bufferedReader().use { it.readText() }
        } else {
            connection.errorStream?.bufferedReader()?.use { it.readText() }.orEmpty()
        }
        if (status !in 200..299) {
            val detail = responseBody.trim().take(240).ifBlank { "no response body" }
            error("Supabase request failed with HTTP $status: $detail")
        }
        return json.decodeFromString(responseBody)
    }

    private fun encode(value: String): String =
        URLEncoder.encode(value, Charsets.UTF_8.name())

    companion object {
        const val BOOTSTRAP_PAGE_SIZE = 500
        const val PULL_BATCH_LIMIT = 100
    }
}
