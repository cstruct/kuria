-- What Kuria knows about a subject to show them: the name and email from
-- their last browser sign-in. Konklav keeps no profile data.
create table person (
    subject_id uuid primary key,
    name text,
    email text,
    updated_at timestamptz not null
);
