#!/usr/bin/env bash
# Run one recreation attempt on a parity case with pi.
#
#   tools/parity/run-case.sh <case> [pi flags…]
#   tools/parity/run-case.sh genai-s33 --model anthropic/claude-sonnet-5 --thinking medium
#
# PARITY_TUI=1 opens the pi TUI instead of the batch run, to watch and steer.
#
# Only the sldr skills and the parity extension load (no global pi extensions,
# skills or prompt templates), so runs are comparable across models. The full
# transcript lands in <case>/runs/<timestamp>/pi.jsonl.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
lab="${PARITY_LAB:-$HOME/sldr-lab}"
name="${1:?usage: run-case.sh <case> [pi flags…]}"
shift
case_dir="$lab/cases/$name"
[[ -f "$case_dir/case.json" ]] || { echo "no case at $case_dir" >&2; exit 1; }

use_sldr="${USE_SLDR_SKILL:-$HOME/.agents/skills/use-sldr}"
run_dir="$case_dir/runs/$(date +%Y%m%dT%H%M%S)"
mkdir -p "$run_dir"
printf '%s\n' "$*" > "$run_dir/pi-args.txt"

prompt="You are in a parity case folder. Follow the sldr-parity skill: recreate original.png in sldr \
(lib/slides/slide.md, plus the deck's shared flavor or a layout in lib/ if needed), score with parity_score after \
every change, iterate on the biggest differences, record every remaining difference with parity_gap, then give the \
two-line summary."
common=(--session-dir "$case_dir/sessions" --no-extensions --no-skills --no-prompt-templates
	--skill "$use_sldr" --skill "$here/skill/sldr-parity" -e "$here/pi-extension/index.ts")

cd "$case_dir"
if [[ "${PARITY_TUI:-0}" == 1 ]]; then
	# Interactive: watch and steer the agent; the session is still saved under sessions/.
	pi "${common[@]}" "$@" "$prompt"
else
	# Batch: print mode, the full event stream kept as the run's transcript.
	pi -p --mode json "${common[@]}" "$@" "$prompt" > "$run_dir/pi.jsonl"
fi

cp -f score.json "$run_dir/" 2>/dev/null || true
cp -f compare.png "$run_dir/" 2>/dev/null || true
echo "$run_dir"
