// Moves the Markdown plans in .agents/plans into the dogfooding Kuria,
// through the `kuria` CLI as the agent, once each:
//
// - a plan, with its goal, design (with its constraints and starting
//   point) and verification, active, or done once every task is;
// - a task per `Tn` item, from the progress snapshot first, then the task
//   list's others, finished if ticked;
// - a gap per `Gn` item, closed with its text as its resolution if ticked;
// - a decision per "Decision Notes" item, decided.
//
// Each plan's number is recorded in .state/migrated.json, so a second run
// moves only the plans not moved yet. With DRY=1 it says what it would
// make, and makes nothing.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ROOT, STATE, envFile } from './stack.ts';

const PLANS = join(ROOT, '.agents', 'plans');
/** The order the plans were worked in; others follow by name. */
const ORDER = [
	'nesting-and-releases.md',
	'field-kinds-and-artifacts.md',
	'contacts-and-checking-for-others.md',
	'frontend-and-e2e.md',
	'usable-design.md',
	'design-gaps.md',
	'cli-and-dogfooding.md'
];

const scratch = mkdtempSync(join(tmpdir(), 'kuria-migrate-'));
const binary = join(ROOT, 'target', 'debug', 'kuria');

/** Runs `kuria args… --format json` as the agent, answering its JSON. */
function kuria(args: string[]): any {
	const ran = spawnSync(binary, [...args, '--format', 'json'], {
		env: { ...process.env, ...envFile('agent.env') },
		encoding: 'utf8'
	});
	if (ran.status !== 0) throw new Error(`kuria ${args.join(' ')}: ${ran.stderr || ran.stdout}`);
	return JSON.parse(ran.stdout);
}

interface Item {
	checked: boolean;
	id: string;
	text: string;
}

/** The `## ` sections of a Markdown document, by heading. */
function sections(markdown: string): { title: string; sections: [string, string][] } {
	const lines = markdown.split('\n');
	const title = (lines.find((l) => l.startsWith('# ')) ?? '# Untitled').slice(2).trim();
	const found: [string, string[]][] = [];
	for (const line of lines) {
		if (line.startsWith('## ')) found.push([line.slice(3).trim(), []]);
		else if (found.length > 0) found[found.length - 1][1].push(line);
	}
	return { title, sections: found.map(([heading, body]) => [heading, body.join('\n').trim()]) };
}

const named = (all: [string, string][], pattern: RegExp) => all.filter(([heading]) => pattern.test(heading));

/** The top-level list items of `body`, each with its continuation lines. */
function items(body: string): string[] {
	const found: string[][] = [];
	for (const line of body.split('\n')) {
		if (line.startsWith('- ')) found.push([line.slice(2)]);
		else if (found.length > 0 && (line.startsWith('  ') || line.trim() === '')) {
			found[found.length - 1].push(line.replace(/^ {2}/, ''));
		} else if (line.trim() !== '') {
			// Text between lists ends an item.
			found.push([]);
		}
	}
	return found.filter((f) => f.length > 0).map((f) => f.join('\n').trim());
}

/** Items such as `[x] T3 Filters: …`, by the letter of their ids. */
function numbered(body: string, letter: string): Item[] {
	const pattern = new RegExp(`^\\[( |x)\\] (${letter}[0-9][0-9a-z.]*)\\s+([\\s\\S]*)$`);
	return items(body).flatMap((item) => {
		const match = pattern.exec(item);
		return match ? [{ checked: match[1] === 'x', id: match[2], text: match[3].trim() }] : [];
	});
}

