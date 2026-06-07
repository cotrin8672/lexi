# Lexi Review Mobile

Android-only Jetpack Compose starter for the Lexi vocabulary review app.

This project is intentionally small:

- single Android application module under `app`;
- Kotlin + Jetpack Compose + Material 3;
- no Kotlin Multiplatform module yet;
- Room-backed local vocabulary/stat storage;
- read-only Supabase Auth/PostgREST boundaries, with no mobile mutation push;
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

```properties
LEXI_SUPABASE_URL=https://your-project.supabase.co
SUPABASE_PUBLISHABLE_KEY=your-publishable-key
```

`LEXI_SUPABASE_PUBLISHABLE_KEY`, `LEXI_SUPABASE_ANON_KEY`, and
`SUPABASE_ANON_KEY` are accepted as compatibility aliases.

## Implemented

- Pure Kotlin review extraction, stable question keys, option generation,
  weighting, and stats updates.
- Room entities/DAO/database for cached vocabulary and `question_stats`.
- ViewModel-driven Compose review session UI with mode select, word list, and
  compact vocabulary source/count status during review.
- Cached-first vocabulary loading with honest Supabase refresh errors.
- Persisted question stats hydrated into session weighting on start.
- Google sign-in through Supabase PKCE and read-only PostgREST refresh.

## Next Steps

1. Add refresh-token rotation when a stored access token expires.
2. Optional background refresh after cache hit when online.
