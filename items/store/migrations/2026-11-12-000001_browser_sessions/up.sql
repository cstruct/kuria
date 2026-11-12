-- A sign-in a browser has started at Konklav and not completed yet.
create table login_attempt (
    -- The authorization request's `state`, also held by the browser's login
    -- cookie.
    state text primary key,
    code_verifier text not null,
    return_to text not null,
    created_at timestamptz not null default now(),
    expires_at timestamptz not null
);

create index login_attempt_expires_at on login_attempt (expires_at);

-- A browser's session: the tokens Konklav issued for it, encrypted.
create table browser_session (
    -- SHA-256 of the session cookie; the cookie itself is never stored.
    id_hash bytea primary key check (length(id_hash) = 32),
    subject_id uuid not null,
    tenant_id uuid not null,
    -- AES-256-GCM: a 12-byte nonce, then the ciphertext and tag.
    access_token bytea not null,
    refresh_token bytea not null,
    access_expires_at timestamptz not null,
    created_at timestamptz not null default now(),
    refreshed_at timestamptz
);

create index browser_session_subject on browser_session (subject_id);
