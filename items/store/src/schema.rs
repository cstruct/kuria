// The tables the migrations create. Keep in step with `migrations/`.

diesel::table! {
    login_attempt (state) {
        state -> Text,
        code_verifier -> Text,
        return_to -> Text,
        created_at -> Timestamptz,
        expires_at -> Timestamptz,
    }
}

diesel::table! {
    browser_session (id_hash) {
        id_hash -> Bytea,
        subject_id -> Uuid,
        tenant_id -> Uuid,
        access_token -> Bytea,
        refresh_token -> Bytea,
        access_expires_at -> Timestamptz,
        created_at -> Timestamptz,
        refreshed_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    erasure_watermark (id) {
        id -> Int2,
        handled_until -> Int8,
        updated_at -> Timestamptz,
    }
}
