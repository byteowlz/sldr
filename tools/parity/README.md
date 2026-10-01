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
| `run-case.sh` | One pi run on one case, isolated (no global extensions/skills), transcript kept |

## The lab

Cases live in `~/sldr-lab` (`PARITY_LAB` overrides). It holds source decks and
their renders, so it stays **private**: never commit it, never copy client
material into this repository. Gap records are generic by rule; only those
reach sldr's issue tracker.

```
~/sldr-lab/
  decks/<deck>/original.pdf     the source deck rendered once (cached)
  cases/<deck>-sNN/
    case.json  original.png  extract.json  media/
    lib/ config/ out/            the case's own sldr library and config
    score.json  compare.png  heatmap.png  history.jsonl  gaps.jsonl
    runs/<timestamp>/pi.jsonl    agent transcripts
  report.html
```

## Use

```bash
P=tools/parity/parity.py
$P ingest ~/decks/talk.pptx --slides 1,4-9 --case-prefix talk   # cases + original renders
$P baseline talk-s04 --library ~/sldr --slide talk/04-intro --flavor house  # score a hand port
tools/parity/run-case.sh talk-s04 --model anthropic/claude-sonnet-5 --thinking medium
$P report                                                        # ~/sldr-lab/report.html
```

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