/** A title from the start of `text`: up to its first colon or sentence, at most 100 characters. */
function headOf(text: string): string {
	// The first paragraph, unwrapped: list items wrap mid-sentence.
	const line = text
		.split(/\n\s*\n/)[0]
		.split('\n')
		.map((l) => l.trim())
		.join(' ')
		.replace(/\*\*/g, '');
	// Colons and full stops inside `code` don't end it.
	const outside = line.replace(/`[^`]*`/g, (code) => 'x'.repeat(code.length));
	const colon = outside.indexOf(':');
	const sentence = outside.search(/\.(\s|$)/);
	const ends = [colon, sentence].filter((i) => i > 0);
	let head = ends.length > 0 ? line.slice(0, Math.min(...ends)) : line;
	if (head.length > 100) head = `${head.slice(0, 99).trimEnd()}…`;
	return head.trim();
}

/** An akt file with `front` and multiline `sections`. */
function akt(front: Record<string, string>, multiline: Record<string, string>): string {
	const path = join(scratch, `akt-${Math.random().toString(36).slice(2)}.md`);
	const lines = ['---', ...Object.entries(front).map(([k, v]) => `${k}: ${JSON.stringify(v)}`), '---', ''];
	for (const [key, text] of Object.entries(multiline)) {
		if (text.trim()) lines.push(`<!-- field: ${key} -->`, text.trim(), '');
	}
	writeFileSync(path, lines.join('\n'));
	return path;
}

function migrate(file: string): string {
	const { title, sections: all } = sections(readFileSync(join(PLANS, file), 'utf8'));
	const reason = `Migrated from .agents/plans/${file}`;
	const body = (pattern: RegExp, withHeadings: boolean) =>
		named(all, pattern)
			.map(([heading, text]) => (withHeadings ? `## ${heading}\n\n${text}` : text))
			.join('\n\n');
	const goal = body(/^(goal|purpose)/i, false) || all[0]?.[1] || title;
	const design = body(/^(where it starts|constraints|current direction|design)/i, true);
	const verification = body(/verification|testing strategy/i, false);

	const snapshot = named(all, /progress snapshot/i).map(([, b]) => b).join('\n\n');
	const list = named(all, /task list|milestones/i).map(([, b]) => b).join('\n\n');
	const tasks = numbered(snapshot, 'T');
	const ids = new Set(tasks.map((t) => t.id));
	for (const task of numbered(list, 'T')) {
		// Sub-tasks of a task already listed are its details.
		if (ids.has(task.id) || ids.has(task.id.split('.')[0])) continue;
		ids.add(task.id);
		tasks.push(task);
	}
	const gaps: Item[] = [];
	for (const [, text] of all) {
		for (const gap of numbered(text, 'G')) {
			if (!gaps.some((g) => g.id === gap.id)) gaps.push(gap);
		}
	}
	const decisions = named(all, /decision/i).flatMap(([, b]) => items(b));
	if (process.env.DRY) {
		console.log(`${file}: "${title}", goal ${goal.length}, design ${design.length}, verification ${verification.length} characters`);
		for (const t of tasks) console.log(`  ${t.checked ? 'done' : 'todo'} ${t.id} ${headOf(t.text)}`);
		for (const g of gaps) console.log(`  ${g.checked ? 'closed' : 'open'} ${g.id} ${headOf(g.text)}`);
		for (const d of decisions) console.log(`  decision ${headOf(/^\*\*(.+?)\*\*/.exec(d)?.[1] ?? d)}`);
		return 'DRY';
	}

	const plan = kuria([
		'new',
		'plan',
		'--file',
		akt({ type: 'plan', title }, { goal, design, verification }),
		'--reason',
		reason
	]).number;
	kuria(['do', plan, 'activate', '--reason', reason]);
	for (const task of tasks) {
		const number = kuria([
			'new',
			'task',
			'--in',
			plan,
			'--file',
			akt({ type: 'task', title: `${task.id} ${headOf(task.text)}` }, { description: task.text }),
			'--reason',
			reason
		]).number;
		if (task.checked) {
			kuria(['do', number, 'start', '--reason', reason]);
			kuria(['do', number, 'finish', '--reason', reason]);
		}
	}
	for (const gap of gaps) {
		const number = kuria([
			'new',
			'gap',
			'--in',
			plan,
			'--file',
			akt({ type: 'gap', title: `${gap.id} ${headOf(gap.text)}` }, { description: gap.text }),
			'--reason',
			reason
		]).number;
		if (gap.checked) {
			const resolution = akt({}, {}).replace(/\.md$/, '.txt');
			writeFileSync(resolution, gap.text);
			kuria(['do', number, 'close', '--set', `resolution=@${resolution}`, '--reason', reason]);
		}
	}
	for (const text of decisions) {
		const lead = /^\*\*(.+?)\*\*[:.]?\s*([\s\S]*)$/.exec(text);
		const decision = kuria([
			'new',
			'decision',
			'--in',
			plan,
			'--file',
			akt(
				{ type: 'decision', title: headOf(lead ? lead[1] : text) },
				{
					context: `Recorded in the plan "${title}", migrated from \`.agents/plans/${file}\`.`,
					decision: text,
					why: (lead?.[2] || text).trim() || text
				}
			),
			'--reason',
			reason
		]).number;
		kuria(['do', decision, 'decide', '--reason', reason]);
	}
	if (tasks.length > 0 && tasks.every((t) => t.checked)) kuria(['do', plan, 'finish', '--reason', reason]);
	console.log(
		`${file} → ${plan}: ${tasks.length} tasks (${tasks.filter((t) => t.checked).length} done), ${gaps.length} gaps, ${decisions.length} decisions`
	);
	return plan;
}

const record = join(STATE, 'migrated.json');
const migrated: Record<string, string> = existsSync(record) ? JSON.parse(readFileSync(record, 'utf8')) : {};
const files = readdirSync(PLANS)
	.filter((f) => f.endsWith('.md') && f !== 'PLANS.md')
	.sort((a, b) => {
		const rank = (f: string) => (ORDER.includes(f) ? ORDER.indexOf(f) : ORDER.length);
		return rank(a) - rank(b) || a.localeCompare(b);
	});
for (const file of files) {
	if (migrated[file]) {
		console.log(`${file}: already ${migrated[file]}`);
		continue;
	}
	const plan = migrate(file);
	if (process.env.DRY) continue;
	migrated[file] = plan;
	writeFileSync(record, JSON.stringify(migrated, null, 2));
}
