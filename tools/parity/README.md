# parity: recreate real slides in sldr, and measure it

A harness for the question "how close can sldr get to a real PowerPoint
slide, and what is missing?". An agent (pi) recreates one slide at a time in
sldr; deterministic tools measure the result; every remaining difference is
recorded as a structured gap, so gaps can be ranked across hundreds of slides.

The agent uses sldr; sldr never calls an agent. Nothing here ships in the
binary.

## Pieces

| Piece | What it does |
|-------|--------------|
| `parity.py` | Deterministic CLI (uv script, deps inline): `ingest`, `baseline`, `render`, `score`, `gap`, `report` |
| `pi-extension/index.ts` | pi tools `parity_score` (numbers + original/sldr/difference image) and `parity_gap` (typed gap record); points sldr at the case's own library |
| `skill/sldr-parity/` | The procedure the agent follows inside a case |
| `run-case.sh` | One pi run on one case, isolated (no global extensions/skills), transcript kept; `PARITY_TUI=1` opens the pi TUI instead |
| `run-deck.sh` | Every case of a deck in order (skips scored ones unless `PARITY_REDO=1`), then rescore all and report |

## The lab

Cases live in `~/sldr-lab` (`PARITY_LAB` overrides). It holds source decks and
their renders, so it stays **private**: never commit it, never copy client
material into this repository. Gap records are generic by rule; only those
reach sldr's issue tracker.

```
~/sldr-lab/
  decks/<deck>/original.pdf     the source deck rendered once (cached)
  decks/<deck>/lib/             the deck's shared flavors/ and layouts/ (house style, built once)
  cases/<deck>-sNN/
    case.json  original.png  extract.json  media/
    lib/ config/ out/            the case's sldr library (flavors/, layouts/ link to the deck's) and config
    score.json  compare.png  heatmap.png  history.jsonl  gaps.jsonl
    runs/<timestamp>/pi.jsonl    agent transcripts
  report.html
```

## Use

```bash
P=tools/parity/parity.py
$P ingest ~/decks/talk.pptx --slides all --case-prefix talk     # or --slides 1,4-9
tools/parity/run-deck.sh talk --thinking medium                 # whole deck, one pi session per slide
tools/parity/run-case.sh talk-s04 --model anthropic/claude-sonnet-5   # one slide, another model
PARITY_TUI=1 tools/parity/run-case.sh talk-s04                   # watch and steer in the pi TUI
$P baseline talk-s04 --library ~/sldr --slide talk/04-intro --flavor house  # score a hand port
$P rescore talk                                                  # after shared-style changes
$P report                                                        # ~/sldr-lab/report.html
```

Batch runs use `pi -p --mode json` (print mode; the event stream is the transcript). The cases
of a deck share its flavors and layouts: the first slide builds the house style, later slides reuse
and refine it, and `run-deck.sh` rescores every slide at the end because a shared change can shift
earlier ones. Each slide still gets its own pi session, score and gaps.

## Scores

- **SSIM** (structural similarity, 0–1) of the two renders in grayscale: the headline number.
- **color error**: mean absolute pixel difference.
- **text recall**: share of the original's words that the sldr render contains (read from its PDF).
- **worst regions**: grid cells with the largest blurred brightness difference, as % boxes.
- The **difference image** marks in red where the renders differ (blurred brightness difference, not
  the SSIM map, which lights up flat backgrounds).

## Caveats

- Originals render through LibreOffice for now; fonts that are not installed shift text. PowerPoint
  renders (on a Mac) are the target ground truth.
- Pixel-identical is not the goal: browsers and PowerPoint rasterize text differently. Use SSIM
  trends, the heatmap and a human eye for the last few percent.
