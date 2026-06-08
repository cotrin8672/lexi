package io.github.cotrin8672.lexi.review.speech

import android.content.Context
import android.speech.tts.TextToSpeech
import java.util.Locale

interface WordSpeech {
    fun speak(text: String)
}

object NoOpWordSpeech : WordSpeech {
    override fun speak(text: String) = Unit
}

class AndroidWordSpeech(context: Context) : WordSpeech {
    private var tts: TextToSpeech? = null
    private var ready = false

    init {
        tts = TextToSpeech(context.applicationContext) { status ->
            if (status == TextToSpeech.SUCCESS) {
                tts?.language = Locale.US
                ready = true
            }
        }
    }

    override fun speak(text: String) {
        val trimmed = text.trim()
        if (trimmed.isEmpty() || !ready) {
            return
        }
        tts?.speak(
            trimmed,
            TextToSpeech.QUEUE_FLUSH,
            null,
            "lexi-review-${trimmed.hashCode()}",
        )
    }

    fun shutdown() {
        tts?.shutdown()
        tts = null
        ready = false
    }
}
