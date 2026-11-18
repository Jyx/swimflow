'use strict';
// Extract the body of a raw HTTP response.
//
// The server tests speak HTTP/1.1 by hand, so the response they capture
// still contains the status line and the headers. The body is either
// `Content-Length` delimited or chunked, depending on what the server
// decided to send; both are handled here so the harnesses always see the
// document as one contiguous string.

function bodyOf(response) {
  const text = typeof response === 'string' ? response : String(response);
  const separator = text.indexOf('\r\n\r\n');
  if (separator < 0) return text;
  const head = text.slice(0, separator);
  const body = text.slice(separator + 4);

  if (!/transfer-encoding:\s*chunked/i.test(head)) return body;

  // Chunked: "<size>\r\n<data>\r\n" repeated, ended by "0\r\n\r\n".
  const chunks = [];
  let rest = body;
  for (;;) {
    const lineEnd = rest.indexOf('\r\n');
    if (lineEnd < 0) break;
    const size = parseInt(rest.slice(0, lineEnd).trim(), 16);
    if (!isFinite(size) || size === 0) break;
    chunks.push(rest.slice(lineEnd + 2, lineEnd + 2 + size));
    rest = rest.slice(lineEnd + 2 + size + 2);
  }
  return chunks.join('');
}

module.exports = { bodyOf };
