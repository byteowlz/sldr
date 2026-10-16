"""One explicit multi-provider measurement run. Evidence stays local; no pass-by-majority."""

from dataclasses import dataclass, field, replace
from itertools import combinations
from pathlib import Path
import json
import os
import shutil
import tempfile
import xml.etree.ElementTree as ET
import zipfile

from engines import ProviderError, RenderSpec, Settings, digest, render_pdf
from pdf_metrics import PageSpec, compare_renders


@dataclass(frozen=True)
class MatrixSpec:
    engines: tuple[str, ...] = ("deckcraft", "libreoffice")
    reference: Path | None = None
    reference_engine: str = "reference"
    reference_version: str = "unspecified"
    overrides: dict = field(default_factory=dict)
    pages: PageSpec = PageSpec()


@dataclass(frozen=True)
class ReferenceSpec:
    renderer: str | None = None
    reference: Path | None = None
    reference_engine: str = "reference"
    config: Path | None = None
    override: str | None = None
    pages: PageSpec = PageSpec()


def private_destination(path):
    path = Path(path).expanduser()
    if path.is_symlink():
        raise ProviderError("output", "Evidence/cache symlinks are not accepted")
    path = path.resolve()
    if any((p / ".git").exists() for p in [path, *path.parents]):
        raise ProviderError(
            "publication_guard",
            "Keep parity evidence outside git working trees (for example ~/sldr-lab)",
        )
    return path


def _reject_external_assets(package):
    for name in package.namelist():
        if name.endswith(".rels"):
            for rel in ET.fromstring(package.read(name)):
                if rel.get("TargetMode") == "External" and not rel.get(
                    "Type", ""
                ).endswith("/hyperlink"):
                    raise ValueError(
                        "Externally linked assets are not reproducible; embed them before comparing"
                    )


def source_slide_count(source, pages):
    """Independent package count, not a renderer's interpretation; don't fetch linked assets."""
    try:
        with zipfile.ZipFile(source) as package:
            if (
                len(package.infolist()) > 20000
                or sum(i.file_size for i in package.infolist()) > 256 * 1024 * 1024
            ):
                raise ValueError("PPTX package exceeds entry/expanded-size limits")
            root = ET.fromstring(package.read("ppt/presentation.xml"))
            ns = {"p": "http://schemas.openxmlformats.org/presentationml/2006/main"}
            count = len(root.findall("./p:sldIdLst/p:sldId", ns))
            size = root.find("./p:sldSz", ns)
            if size is not None:
                cx, cy = int(size.get("cx", "0")), int(size.get("cy", "0"))
                if (
                    cy <= 0
                    or pages.height <= 0
                    or abs(cx / cy - pages.width / pages.height) > 0.001
                ):
                    raise ValueError(
                        "Source slide aspect disagrees with normalization; set --width/--height explicitly"
                    )
            if not 1 <= count <= 200:
                raise ValueError("Expected 1..200 source slides")
            _reject_external_assets(package)
            return count
    except (OSError, ValueError, KeyError, zipfile.BadZipFile, ET.ParseError) as exc:
        raise ProviderError("input", f"Cannot validate source PPTX: {exc}") from exc


def _matrix_inputs(source, output, cache, options):
    source = Path(source).expanduser().resolve()
    if not source.is_file():
        raise ProviderError("input", f"Source PPTX missing: {source}")
    output, cache = private_destination(output), private_destination(cache)
    if output.exists():
        raise ProviderError(
            "output_exists",
            f"Evidence directory already exists: {output}; choose a new run directory",
        )
    if cache == output or output in cache.parents or cache in output.parents:
        raise ProviderError(
            "output", "Keep run evidence and render cache in separate trees"
        )
    if not options.engines or any(
        e not in ("deckcraft", "libreoffice") for e in options.engines
    ):
        raise ProviderError("arguments", "Engines must be deckcraft and/or libreoffice")
    if options.reference_engine not in ("reference", "chrome", "powerpoint"):
        raise ProviderError(
            "arguments", "Reference label must be reference, chrome or powerpoint"
        )
    if set(options.overrides) - set(options.engines):
        raise ProviderError(
            "arguments", "Executable override supplied for an unselected engine"
        )
    if not options.reference and (
        options.reference_engine != "reference"
        or options.reference_version != "unspecified"
    ):
        raise ProviderError("arguments", "Reference labels require --reference-pdf")
    count = options.pages.count
    if count is None:
        count = source_slide_count(source, options.pages)
    return (
        source,
        output,
        cache,
        replace(options, pages=replace(options.pages, count=count)),
    )


