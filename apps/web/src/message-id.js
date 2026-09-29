// UI-only correlation key, never an authentication token. Works on HTTP origins too.
let sequence = 0
export function messageId(crypto = globalThis.crypto) {
  return typeof crypto?.randomUUID === 'function'
    ? crypto.randomUUID()
    : `pending-${Date.now()}-${++sequence}`
}
