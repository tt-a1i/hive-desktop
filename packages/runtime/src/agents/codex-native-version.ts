/** Codex 0.169 initializes its user agent with our clientInfo.name, followed by
 * the native CLI version. The trailing `(hive_desktop; 0.1.0)` is the client
 * version and must never be used as evidence of the native protocol version. */
export function codexNativeVersion(initialization: unknown): string | undefined {
  if (!initialization || typeof initialization !== 'object' || !('userAgent' in initialization)
    || typeof initialization.userAgent !== 'string') return;
  return /^hive_desktop\/(\d+\.\d+\.\d+)(?:\s|$)/.exec(initialization.userAgent)?.[1];
}
export function supportsCodexDeferredContinuation(initialization: unknown): boolean {
  return codexNativeVersion(initialization) === '0.169.0';
}
