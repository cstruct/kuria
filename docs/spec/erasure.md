# Erasure

When Konklav erases a subject, Kuria deletes what it holds about that
subject. Konklav says so through the relying SDK, with a live notice for
each erasure and a durable list of erased subjects that can be read from any
time on.

r[erasure.browser-sessions]
When Konklav erases a subject, Kuria MUST delete every browser session of
that subject.

r[erasure.catch-up]
Notices can be missed, such as while Kuria is not running. Kuria MUST keep in
its database the time it has handled erasures up to, and whenever the
relying SDK says erasures may have been missed, which includes every start,
it MUST read the erased subjects from that time, delete what it holds about
them, and only then move the stored time forward, to the latest erasure it
read. Handling a live notice MUST NOT move the stored time.

r[erasure.retry]
When the list of erased subjects cannot be read, Kuria MUST keep trying
until it succeeds, and MUST NOT move the stored time until then.
