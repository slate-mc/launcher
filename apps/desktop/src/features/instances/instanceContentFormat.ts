export function formatContentFileSize(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

export function contentErrorMessage(error: unknown) {
  return getUserFacingError(
    error,
    "slate could not complete that content request. Try again.",
  );
}
import { getUserFacingError } from "../../lib/userFacingError";
