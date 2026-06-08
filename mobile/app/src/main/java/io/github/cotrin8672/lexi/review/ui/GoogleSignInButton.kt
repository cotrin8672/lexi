package io.github.cotrin8672.lexi.review.ui

import android.widget.Toast
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import io.github.cotrin8672.lexi.review.sync.SupabaseSessionStore
import io.github.jan.supabase.compose.auth.composeAuth
import io.github.jan.supabase.compose.auth.composable.GoogleDialogType
import io.github.jan.supabase.compose.auth.composable.NativeSignInResult
import io.github.jan.supabase.compose.auth.composable.SignInResultData
import io.github.jan.supabase.compose.auth.composable.rememberSignInWithGoogle

@Composable
fun GoogleSignInButton(
    sessionStore: SupabaseSessionStore?,
    modifier: Modifier = Modifier,
    onSignedIn: () -> Unit = {},
) {
    val context = LocalContext.current
    var signingIn by remember { mutableStateOf(false) }

    if (sessionStore == null || !sessionStore.nativeGoogleSignInEnabled) {
        OutlinedButton(
            modifier = modifier,
            enabled = false,
            onClick = {},
        ) {
            Text("Sign in with Google")
        }
        return
    }

    val signInAction = sessionStore.client.composeAuth.rememberSignInWithGoogle(
        type = GoogleDialogType.BOTTOM_SHEET,
        onResult = { result ->
            signingIn = false
            when (result) {
                is NativeSignInResult.Success -> {
                    val label = (result.data as SignInResultData.Google).credential.displayName
                        ?: sessionStore.readUserId()
                        ?: "Google account"
                    Toast.makeText(
                        context,
                        "Signed in as $label",
                        Toast.LENGTH_LONG,
                    ).show()
                    onSignedIn()
                }
                is NativeSignInResult.ClosedByUser -> {
                    Toast.makeText(
                        context,
                        "Google sign-in was cancelled.",
                        Toast.LENGTH_SHORT,
                    ).show()
                }
                is NativeSignInResult.Error -> {
                    Toast.makeText(
                        context,
                        "Google sign-in failed: ${result.message}",
                        Toast.LENGTH_LONG,
                    ).show()
                }
                is NativeSignInResult.NetworkError -> {
                    Toast.makeText(
                        context,
                        "Google sign-in failed: ${result.message}",
                        Toast.LENGTH_LONG,
                    ).show()
                }
            }
        },
    )

    OutlinedButton(
        modifier = modifier,
        enabled = !signingIn,
        onClick = {
            signingIn = true
            signInAction.startFlow()
        },
    ) {
        Text(if (signingIn) "Signing in..." else "Sign in with Google")
    }
}
