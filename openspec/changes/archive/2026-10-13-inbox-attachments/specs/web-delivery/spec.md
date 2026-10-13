## MODIFIED Requirements

### Requirement: Security headers
Every response, API or web client, SHALL carry
`X-Content-Type-Options: nosniff`,
`Referrer-Policy: strict-origin-when-cross-origin` and
`X-Frame-Options: DENY`. Every HTML response from the web client SHALL carry a
Content-Security-Policy that allows scripts, style sheets and connections only
from the same origin, inline `style` attributes, images from the same origin,
`data:` URLs, `blob:` URLs (an attachment's preview, fetched through the
API) or `https` (for the remote images of an email the member chose to
show, inside its sandboxed frame), no plugins, no `<base>` element, no framing
by any origin, and form submissions only to the same origin.

#### Scenario: API response
- **WHEN** a client requests `/api/v1/system/version`
- **THEN** the response carries the three security headers

#### Scenario: HTML response
- **WHEN** a client requests `/`
- **THEN** the response carries the three security headers and a Content-Security-Policy with `default-src 'self'`, `script-src 'self'`, `style-src 'self'`, `style-src-attr 'unsafe-inline'`, `img-src 'self' data: blob: https:`, `object-src 'none'` and `frame-ancestors 'none'`
