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

diesel::table! {
    tenant_counter (tenant_id, name) {
        tenant_id -> Uuid,
        name -> Text,
        value -> Int8,
    }
}

diesel::table! {
    akt_type (tenant_id, key) {
        tenant_id -> Uuid,
        key -> Text,
        latest_version -> Int4,
    }
}

diesel::table! {
    akt_type_version (id) {
        id -> Uuid,
        tenant_id -> Uuid,
        key -> Text,
        version -> Int4,
        definition -> Jsonb,
        created_by -> Uuid,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    akt (id) {
        id -> Uuid,
        tenant_id -> Uuid,
        number -> Text,
        type_version_id -> Uuid,
        state -> Text,
        fields -> Jsonb,
        revision -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        rank -> Text,
        title -> Nullable<Text>,
    }
}

diesel::table! {
    akt_revision (akt_id, revision) {
        akt_id -> Uuid,
        revision -> Int4,
        tenant_id -> Uuid,
        author -> Uuid,
        transition -> Nullable<Text>,
        reason -> Nullable<Text>,
        changes -> Jsonb,
        state -> Text,
        hash -> Bytea,
        created_at -> Timestamptz,
        membership -> Nullable<Jsonb>,
    }
}

diesel::table! {
    change_event (tenant_id, seq) {
        tenant_id -> Uuid,
        seq -> Int8,
        akt_id -> Uuid,
        revision -> Nullable<Int4>,
        created_at -> Timestamptz,
        published_at -> Nullable<Timestamptz>,
        kind -> Text,
    }
}

diesel::table! {
    akt_membership (container_id, member_id) {
        tenant_id -> Uuid,
        container_id -> Uuid,
        member_id -> Uuid,
        added_at -> Timestamptz,
        added_by -> Uuid,
    }
}

diesel::table! {
    akt_link (source_id, kind, target_id) {
        tenant_id -> Uuid,
        source_id -> Uuid,
        kind -> Text,
        target_id -> Uuid,
        pinned_revision -> Nullable<Int4>,
        added_at -> Timestamptz,
        added_by -> Uuid,
    }
}

diesel::table! {
    contact (id) {
        id -> Uuid,
        tenant_id -> Uuid,
        name -> Nullable<Text>,
        email -> Nullable<Text>,
        linked_subject -> Nullable<Uuid>,
        linked_at -> Nullable<Timestamptz>,
        linked_by -> Nullable<Uuid>,
        created_by -> Uuid,
        created_at -> Timestamptz,
    }
}

diesel::joinable!(akt -> akt_type_version (type_version_id));
diesel::joinable!(akt_revision -> akt (akt_id));
diesel::table! {
    akt_origin (akt_id) {
        tenant_id -> Uuid,
        akt_id -> Uuid,
        origin_id -> Uuid,
    }
}

diesel::joinable!(akt_origin -> akt (origin_id));

diesel::table! {
    akt_commit (akt_id, revision) {
        tenant_id -> Uuid,
        akt_id -> Uuid,
        revision -> Int4,
        commit -> Jsonb,
    }
}

diesel::table! {
    akt_version (akt_id, number) {
        tenant_id -> Uuid,
        akt_id -> Uuid,
        number -> Int4,
        revision -> Int4,
        label -> Nullable<Text>,
        released_by -> Uuid,
        released_at -> Timestamptz,
        pins -> Jsonb,
    }
}

diesel::table! {
    akt_diagram (akt_id, field, source_sha256) {
        tenant_id -> Uuid,
        akt_id -> Uuid,
        field -> Text,
        source_sha256 -> Bytea,
        attachment_id -> Uuid,
    }
}

diesel::table! {
    git_ref (tenant_id, akt_id, name) {
        tenant_id -> Uuid,
        akt_id -> Uuid,
        name -> Text,
        target -> Text,
    }
}

diesel::table! {
    akt_held (akt_id, attachment_id) {
        tenant_id -> Uuid,
        akt_id -> Uuid,
        attachment_id -> Uuid,
    }
}

diesel::table! {
    akt_draft (akt_id) {
        tenant_id -> Uuid,
        akt_id -> Uuid,
        state -> Bytea,
        fields -> Jsonb,
        compacted_seq -> Int8,
        released_seq -> Int8,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    akt_draft_update (akt_id, seq) {
        tenant_id -> Uuid,
        akt_id -> Uuid,
        seq -> Int8,
        update -> Bytea,
        by -> Uuid,
        at -> Timestamptz,
        fields -> Jsonb,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    akt,
    akt_diagram,
    akt_draft,
    akt_draft_update,
    git_ref,
    akt_held,
    akt_origin,
    akt_commit,
    akt_version,
    akt_type,
    akt_type_version,
    akt_revision,
    akt_membership,
    akt_link,
    change_event,
    akt_attachment,
    attachment
);

diesel::table! {
    person (subject_id) {
        subject_id -> Uuid,
        name -> Nullable<Text>,
        email -> Nullable<Text>,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    tenant_person (tenant_id, subject_id) {
        tenant_id -> Uuid,
        subject_id -> Uuid,
        first_seen_at -> Timestamptz,
        last_seen_at -> Timestamptz,
    }
}

diesel::table! {
    attachment (id) {
        id -> Uuid,
        tenant_id -> Uuid,
        name -> Text,
        media_type -> Text,
        size -> Int8,
        sha256 -> Bytea,
        uploaded_by -> Uuid,
        uploaded_at -> Timestamptz,
        sbom -> Nullable<Jsonb>,
    }
}

diesel::table! {
    akt_attachment (akt_id, field) {
        tenant_id -> Uuid,
        akt_id -> Uuid,
        field -> Text,
        attachment_id -> Uuid,
    }
}

diesel::allow_tables_to_appear_in_same_query!(person, tenant_person);

diesel::table! {
    subject_alias (tenant_id, subject_id) {
        tenant_id -> Uuid,
        subject_id -> Uuid,
        alias_of -> Uuid,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    bundle_import (id) {
        id -> Uuid,
        tenant_id -> Uuid,
        imported_by -> Uuid,
        imported_at -> Timestamptz,
        scope -> Text,
        akt_types -> Int4,
        akter -> Int4,
    }
}
