package io.github.cotrin8672.lexi.review

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import io.github.cotrin8672.lexi.review.ui.ReviewSessionScreen
import io.github.cotrin8672.lexi.review.ui.theme.LexiReviewTheme

class MainActivity : ComponentActivity() {
    private lateinit var dependencies: AppDependencies

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        dependencies = (application as LexiReviewApp).dependencies
        setContent {
            LexiReviewTheme {
                ReviewSessionScreen(
                    dependencies = dependencies,
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }
    }
}
