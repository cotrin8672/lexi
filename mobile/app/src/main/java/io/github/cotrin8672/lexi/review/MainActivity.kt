package io.github.cotrin8672.lexi.review

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.lifecycle.lifecycleScope
import io.github.cotrin8672.lexi.review.ui.ReviewSessionScreen
import io.github.cotrin8672.lexi.review.ui.theme.LexiReviewTheme
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private lateinit var dependencies: AppDependencies

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        dependencies = (application as LexiReviewApp).dependencies
        handleAuthIntent(intent)
        setContent {
            LexiReviewTheme {
                ReviewSessionScreen(
                    dependencies = dependencies,
                    onSignIn = ::startGoogleSignIn,
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleAuthIntent(intent)
    }

    private fun startGoogleSignIn() {
        lifecycleScope.launch {
            runCatching {
                dependencies.signInWithGoogle()
            }.onFailure { error ->
                Toast.makeText(
                    this@MainActivity,
                    error.message ?: "Sign-in failed.",
                    Toast.LENGTH_LONG,
                ).show()
            }
        }
    }

    private fun handleAuthIntent(intent: Intent?) {
        intent ?: return
        runCatching {
            dependencies.sessionStore?.handleDeeplink(intent)
        }.onFailure { error ->
            Toast.makeText(
                this,
                error.message ?: "Sign-in callback failed.",
                Toast.LENGTH_LONG,
            ).show()
        }
    }
}
