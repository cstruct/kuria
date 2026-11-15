# PDFs

A `markdown` field can be read as a PDF: an akt's living version, or one
of its versions. A PDF holds the field's body only, with no title page.

r[pdf.render]
A PDF of a `markdown` field MUST hold its value rendered: headings,
paragraphs, emphasis, lists, task lists, quotes, code, tables,
horizontal rules and footnotes, in the field's order, each footnote at
the foot of the page that first refers to it, numbered in that order. Each heading MUST be a bookmark,
nested by level. A reference MUST be shown by its text, as a link to the
akt in the app (`app.references`).

r[pdf.route]
`GET /akter/{number}/pdf?field=<key>` MUST answer the PDF of the akt's
`markdown` field `key`, and with `&version=<n>` of that field at the
akt's version `n` (`version.read`). It MUST be authenticated as `/graphql`
is, and answer 404 for an akt the subject may not read, as if it did not
exist, and for a field that isn't `markdown` or a version the akt doesn't
have.

r[pdf.repeatable]
A version's PDF MUST be the same each time it is rendered by the same
Kuria: it is made from the version, which never changes
(`version.immutable`), and its pins, never from a target as it is now.
