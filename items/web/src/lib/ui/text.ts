/** `value` shortened in the middle to at most `max` characters, keeping
 * its beginning and end, as identifiers, SHAs and digests read best. */
export function truncateMiddle(value: string, max: number): string {
	if (value.length <= max) return value;
	const keep = Math.max(max - 1, 2);
	const head = Math.ceil(keep / 2);
	const tail = keep - head;
	return `${value.slice(0, head)}…${tail > 0 ? value.slice(-tail) : ''}`;
}