def _render_jobs(source, cache, settings, options, stage):
    jobs = [(e, source) for e in dict.fromkeys(options.engines)]
    if options.reference:
        jobs.insert(
            0,
            (options.reference_engine, Path(options.reference).expanduser().resolve()),
        )
    successful, providers = {}, {}
    for engine, input_file in jobs:
        try:
            spec = RenderSpec(
                options.pages, options.overrides.get(engine), options.reference_version
            )
            result = render_pdf(engine, input_file, cache, settings, spec)
            shutil.copytree(result["directory"], stage / engine)
            successful[engine] = {**result, "directory": str(stage / engine)}
            providers[engine] = {
                **result,
                "status": "ok",
                "directory": engine,
                "pdf": f"{engine}/normalized.pdf",
            }
        except (ProviderError, OSError) as exc:
            providers[engine] = {
                "status": "skipped"
                if getattr(exc, "code", None) == "unavailable"
                else "error",
                "error": {"code": getattr(exc, "code", "io"), "detail": str(exc)},
            }
    return successful, providers


def _compare_inputs(successful, stage):
    pairs = {}
    for first, second in combinations(successful, 2):
        name = f"{first}-vs-{second}"
        folder = stage / name
        folder.mkdir()
        try:
            comparison = compare_renders(successful[first], successful[second], folder)
            pairs[name] = {
                "status": "ok",
                **comparison,
                "directory": name,
                "reference": first,
                "candidate": second,
            }
            (folder / "result.json").write_text(
                json.dumps(pairs[name], indent=2, ensure_ascii=False)
            )
        except (ValueError, OSError) as exc:
            pairs[name] = {
                "status": "error",
                "error": {"code": "comparison_failed", "detail": str(exc)},
            }
    return pairs


def _measurement_status(providers, pairs):
    states = [p["status"] for p in [*providers.values(), *pairs.values()]]
    if "error" in states:
        return "error"
    if "skipped" in states or sum(p["status"] == "ok" for p in providers.values()) < 2:
        return "partial"
    return "complete"


def _snapshot(source, stage):
    expected = digest(source)
    folder = stage / "input"
    folder.mkdir()
    captured = folder / source.name
    shutil.copyfile(source, captured)
    if digest(captured) != expected or digest(source) != expected:
        raise ProviderError(
            "inputs_changed", "Source changed while capturing the common input"
        )
    return captured, expected


def execute(source, output, cache, settings, options=None):
    options = options or MatrixSpec()
    audit_source = options.pages.count is None
    source, output, cache, options = _matrix_inputs(source, output, cache, options)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".parity-", dir=output.parent) as staging:
        stage = Path(staging)
        captured, source_hash = _snapshot(source, stage)
        if (
            audit_source
            and source_slide_count(captured, options.pages) != options.pages.count
        ):
            raise ProviderError(
                "inputs_changed", "Source changed while validating slide counts"
            )
        successful, providers = _render_jobs(captured, cache, settings, options, stage)
        if digest(captured) != source_hash:
            raise ProviderError(
                "inputs_changed", "A provider changed the common captured input"
            )
        pairs = _compare_inputs(successful, stage)
        report = {
            "schema_version": 1,
            "status": _measurement_status(providers, pairs),
            "verdict": "measurement_only",
            "source": {
                "path": str(source),
                "captured_input": str(captured.relative_to(stage)),
                "sha256": source_hash,
                "slides": options.pages.count,
            },
            "providers": providers,
            "pairs": pairs,
            "qualifications": [
                "Engine agreement is not PowerPoint certification or a recommendation.",
                "Reference origin/revision labels and declared font inputs are operator supplied.",
                "Executable hashes do not attest dynamic libraries, system fallback fonts or operating-system rasterization.",
            ],
        }
        (stage / "report.json").write_text(
            json.dumps(report, indent=2, ensure_ascii=False)
        )
        if output.exists():
            raise ProviderError(
                "output_exists", f"Evidence directory appeared during run: {output}"
            )
        stage.rename(output)
    return report


def _legacy_reference(pdf, meta, source, options):
    if meta.exists() and not pdf.exists():
        raise ProviderError(
            "reference_corrupt", "Reference metadata exists but its PDF is missing"
        )
    if not pdf.exists() or meta.exists():
        return False
    if (
        options.renderer is not None
        or options.reference is not None
        or pdf.stat().st_mtime < source.stat().st_mtime
    ):
        raise ProviderError(
            "reference_exists",
            "Existing legacy reference is unverified; choose a new --case-prefix rather than overwrite it",
        )
    return True


def _check_snapshot(pdf, meta, source, result):
    try:
        stored = json.loads(meta.read_text())
        changed = (
            stored["key"] != result["key"]
            or stored["source_sha256"] != digest(source)
            or stored["pdf_sha256"] != digest(pdf)
        )
    except (OSError, ValueError, KeyError, TypeError) as exc:
        raise ProviderError(
            "reference_corrupt", f"Invalid reference snapshot: {exc}"
        ) from exc
    if changed:
        raise ProviderError(
            "reference_changed",
            "Reference/provenance changed; choose a new --case-prefix. --force recreates cases, not references.",
        )


