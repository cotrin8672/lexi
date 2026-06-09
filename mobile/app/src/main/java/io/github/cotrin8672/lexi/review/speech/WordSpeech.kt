package io.github.cotrin8672.lexi.review.speech

import android.content.Context
import android.media.AudioAttributes
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
    private val pending = ArrayDeque<String>()
    private val lock = Any()

    init {
        tts = TextToSpeech(context.applicationContext) { status ->
            synchronized(lock) {
                if (status != TextToSpeech.SUCCESS) {
                    tts?.shutdown()
                    tts = null
                    pending.clear()
                    return@synchronized
                }
                val engine = tts ?: return@synchronized
                engine.setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_MEDIA)
                        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                        .build(),
                )
                configureLanguage(engine)
                ready = true
                flushPendingLocked()
            }
        }
    }

    override fun speak(text: String) {
        val trimmed = text.trim()
        if (trimmed.isEmpty()) {
            return
        }
        synchronized(lock) {
            if (!ready) {
                pending.addLast(trimmed)
                return
            }
            speakNow(trimmed, TextToSpeech.QUEUE_FLUSH)
        }
    }

    fun shutdown() {
        synchronized(lock) {
            tts?.shutdown()
            tts = null
            ready = false
            pending.clear()
        }
    }

    private fun flushPendingLocked() {
        if (pending.isEmpty()) {
            return
        }
        pending.forEachIndexed { index, text ->
            speakNow(
                text = text,
                queueMode = if (index == 0) {
                    TextToSpeech.QUEUE_FLUSH
                } else {
                    TextToSpeech.QUEUE_ADD
                },
            )
        }
        pending.clear()
    }

    private fun speakNow(text: String, queueMode: Int) {
        tts?.speak(
            text,
            queueMode,
            null,
            "lexi-review-${text.hashCode()}",
        )
    }

    private fun configureLanguage(engine: TextToSpeech) {
        when (engine.setLanguage(Locale.US)) {
            TextToSpeech.LANG_MISSING_DATA,
            TextToSpeech.LANG_NOT_SUPPORTED,
            -> engine.setLanguage(Locale.ENGLISH)
        }
    }
}
