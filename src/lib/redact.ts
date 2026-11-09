// Pure redaction for error strings shown or copied from FatalState.
// Query secrets become key=[redacted]. Authorization and Cookie header
// values become Header: [redacted]. Anything else passes through.

const QUERY_RE =
  /([?&](?:token|access_token|access-token|refresh_token|refresh-token|id_token|id-token|auth|api_key|api-key|key|sig|signature|password|secret|client_secret|client-secret)=)([^&\s"'<>`]+)/gi;

// A header value is redacted to the end of its line, so inline auth tokens
// and multi pair Cookie values cannot leak past a space or semicolon.
const HEADER_RE =
  /(\b(?:authorization|cookie|set-cookie)\b["']?\s*[:=]\s*)[^\n]*/gi;

export function redactSecrets(input: string): string {
  if (!input) return input;
  return input
    .replace(QUERY_RE, "$1[redacted]")
    .replace(HEADER_RE, "$1[redacted]");
}
