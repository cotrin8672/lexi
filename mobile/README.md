# Lexi Review Mobile

Android-only Jetpack Compose starter for the Lexi vocabulary review app.

This project is intentionally small:

- single Android application module under `app`;
- Kotlin + Jetpack Compose + Material 3;
- no Kotlin Multiplatform module yet;
- Room-backed local vocabulary/stat storage;
- read-only Supabase Auth/PostgREST boundaries through supabase-kt, with no mobile mutation push;
- no generated-question persistence.

Normal launch loads cached vocabulary for the signed-in account, then refreshes
from Supabase when configured and authenticated. Fixture vocabulary remains
available for previews and unit tests only.

## Open in Android Studio

Open this `mobile` directory as the Android project root.

## Build

From this directory:

```powershell
.\gradlew.bat :app:assembleDebug
.\gradlew.bat :app:testDebugUnitTest
```

From the repository root:

```powershell
rtk mise run mobile-build
rtk mise run mobile-test
```

## Supabase configuration

Mobile builds read Supabase config from Gradle properties or environment
variables. The repository-level mise tasks load `.env.secret` and `.env` through
dotenvx before invoking Gradle.

Repository root `.env` example:

```properties
LEXI_SUPABASE_URL=https://your-project.supabase.co
SUPABASE_PUBLISHABLE_KEY=your-publishable-key
LEXI_GOOGLE_WEB_CLIENT_ID=your-google-web-client-id.apps.googleusercontent.com
```

`mise run mobile-build` loads `.env.secret` and `.env` through dotenvx, then
passes the values into Gradle as build properties.

`LEXI_SUPABASE_PUBLISHABLE_KEY`, `LEXI_SUPABASE_ANON_KEY`, and
`SUPABASE_ANON_KEY` are accepted as compatibility aliases for the publishable
key. `GOOGLE_WEB_CLIENT_ID`, `LEXI_GOOGLE_CLIENT_ID`, and `GOOGLE_CLIENT_ID`
are accepted aliases for the web OAuth client id.

Put the **Web application** client id in `.env`. The **Android** client id from
GCP goes only into Supabase Dashboard → Google → Authorized Client IDs.

## Native Google sign-in (GCP)

Mobile uses Android Credential Manager through supabase-kt Compose Auth.
Configure Google OAuth in the same GCP project used for desktop Supabase Auth:

1. Keep the existing **Web application** OAuth client (redirect:
   `https://<project-ref>.supabase.co/auth/v1/callback`) and use its client id
   as `LEXI_GOOGLE_WEB_CLIENT_ID`.
2. Create an **Android** OAuth client with package name
   `io.github.cotrin8672.lexi.review` and the debug/release SHA-1 from
   `.\gradlew.bat :app:signingReport`.
3. In Supabase Dashboard → Authentication → Google, add the Android client id
   to **Authorized Client IDs** (in addition to the existing web client).

No browser deeplink callback is required on Android for native sign-in.

## Implemented

- Pure Kotlin review extraction, stable question keys, option generation,
  weighting, and stats updates.
- Room entities/DAO/database for cached vocabulary, `question_stats`,
  `review_attempt_events`, and `study_sessions`.
- ViewModel-driven Compose review session UI with mode select, word list, and
  compact vocabulary source/count status during review.
- Cached-first vocabulary loading with honest Supabase refresh errors.
- Persisted question stats hydrated into session weighting on start.
- Native Google sign-in through supabase-kt Compose Auth (Credential Manager)
  and read-only PostgREST refresh.
- Local stats dashboard: today summary, streaks, 7-day charts, question-type
  breakdown, weak words, and vocabulary growth. Entry point is the mode-select
  `統計` button.
- Study-session tracking during review with foreground-time measurement, idle
  cap, and pause/resume lifecycle hooks.
- Vocabulary `created_at` preserved from Supabase bootstrap/pull for per-day new
  word counts.

## Next Steps

1. Optional background refresh after cache hit when online.
2. Optional cloud sync for review stats across devices.
