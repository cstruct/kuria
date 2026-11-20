# People

Konklav keeps no profile data, so Kuria learns people's names when they sign
in, and shows them wherever a `user` field or a revision names a subject.

r[person.from-sign-in]
A browser sign-in MUST ask Konklav for the `profile` and `email` scopes, and
MUST record the subject's name and email from the claims it receives, or
update them if they changed. A claim it doesn't receive, such as the name
Konklav keeps none of, MUST leave what was recorded, such as the name a
bundle's people gave (`bundle.import`).

r[person.tenant]
A browser sign-in MUST record that the subject signed in to the tenant, so
that a tenant's users can be listed.

r[person.list]
`users` MUST list the users who signed in to the caller's tenant, by
`person.name`, to a subject with `kuria:contacts:read` or
`kuria:contacts:manage`, and be refused as `FORBIDDEN` to anyone else.

r[person.name]
A user MUST be shown by the name Kuria recorded, else by the recorded email,
else by the subject identifier, so that a subject who never signed in
through a browser, such as a service principal, is still shown.

r[person.erasure]
When Konklav erases a subject, Kuria MUST delete the name and email it
recorded for them and forget the tenants they signed in to
(`erasure.browser-sessions` deletes their sessions the same way).
