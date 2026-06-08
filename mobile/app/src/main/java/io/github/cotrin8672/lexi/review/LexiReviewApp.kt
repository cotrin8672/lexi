package io.github.cotrin8672.lexi.review

import android.app.Application

class LexiReviewApp : Application() {
    lateinit var dependencies: AppDependencies
        private set

    override fun onCreate() {
        super.onCreate()
        dependencies = AppDependencies.create(this)
        dependencies.warmUpVocabularyCache()
    }

    override fun onTerminate() {
        dependencies.shutdown()
        super.onTerminate()
    }
}
