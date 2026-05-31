export const appErrorCodes = [
  "ShortcutRegistrationFailed",
  "SelectionUnavailable",
  "SelectionEmpty",
  "SelectionPermissionDenied",
  "ProviderNotConfigured",
  "ProviderRequestFailed",
  "InvalidModelOutput",
  "CredentialStorageFailed",
  "SettingsIoFailed",
  "VocabularyStoreFailed",
  "SyncAuthRequired",
  "SyncPushFailed",
  "SyncPullFailed",
] as const;

export type AppErrorCode = (typeof appErrorCodes)[number];

export interface AppError {
  code: AppErrorCode;
  userMessage: string;
  diagnosticMessage: string;
  retryable: boolean;
}

export function isAppError(value: unknown): value is AppError {
  if (!isRecord(value)) {
    return false;
  }

  return (
    typeof value.code === "string" &&
    appErrorCodes.includes(value.code as AppErrorCode) &&
    typeof value.userMessage === "string" &&
    typeof value.diagnosticMessage === "string" &&
    typeof value.retryable === "boolean"
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
