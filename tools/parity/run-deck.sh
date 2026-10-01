#!/usr/bin/env bash
# Recreate every ingested slide of one deck, in order, then rescore them all.
#
#   tools/parity/run-deck.sh <prefix> [pi flags…]
#   tools/parity/run-deck.sh kickoff19 --thinking medium
#
# The cases share the deck's flavors/ and layouts/, so the first slide builds
# the house style and later slides reuse it. A shared change can shift
# earlier slides, hence the final rescore. One pi session per slide.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
lab="${PARITY_LAB:-$HOME/sldr-lab}"
prefix="${1:?usage: run-deck.sh <prefix> [pi flags…]}"
shift

mapfile -t cases < <(find "$lab/cases" -maxdepth 1 -mindepth 1 -type d -name "$prefix-s*" -printf '%f\n' | sort)
[[ ${#cases[@]} -gt 0 ]] || { echo "no cases for '$prefix' in $lab/cases (run parity.py ingest first)" >&2; exit 1; }

echo "recreating ${#cases[@]} slides of '$prefix'"
for c in "${cases[@]}"; do
	if [[ -f "$lab/cases/$c/score.json" && "${PARITY_REDO:-0}" != 1 ]]; then
		echo "  = $c (already scored; PARITY_REDO=1 to run again)"
		continue
	fi
	echo "  > $c"
	"$here/run-case.sh" "$c" "$@" >/dev/null || echo "  ! $c failed (see its runs/ folder)"
done

echo "rescoring the deck"
"$here/parity.py" rescore "$prefix"
"$here/parity.py" report
