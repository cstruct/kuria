# Attachments

An attachment is a file uploaded to Kuria, such as a log, a screenshot or a
software bill of materials (SBOM). Akter hold attachments in `attachment`
fields (`akt.field.attachment`). The bytes live in S3-compatible object
storage, and Postgres keeps what Kuria knows about them.

## Uploading

r[attachment.upload]
`POST /attachments` MUST store its body as a new attachment of the caller's
tenant, named by the `X-Attachment-Name` header and typed by `Content-Type`,
and answer with its identifier, size and SHA-256. It MUST be authenticated
as `/graphql` is, including the origin check for session cookies.

r[attachment.limit]
An upload larger than the configured limit MUST be refused with 413, and
nothing of it stored.

r[attachment.immutable]
An attachment MUST never change once stored. Its bytes MUST be stored under
their SHA-256 within the tenant, so that the same file uploaded twice is
stored once.

r[attachment.metadata]
Kuria MUST keep, for each attachment: its tenant, identifier, name, media
type, size, SHA-256, the subject who uploaded it and when, and, for an SBOM,
its summary.

## SBOMs

r[attachment.sbom]
At upload, Kuria MUST recognise as an SBOM a JSON document that is either
SPDX (it has `spdxVersion`) or CycloneDX (its `bomFormat` is `CycloneDX`),
and keep its summary: the format, the specification version, the document's
name if it has one, and how many packages or components it lists. Any other
file MUST NOT be taken for an SBOM.

## Reading

r[attachment.media-types]
A write MUST be refused, as `REFUSED`, when it puts in an
`attachment` field that lists `media_types` an attachment of a media type
it doesn't list.

r[attachment.download]
`GET /attachments/{id}` MUST answer the attachment's bytes, with its media
type and name, to a subject who may read an akt that holds it, in a field
or as a diagram (`akt.field.markdown.diagrams`), or to the subject who
uploaded it while no akt holds it. To anyone else it MUST answer
404, as if it did not exist.

r[attachment.references]
Kuria MUST know, whenever an akt is written, which attachments its fields
hold, so that `attachment.download` is decided without reading every akt.
