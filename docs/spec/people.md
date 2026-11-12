# People

Konklav keeps no profile data, so Kuria learns people's names when they sign
in, and shows them wherever a `user` field or a revision names a subject.

r[person.from-sign-in]
A browser sign-in MUST ask Konklav for the `profile` and `email` scopes, and
MUST record the subject's name and email from the claims it receives, or
update them if they changed.

r[person.name]
A user MUST be shown by the name Kuria recorded, else by the recorded email,
else by the subject identifier, so that a subject who never signed in
through a browser, such as a service principal, is still shown.

r[person.erasure]
When Konklav erases a subject, Kuria MUST delete the name and email it
recorded for them (`erasure.browser-sessions` deletes their sessions the same
way).
