# Login page

Konklav has no user interface: its authorization endpoint hands the browser
to a login application, which signs the person in through Konklav's
subject-facing API and accepts the login request. Kuria's login page is
that application. Kuria's own sign-in ([browser sessions](browser-sessions.md))
then completes as it would with any OpenID provider.

## The proxy

r[login.proxy]
When configured as Konklav's login application, the server MUST pass
requests under `/v1/` and `/.well-known/` to Konklav's subject-facing
surface and answer with Konklav's responses, cookies and redirects
included, without following redirects. Konklav's issuer is then the
server's public URL, and its login URL `<public URL>/login`.

r[login.discovery]
The server MUST discover the OpenID provider from Konklav's subject-facing
surface directly, not through its own proxy, and MUST still refuse a
provider whose discovered issuer is not the configured issuer.

## Signing in

r[login.request]
`/login?login_request=<id>` MUST show which client asked to sign the
person in, and sign them in for it. Opened without a login request, or with
one that has expired, it MUST say so rather than sign anyone in.

r[login.password]
The page MUST sign in with an email and a password through Konklav's
`/v1/login/password` as the `browser` client. A wrong email or password
MUST be shown as such, without saying which was wrong. An account that
needs a second factor MUST be told that this page can't sign it in yet.

r[login.dpop]
The page's Konklav session MUST be bound to a DPoP key the page creates as
a non-extractable WebCrypto key and keeps in IndexedDB, and every request
to Konklav MUST carry a proof. The page MUST NOT write tokens or keys to
`localStorage` or `sessionStorage`; its access token lives in memory only,
and its refresh token is Konklav's `HttpOnly` cookie.

r[login.single-sign-on]
When a Konklav session can be restored from its cookie, and the request
doesn't ask for `prompt=login`, the page MUST accept the request without
asking the person anything.

r[login.prompt-none]
With `prompt=none` and nobody signed in, or a session that can't satisfy
the request, the page MUST refuse the request with `login_required`
without showing a form.

r[login.tenant]
When the login request names a tenant, the page MUST move its session into
that tenant (`/v1/login/tenant`) before accepting, and MUST say so if the
person has no membership there.

r[login.accept]
After signing in, the page MUST accept the login request and send the
browser where Konklav answers, and MUST refuse it with `access_denied` when
the person cancels.

r[login.csp]
The page MUST be served with a Content Security Policy that allows scripts,
styles and connections from Kuria's own origin only, and forbids framing.
