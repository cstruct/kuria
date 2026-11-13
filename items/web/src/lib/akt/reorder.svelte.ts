/** Dragging members within a board column or a list to reorder them. A
 * member moves only within the group it is in: a column change would be a
 * change of state, which only a transition makes. The menu on each member
 * is the keyboard's way to do the same. */
// r[impl app.members]
import { placeFor } from './members';

export type Place = { before: string } | { after: string };

export class Reorder {
	/** The member being dragged, and the group it is in. */
	dragging = $state<{ number: string; group: string } | null>(null);
	/** Where it would land: before the member at `index` of `group`. */
	target = $state<{ group: string; index: number } | null>(null);

	#onMove: (number: string, place: Place) => void;

	constructor(onMove: (number: string, place: Place) => void) {
		this.#onMove = onMove;
	}

	/** Whether a group refuses what is being dragged. */
	refuses(group: string): boolean {
		return this.dragging !== null && this.dragging.group !== group;
	}

	start(event: DragEvent, number: string, group: string) {
		this.dragging = { number, group };
		if (event.dataTransfer) {
			event.dataTransfer.effectAllowed = 'move';
			event.dataTransfer.setData('text/plain', number);
		}
	}

	/** Over the member at `index`: before it in its top half, after it in
	 * its bottom half. */
	overMember(event: DragEvent, group: string, index: number) {
		if (!this.#accepts(event, group)) return;
		event.stopPropagation();
		const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
		const after = event.clientY > box.top + box.height / 2;
		this.target = { group, index: after ? index + 1 : index };
	}

	/** Over the group itself, past its members: at its end. */
	overGroup(event: DragEvent, group: string, count: number) {
		if (!this.#accepts(event, group)) return;
		this.target = { group, index: count };
	}

	drop(event: DragEvent, group: string, numbers: readonly string[]) {
		event.preventDefault();
		const { dragging, target } = this;
		this.end();
		if (!dragging || !target || dragging.group !== group || target.group !== group) return;
		const place = placeFor(numbers, dragging.number, target.index);
		if (place) this.#onMove(dragging.number, place);
	}

	end() {
		this.dragging = null;
		this.target = null;
	}

	#accepts(event: DragEvent, group: string): boolean {
		if (!this.dragging) return false;
		if (this.dragging.group !== group) {
			if (event.dataTransfer) event.dataTransfer.dropEffect = 'none';
			this.target = null;
			return false;
		}
		event.preventDefault();
		if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
		return true;
	}
}
