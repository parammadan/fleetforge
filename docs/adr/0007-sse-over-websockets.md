# ADR-0007 — Server-Sent Events rather than WebSockets

**Status:** Accepted · 2026-09-12

## Context
The UI must reflect cluster changes as they happen, driven by real watches and never by a
frontend polling timer. Both SSE and WebSockets can carry that.

## Decision
SSE for server → browser streaming. Commands travel over ordinary POST endpoints.

## Consequences
- Automatic browser reconnection with `Last-Event-ID`, so resumption is built in rather than
  hand-rolled.
- A plain `GET` over HTTP, so authorization, proxies, and observability all work normally —
  worth a great deal when this eventually sits behind a corporate ingress.
- Commands get standard request/response semantics: status codes, audit logging, idempotency keys.
- Text/UTF-8 only, and browsers cap concurrent connections per origin over HTTP/1.1. Neither
  constrains this product; HTTP/2 removes the second anyway.
- If bidirectional low-latency interaction ever becomes necessary, this decision is revisited in
  a new ADR.

## Alternatives rejected
- **WebSockets.** More capable than needed; reconnection, auth, and proxy behaviour all become
  our problem.
- **Long polling.** Works everywhere, wastes connections, and adds latency for no benefit.
