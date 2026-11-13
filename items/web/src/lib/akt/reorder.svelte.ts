/** Dragging members within a board column or a list to reorder them, or
 * onto another column to change their state. Within its group a member is
 * placed by rank; another column takes it only when a transition leads
 * there (`accepts`), and dropping it there asks which (`onInto`). The menu
 * on each member is the keyboard's way to reorder. */
// r[impl app.members]
import { placeFor } from './members';

export type Place = { before: string } | { after: string };

export class Reorder {
	/** The member being dragged, and the group it is in. */
	dragging = $state<{ number: string; group: string } | null>(null);
	/** Where it would land: before the member at `index` of `group`. */
	target = $state<{ group: string; index: number } | null>(null);
	/** Another group it would be dropped into, which accepts it. */
	into = $state<string | null>(null);

	#onMove: (number: string, place: Place) => void;
	#accepts: (number: string, group: string) => boolean;
	#onInto: (number: string, group: string) => void;

	constructor(
		onMove: (number: string, place: Place) => void,
		accepts: (number: string, group: string) => boolean = () => false,
		onInto: (number: string, group: string) => void = () => {}
	) {
		this.#onMove = onMove;
		this.#accepts = accepts;
		this.#onInto = onInto;
	}

	/** Whether a group refuses what is being dragged. */
	refuses(group: string): boolean {
		return (
			this.dragging !== null &&
			this.dragging.group !== group &&
			!this.#accepts(this.dragging.number, group)
		);
	}

	/** Whether a group would take what is being dragged into it. */
	takes(group: string): boolean {
		return (
			this.dragging !== null &&
			this.dragging.group !== group &&
			this.#accepts(this.dragging.number, group)
		);
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
		if (!this.#within(event, group)) return;
		event.stopPropagation();
		const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
		const after = event.clientY > box.top + box.height / 2;
		this.target = { group, index: after ? index + 1 : index };
	}

	/** Over the group itself, past its members: at its end. */
	overGroup(event: DragEvent, group: string, count: number) {
		if (!this.#within(event, group)) return;
		this.target = { group, index: count };
	}

	drop(event: DragEvent, group: string, numbers: readonly string[]) {
		event.preventDefault();
		const { dragging, target, into } = this;
		this.end();
		if (dragging && into === group && dragging.group !== group) {
			this.#onInto(dragging.number, group);
			return;
		}
		if (!dragging || !target || dragging.group !== group || target.group !== group) return;
		const place = placeFor(numbers, dragging.number, target.index);
		if (place) this.#onMove(dragging.number, place);
	}

	end() {
		this.dragging = null;
		this.target = null;
		this.into = null;
	}

	/** Whether the drag over `group` reorders within it. Over another
	 * group, it notes whether that group takes it instead. */
	#within(event: DragEvent, group: string): boolean {
		if (!this.dragging) return false;
		if (this.dragging.group !== group) {
			this.target = null;
			if (this.#accepts(this.dragging.number, group)) {
				event.preventDefault();
				event.stopPropagation();
				if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
				this.into = group;
			} else {
				if (event.dataTransfer) event.dataTransfer.dropEffect = 'none';
				this.into = null;
			}
			return false;
		}
		this.into = null;
		event.preventDefault();
		if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
		return true;
	}
}
