import { errorNumber, localizeError } from '../language/errorMessages';

/** Stable editor/transport errors; native compiler and VM codes pass through. */
export class CodedError extends Error {
  readonly errorNumber: string;
  constructor(readonly code: string, readonly originalMessage: string) {
    super(localizeError(code, originalMessage));
    this.errorNumber = errorNumber(code);
  }
}

export function codedError(error: unknown, fallback = 'editor.operation_failed'): CodedError {
  if (error instanceof CodedError) return error;
  return new CodedError(fallback, error instanceof Error ? error.message : String(error));
}
