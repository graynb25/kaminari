// Pure redaction for error strings shown or copied from FatalState.
// Query secrets become key=[redacted]. Authorization and Cookie header
// values become Header: [redacted]. Anything else passes through.

const QUERY_RE =
  /([?&](?:token|access_token|access-token|auth|api_key|api-key|key|sig|signature)=)([^&\s"'<>`]+)/gi;

// A full header line is redacted to end of line, so multi pair Cookie
// values cannot leak past a semicolon.
const HEADER_LINE_RE =
  /^(\s*[{[]?\s*["']?(?:authorization|cookie)["']?\s*[:=]\s*["']?).*?(["']?\s*,?\s*[}\]]?\s*)$/gim;

// Inline header mid line, such as "failed with Authorization: Bearer x".
const HEADER_INLINE_RE =
  /(\b(?:authorization|cookie)\b["']?\s*[:=]\s*["']?)([^\s,;"'}]+)/gi;

export function redactSecrets(input: string): string {
  if (!input) return input;
  return input
    .replace(QUERY_RE, "$1[redacted]")
    .replace(HEADER_LINE_RE, "$1[redacted]$2")
    .replace(HEADER_INLINE_RE, "$1[redacted]");
}
