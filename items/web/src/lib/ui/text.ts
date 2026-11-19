/** `value` shortened in the middle to at most `max` characters, keeping
 * its beginning and end, as identifiers, SHAs and digests read best. */
export function truncateMiddle(value: string, max: number): string {
	if (value.length <= max) return value;
	const keep = Math.max(max - 1, 2);
	const head = Math.ceil(keep / 2);
	const tail = keep - head;
	return `${value.slice(0, head)}…${tail > 0 ? value.slice(-tail) : ''}`;
}

/** Words joined as people list alternatives: "task", "task or epic",
 * "task, epic or bug". */
export function orList(words: readonly string[]): string {
	if (words.length <= 1) return words[0] ?? '';
	return `${words.slice(0, -1).join(', ')} or ${words[words.length - 1]}`;
}
