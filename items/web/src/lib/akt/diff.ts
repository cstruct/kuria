/** Two texts compared line by line (`version.diff`): each line kept,
 * removed or added, in order. Nothing here knows any akt type. */
// r[impl app.versions]

export interface DiffLine {
	tag: ' ' | '-' | '+';
	text: string;
}

/** `before` and `after` line by line, by their longest common lines. */
export function lineDiff(before: string, after: string): DiffLine[] {
	const a = before === '' ? [] : before.replace(/\n$/, '').split('\n');
	const b = after === '' ? [] : after.replace(/\n$/, '').split('\n');
	// For each i and j, how many lines the rest of a from i and of b from j
	// have in common, at most.
	const common = Array.from({ length: a.length + 1 }, () =>
		new Array<number>(b.length + 1).fill(0)
	);
	for (let i = a.length - 1; i >= 0; i--) {
		for (let j = b.length - 1; j >= 0; j--) {
			common[i][j] =
				a[i] === b[j] ? common[i + 1][j + 1] + 1 : Math.max(common[i + 1][j], common[i][j + 1]);
		}
	}
	const lines: DiffLine[] = [];
	let i = 0;
	let j = 0;
	while (i < a.length || j < b.length) {
		if (i < a.length && j < b.length && a[i] === b[j]) {
			lines.push({ tag: ' ', text: a[i] });
			i++;
			j++;
		} else if (i < a.length && (j === b.length || common[i + 1][j] >= common[i][j + 1])) {
			lines.push({ tag: '-', text: a[i] });
			i++;
		} else {
			lines.push({ tag: '+', text: b[j] });
			j++;
		}
	}
	return lines;
}