def _publish_reference(source, pdf, meta, result, renderer):
    with tempfile.TemporaryDirectory(prefix=".reference-", dir=pdf.parent) as temporary:
        temporary = Path(temporary)
        shutil.copyfile(
            Path(result["directory"]) / "source.pdf", temporary / "original.pdf"
        )
        (temporary / "reference.json").write_text(
            json.dumps(
                {
                    "renderer": renderer,
                    "key": result["key"],
                    "source_sha256": digest(source),
                    "pdf_sha256": digest(temporary / "original.pdf"),
                    "provenance": result,
                },
                indent=2,
            )
        )
        try:
            os.link(temporary / "original.pdf", pdf)
            os.link(temporary / "reference.json", meta)
        except FileExistsError as exc:
            raise ProviderError(
                "reference_exists",
                "Reference appeared during publication; prior files were not overwritten",
            ) from exc


def ingest_reference(source, deck_dir, cache, options=None):
    """Keep the case reference immutable, including unverified legacy snapshots."""
    options = options or ReferenceSpec()
    if options.reference is not None and (
        options.renderer is not None or options.override is not None
    ):
        raise ProviderError(
            "arguments", "Choose a supplied reference PDF or a renderer, not both"
        )
    source = Path(source).resolve()
    deck_dir = private_destination(deck_dir)
    pdf, meta = deck_dir / "original.pdf", deck_dir / "reference.json"
    if _legacy_reference(pdf, meta, source, options):
        return pdf
    selected = (
        options.reference_engine
        if options.reference
        else options.renderer or "libreoffice"
    )
    settings = Settings.load(options.config)
    spec = RenderSpec(options.pages, options.override)
    result = render_pdf(
        selected,
        Path(options.reference).expanduser() if options.reference else source,
        private_destination(cache),
        settings,
        spec,
    )
    if pdf.exists():
        _check_snapshot(pdf, meta, source, result)
    else:
        deck_dir.mkdir(parents=True, exist_ok=True)
        _publish_reference(source, pdf, meta, result, selected)
    return pdf


def add_parser(subparsers, lab):
    p = subparsers.add_parser(
        "matrix",
        help="Compare optional PPTX renderers and a supplied Chrome/PowerPoint PDF",
    )
    p.add_argument("--pptx", required=True)
    p.add_argument("--output", required=True)
    p.add_argument("--cache", default=str(lab / "cache/providers"))
    p.add_argument("--engines", default="deckcraft,libreoffice")
    p.add_argument(
        "--no-cache",
        action="store_true",
        help="Force fresh renders for dynamic or uncontrolled runtime state",
    )
    p.add_argument("--reference-pdf")
    p.add_argument(
        "--reference-engine",
        choices=["reference", "chrome", "powerpoint"],
        default="reference",
    )
    p.add_argument("--reference-version", default="unspecified")
    p.add_argument("--config", type=Path)
    p.add_argument("--deckcraft-bin")
    p.add_argument("--libreoffice-bin")
    p.add_argument("--timeout-secs", type=float)
    p.add_argument("--font-file", action="append", default=None)
    p.add_argument(
        "--expect-font",
        action="append",
        default=None,
        help="Exact subset-prefix-stripped PostScript face; repeat per expected face",
    )
    p.add_argument("--width", type=float, default=960)
    p.add_argument("--height", type=float, default=540)
    p.add_argument("--dpi", type=int, default=100)
    p.set_defaults(fn=command)


def command(args):
    try:
        settings = Settings.load(args.config)
        if args.no_cache:
            settings.data["cache_enabled"] = False
        if args.timeout_secs is not None:
            if not 0 < args.timeout_secs <= 3600:
                raise ProviderError(
                    "arguments", "Timeout must be > 0 and <= 3600 seconds"
                )
            settings.data["timeout_seconds"] = args.timeout_secs
        for flag, key in [(args.font_file, "files"), (args.expect_font, "expected")]:
            if flag is not None:
                settings.data["fonts"][key] = flag
        options = MatrixSpec(
            engines=tuple(e.strip() for e in args.engines.split(",")),
            reference=args.reference_pdf,
            reference_engine=args.reference_engine,
            reference_version=args.reference_version,
            overrides={
                k: v
                for k, v in [
                    ("deckcraft", args.deckcraft_bin),
                    ("libreoffice", args.libreoffice_bin),
                ]
                if v is not None
            },
            pages=PageSpec(width=args.width, height=args.height, dpi=args.dpi),
        )
        result = execute(args.pptx, args.output, args.cache, settings, options)
        print(
            json.dumps(
                {
                    "status": result["status"],
                    "verdict": result["verdict"],
                    "report": str(
                        Path(args.output).expanduser().resolve() / "report.json"
                    ),
                    "providers": {
                        k: v["status"] for k, v in result["providers"].items()
                    },
                }
            )
        )
        if result["status"] != "complete":
            raise SystemExit(2 if result["status"] == "partial" else 1)
    except (ProviderError, OSError, ValueError) as exc:
        print(
            json.dumps(
                {
                    "status": "error",
                    "error": {"code": getattr(exc, "code", "io"), "detail": str(exc)},
                }
            )
        )
        raise SystemExit(1)
