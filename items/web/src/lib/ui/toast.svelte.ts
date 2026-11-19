/** Short messages shown for a moment: what a change did, or why it was
 * refused, read out by assistive technology as they appear. */

export interface Toast {
	id: number;
	message: string;
	tone: 'neutral' | 'success' | 'danger';
	/** Something to do about it, such as undoing it. */
	action?: { label: string; run: () => void };
}

let next = 1;
export const toasts = $state<Toast[]>([]);

/** Shows `message` for a few seconds, or until dismissed; one with an
 * action a little longer, to give time to take it. */
export function toast(
	message: string,
	tone: Toast['tone'] = 'neutral',
	ms = 5000,
	action?: Toast['action']
): void {
	const id = next++;
	toasts.push({ id, message, tone, action });
	setTimeout(() => dismiss(id), action ? Math.max(ms, 8000) : ms);
}

export function dismiss(id: number): void {
	const i = toasts.findIndex((t) => t.id === id);
	if (i >= 0) toasts.splice(i, 1);
}
