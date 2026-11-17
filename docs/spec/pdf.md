# PDFs

A `markdown` field can be read as a PDF: an akt's living version, or one
of its versions. A PDF holds the field's body only, with no title page.

r[pdf.render]
A PDF of a `markdown` field MUST hold its value rendered: headings,
paragraphs, emphasis, lists, task lists, quotes, code, tables,
horizontal rules and footnotes, in the field's order, each footnote at
the foot of the page that first refers to it, numbered in that order. Each heading MUST be a bookmark,
nested by level. A reference MUST be shown by its text, as a link to the
akt in the app (`app.references`). An embedded image MUST be drawn as its
picture, as the PDF's version reads it, and a broken one as its alt text.
A diagram block MUST be drawn from its source, as a vector drawing. A page
break MUST start a new page. The PDF of an akt with a live draft MUST be of
its live draft, unless a version is asked for.

r[render.limits]
Kuria MUST render each PDF and each diagram in a process of its own, which
it stops past a wall time, and which can use at most a CPU time and, where
the system lets a process be limited so, an amount of memory: by default
30 s, 20 s and 1 GiB, set by `KURIA_RENDER_SECONDS`,
`KURIA_RENDER_CPU_SECONDS` and `KURIA_RENDER_MEMORY_MB`. A diagram stopped
by a limit MUST refuse its write as `INVALID_DIAGRAM`, saying which
limit; a PDF stopped by one MUST be answered with 500.

r[pdf.packages]
typst MUST be run with Kuria's bundled fonts and packages only: fletcher
0.5.8, cetz 0.3.4 and 0.5.2, cetz-plot 0.1.4, and oxifmt 0.2.1 and
1.0.0, which cetz imports, vendored unmodified with their licenses. It MUST NOT read any other package, the network or
the file system; the only files it reads are the pictures Kuria gives
it.

r[pdf.route]
`GET /akter/{number}/pdf?field=<key>` MUST answer the PDF of the akt's
`markdown` field `key`, and with `&version=<n>` of that field at the
akt's version `n` (`version.read`), to be shown where it opens, or with
`&download` to be saved. It MUST be authenticated as `/graphql`
is, and answer 404 for an akt the subject may not read, as if it did not
exist, and for a field that isn't `markdown` or a version the akt doesn't
have. For an akt whose type declares `versions.file`
(`version.file`), it MUST answer instead the PDF the version asked for was
released with, if it was; and for one whose type has no `markdown` fields,
the latest version's PDF when none is asked for, and 404 before its first
release.

r[pdf.repeatable]
A version's PDF MUST be the same each time it is rendered by the same
Kuria: it is made from the version, which never changes
(`version.immutable`), and its pins, never from a target as it is now.
