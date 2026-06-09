package io.github.cotrin8672.lexi.review.sync

import org.junit.Assert.assertEquals
import org.junit.Test

class SyncErrorMessagesTest {
    @Test
    fun mapsUnauthorizedSyncFailuresToSignInPrompt() {
        val message = syncErrorUserMessage("Supabase request failed with HTTP 401: no response body")
        assertEquals(
            "Session expired. Sign in again to sync vocabulary.",
            message,
        )
    }
}
