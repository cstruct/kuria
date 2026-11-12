/** Short messages shown for a moment: what a change did, or why it was
 * refused, read out by assistive technology as they appear. */

export interface Toast {
	id: number;
	message: string;
	tone: 'neutral' | 'success' | 'danger';
}

let next = 1;
export const toasts = $state<Toast[]>([]);

/** Shows `message` for a few seconds, or until dismissed. */
export function toast(message: string, tone: Toast['tone'] = 'neutral', ms = 5000): void {
	const id = next++;
	toasts.push({ id, message, tone });
	setTimeout(() => dismiss(id), ms);
}

export function dismiss(id: number): void {
	const i = toasts.findIndex((t) => t.id === id);
	if (i >= 0) toasts.splice(i, 1);
}
