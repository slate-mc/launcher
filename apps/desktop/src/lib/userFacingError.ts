export class UserFacingError extends Error {
  override readonly name = "UserFacingError";
}

export function getUserFacingError(error: unknown, fallback: string): string {
  if (error instanceof UserFacingError && error.message.trim()) {
    return error.message;
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "userMessage" in error &&
    typeof error.userMessage === "string" &&
    error.userMessage.trim()
  ) {
    return error.userMessage;
  }
  return fallback;
}
