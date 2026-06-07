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
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

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
        val signInIntent = dependencies.createGoogleSignInIntent()
        if (signInIntent == null) {
            Toast.makeText(this, "Supabase is not configured.", Toast.LENGTH_SHORT).show()
            return
        }
        startActivity(signInIntent)
    }

    private fun handleAuthIntent(intent: Intent?) {
        val uri = intent?.data ?: return
        lifecycleScope.launch {
            val status = runCatching {
                withContext(Dispatchers.IO) {
                    dependencies.handleAuthCallback(uri)
                }
            }
            status.onSuccess { result ->
                if (result == AuthCallbackStatus.SignedIn) {
                    Toast.makeText(this@MainActivity, "Signed in.", Toast.LENGTH_SHORT).show()
                }
            }.onFailure { error ->
                Toast.makeText(
                    this@MainActivity,
                    error.message ?: "Sign-in failed.",
                    Toast.LENGTH_LONG,
                ).show()
            }
        }
    }
}
