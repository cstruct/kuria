/** How an akt type is marked wherever the app names it: the icon and colour
 * its presentation gives, or none and neutral. Typed by GraphQL's enums, so
 * an icon or colour the server adds can't be missed here. */
// r[impl app.type-identity]
import type { Component } from 'svelte';
import Archive from '@lucide/svelte/icons/archive';
import BookOpen from '@lucide/svelte/icons/book-open';
import Box from '@lucide/svelte/icons/box';
import Bug from '@lucide/svelte/icons/bug';
import Calendar from '@lucide/svelte/icons/calendar';
import CircleCheck from '@lucide/svelte/icons/circle-check';
import ClipboardCheck from '@lucide/svelte/icons/clipboard-check';
import FileText from '@lucide/svelte/icons/file-text';
import Flag from '@lucide/svelte/icons/flag';
import FlaskConical from '@lucide/svelte/icons/flask-conical';
import GitBranch from '@lucide/svelte/icons/git-branch';
import Layers from '@lucide/svelte/icons/layers';
import ListChecks from '@lucide/svelte/icons/list-checks';
import Milestone from '@lucide/svelte/icons/milestone';
import NotebookPen from '@lucide/svelte/icons/notebook-pen';
import Package from '@lucide/svelte/icons/package';
import Rocket from '@lucide/svelte/icons/rocket';
import ShieldCheck from '@lucide/svelte/icons/shield-check';
import Tag from '@lucide/svelte/icons/tag';
import Target from '@lucide/svelte/icons/target';
import TriangleAlert from '@lucide/svelte/icons/alert-triangle';
import Users from '@lucide/svelte/icons/users';
import Wrench from '@lucide/svelte/icons/wrench';
import Zap from '@lucide/svelte/icons/zap';
import type { AktTypeColour, AktTypeIcon } from '#lib/graphql/generated/graphql.ts';

type Icon = Component<{ size?: number; 'aria-hidden'?: boolean }>;

export const ICONS: Record<AktTypeIcon, Icon> = {
	ARCHIVE: Archive,
	BOOK_OPEN: BookOpen,
	BOX: Box,
	BUG: Bug,
	CALENDAR: Calendar,
	CIRCLE_CHECK: CircleCheck,
	CLIPBOARD_CHECK: ClipboardCheck,
	FILE_TEXT: FileText,
	FLAG: Flag,
	FLASK_CONICAL: FlaskConical,
	GIT_BRANCH: GitBranch,
	LAYERS: Layers,
	LIST_CHECKS: ListChecks,
	MILESTONE: Milestone,
	NOTEBOOK_PEN: NotebookPen,
	PACKAGE: Package,
	ROCKET: Rocket,
	SHIELD_CHECK: ShieldCheck,
	TAG: Tag,
	TARGET: Target,
	TRIANGLE_ALERT: TriangleAlert,
	USERS: Users,
	WRENCH: Wrench,
	ZAP: Zap
};

export const COLOURS: Record<AktTypeColour, string> = {
	NEUTRAL: 'neutral',
	BLUE: 'blue',
	GREEN: 'green',
	AMBER: 'amber',
	RED: 'red',
	PURPLE: 'purple',
	TEAL: 'teal',
	PINK: 'pink'
};

export interface Marked {
	presentation: { icon?: AktTypeIcon | null; colour: AktTypeColour };
}

/** A type's icon, if it has one, and the colour name its tokens use
 * (`--type-<colour>-bg` and `-fg`). */
export function markOf(type: Marked): { icon: Icon | null; colour: string } {
	const { icon, colour } = type.presentation;
	return { icon: icon ? ICONS[icon] : null, colour: COLOURS[colour] };
}
