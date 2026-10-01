/**
 * sldr-parity — pi tools for recreating a real PowerPoint slide in sldr.
 *
 * Thin, typed adapter over tools/parity/parity.py (the deterministic part):
 *   parity_score  render the case's sldr slide and compare it with the original;
 *                 returns the numbers plus the side-by-side/heatmap image
 *   parity_gap    record one remaining difference as a structured gap
 *
 * Run pi inside a case folder (<lab>/cases/<case>). On session start the
 * extension points sldr at that case's own library (XDG_CONFIG_HOME=<case>/config),
 * so `sldr` commands the agent runs can never touch the user's real library.
 */

import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { execFile } from "node:child_process";
import { fileURLToPath } from "node:url";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";

const HERE = dirname(fileURLToPath(import.meta.url));
const PARITY = process.env.PARITY_BIN ?? resolve(HERE, "..", "parity.py");
const GAP_KINDS = ["missing_feature", "parameter", "agent_error", "renderer"] as const;

function caseDir(cwd: string): string | undefined {
	return existsSync(join(cwd, "case.json")) ? cwd : undefined;
}

function parity(args: string[], cwd: string, signal?: AbortSignal): Promise<{ ok: boolean; out: string }> {
	return new Promise((done) => {
		execFile(PARITY, args, { cwd, signal, maxBuffer: 16 * 1024 * 1024, timeout: 15 * 60 * 1000 }, (err, stdout, stderr) => {
			done({ ok: !err, out: `${stdout}${stderr}`.trim() });
		});
	});
}

function text(t: string, isError = false) {
	return { content: [{ type: "text" as const, text: t }], isError, details: {} };
}

export default function (pi: ExtensionAPI) {
	pi.on("session_start", async (_event, ctx) => {
		const dir = caseDir(ctx.cwd);
		if (dir) process.env.XDG_CONFIG_HOME = join(dir, "config");
	});

	pi.registerTool({
		name: "parity_score",
		label: "Parity score",
		description:
			"Build this case's sldr slide (lib/slides, playlist 'case'), render it at 1920x1080 and compare it with " +
			"original.png. Returns SSIM (structure, 1.0 = identical), color error, text recall, the words still missing, " +
			"the worst regions as % boxes, and an image: original | sldr | difference (red = differs). Call it after every " +
			"change; the score history is kept in history.jsonl.",
		parameters: Type.Object({
			note: Type.Optional(Type.String({ description: "What you changed since the last score (kept in the history)" })),
		}),
		execute: async (_id, params, signal, _onUpdate, ctx) => {
			const dir = caseDir(ctx.cwd);
			if (!dir) return text("Not in a parity case folder (no case.json in the working directory).", true);
			const args = ["score", dir];
			if (params.note) args.push("--note", params.note);
			const r = await parity(args, dir, signal);
			if (!r.ok) return text(`parity score failed:\n${r.out}`, true);
			const score = JSON.parse(readFileSync(join(dir, "score.json"), "utf8"));
			const summary = [
				`SSIM ${score.ssim} | match ${Math.round(score.match * 100)}% of the slide (offsets up to 6 px tolerated) | color error ${score.color_error} | text recall ${Math.round(score.text_recall * 100)}%`,
				score.missing_words.length ? `missing words: ${score.missing_words.join(" ")}` : "no missing words",
				`worst regions (% of slide): ${JSON.stringify(score.worst_regions)}`,
			].join("\n");
			const image = readFileSync(score.compare).toString("base64");
			return {
				content: [
					{ type: "text" as const, text: summary },
					{ type: "image" as const, data: image, mimeType: "image/png" },
				],
				details: score,
			};
		},
	});

	pi.registerTool({
		name: "parity_gap",
		label: "Parity gap",
		description:
			"Record one difference you could not close, so gaps can be ranked across many slides. kind: " +
			"missing_feature (sldr cannot express it: no layout, primitive or option), parameter (expressible, but a " +
			"value is off: size, spacing, color, position), agent_error (you could do it but did not), renderer (the " +
			"original's render is wrong, e.g. a missing font). feature: a short, generic name reused across slides " +
			"(e.g. 'shape: triangle with labels at the corners', 'chart: line chart', 'logo size in framed header'). " +
			"evidence: what the original shows vs what sldr shows. Never include names, brands or client details.",
		parameters: Type.Object({
			kind: Type.Union(GAP_KINDS.map((k) => Type.Literal(k))),
			feature: Type.String(),
			evidence: Type.String(),
		}),
		execute: async (_id, params, signal, _onUpdate, ctx) => {
			const dir = caseDir(ctx.cwd);
			if (!dir) return text("Not in a parity case folder (no case.json in the working directory).", true);
			const r = await parity(["gap", dir, "--kind", params.kind, "--feature", params.feature, "--evidence", params.evidence], dir, signal);
			return text(r.ok ? `recorded: ${r.out}` : `parity gap failed:\n${r.out}`, !r.ok);
		},
	});
}
