/** Uploads `file` as an attachment; its identifier and name. */
export async function upload(file: File): Promise<{ id: string; name: string }> {
	const response = await fetch('/attachments', {
		method: 'POST',
		credentials: 'same-origin',
		headers: {
			'x-attachment-name': encodeURIComponent(file.name),
			'content-type': file.type || 'application/octet-stream'
		},
		body: file
	});
	if (response.status === 413) throw new Error(`${file.name} is larger than Kuria takes.`);
	if (!response.ok) throw new Error(`Uploading ${file.name} failed (${response.status}).`);
	const answer = (await response.json()) as { id: string; name: string };
	return { id: answer.id, name: answer.name };
}
