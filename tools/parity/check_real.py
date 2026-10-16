# /// script
# requires-python = ">=3.11"
# dependencies = ["pypdf==6.20.1", "pillow>=10", "numpy>=1.26"]
# ///
"""Opt-in synthetic renderer integration and WordCraft metric-equivalence proof."""

import argparse
import json
from pathlib import Path
import shutil

from engines import Settings, digest, process
from matrix import MatrixSpec, execute

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("pptx", type=Path)
p.add_argument("reference_pdf", type=Path)
p.add_argument("config", type=Path)
p.add_argument("wordcraft_xtask", type=Path)
p.add_argument("output", type=Path)
p.add_argument("--reference-version", default="unspecified")
p.add_argument(
    "--wordcraft-revision", default="4126dce8943eb9ed3e7e9fe3dea54bcdeee5083b"
)
a = p.parse_args()
settings = Settings.load(a.config)
report = execute(
    a.pptx,
    a.output,
    a.output.parent / "cache",
    settings,
    MatrixSpec(
        reference=a.reference_pdf,
        reference_engine="chrome",
        reference_version=a.reference_version,
    ),
)
if report["status"] != "complete":
    raise SystemExit("Real integration incomplete; inspect report.json")
checks = []
for name, pair in report["pairs"].items():
    first, second = (
        report["providers"][pair["reference"]],
        report["providers"][pair["candidate"]],
    )
    # The reference comparator invokes Poppler on PATH; refuse a mixed-tool equivalence claim.
    for tool, identity in first["identity"]["tools"].items():
        found = shutil.which(tool)
        if not found or digest(found) != identity["sha256"]:
            raise SystemExit(f"Reference comparator PATH uses different {tool}")
    original = a.output / (name + "-wordcraft")
    process(
        [
            a.wordcraft_xtask.resolve(),
            "visual-diff",
            a.output / first["pdf"],
            a.output / second["pdf"],
            original,
            "--private",
            "--dpi",
            "100",
        ],
        settings.data["timeout_seconds"],
    )
    expected = json.loads((original / "result.json").read_text())
    fields = ["ssim_mean", "same_break_pct", "lines", "pages", "page_starts_same"]
    for field in fields:
        checks.append(
            {
                "pair": name,
                "field": "summary." + field,
                "matched": pair["summary"][field] == expected["summary"][field],
            }
        )
    for current, old in zip(pair["pages"], expected["pages"]):
        for field in [
            "ssim",
            "diff_px",
            "same_break",
            "dx",
            "dy",
            "lines",
            "page_start_same",
        ]:
            checks.append(
                {
                    "pair": name,
                    "page": current["page"],
                    "field": field,
                    "matched": current.get(field) == old.get(field),
                }
            )
result = {
    "status": "passed" if all(c["matched"] for c in checks) else "failed",
    "wordcraft_revision_label": a.wordcraft_revision,
    "wordcraft_binary_sha256": digest(a.wordcraft_xtask),
    "checks": checks,
    "gaps": "Synthetic, same-Poppler comparison; not PowerPoint or general renderer certification.",
}
(a.output / "metric-equivalence.json").write_text(json.dumps(result, indent=2))
print(
    json.dumps(
        {
            "status": result["status"],
            "checks": len(checks),
            "report": str(a.output / "report.json"),
        }
    )
)
raise SystemExit(0 if result["status"] == "passed" else 1)
