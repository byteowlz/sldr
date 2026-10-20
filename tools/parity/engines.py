"""Optional trusted CLI providers. No office libraries, model calls or automatic installs."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import hashlib
import errno
import json
import os
import platform
import re
import shutil
import signal
import subprocess
import tempfile
import sys
from importlib.metadata import version
import tomllib
import uuid

from pdf_metrics import PageSpec, PdfError, prepare_pdf
from pypdf.errors import PyPdfError

DEFAULTS = """#:schema https://raw.githubusercontent.com/byteowlz/sldr/main/tools/parity/config.schema.json
# Explicit --config values precede environment variables; local/global defaults do not.
timeout_seconds = 60
cache_enabled = true
[providers.deckcraft]
binary = ""
revision = "unspecified"
sha256 = ""
home = ""
[providers.libreoffice]
binary = ""
revision = "unspecified"
sha256 = ""
home = ""
[tools]
pdftoppm = ""
pdftotext = ""
pdffonts = ""
[fonts]
files = []
expected = []
"""


_BINARIES = {
    "deckcraft": ("pptx-cli", "pptxr", "deckcraft-cli"),
    "libreoffice": ("soffice",),
    "pdftoppm": ("pdftoppm",),
    "pdftotext": ("pdftotext",),
    "pdffonts": ("pdffonts",),
}


class ProviderError(RuntimeError):
    def __init__(self, code, detail):
        super().__init__(detail)
        self.code = code


def digest(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def json_key(value):
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def _merge(base, values, prefix=()):
    for key, value in values.items():
        if key not in base:
            raise ProviderError(
                "config", f"Unknown config key: {'.'.join((*prefix, key))}"
            )
        if isinstance(base[key], dict):
            if not isinstance(value, dict):
                raise ProviderError("config", f"Expected table: {key}")
            _merge(base[key], value, (*prefix, key))
        elif type(value) is not type(base[key]) and not (
            key == "timeout_seconds"
            and isinstance(value, (int, float))
            and not isinstance(value, bool)
        ):
            raise ProviderError("config", f"Invalid value type: {key}")
        else:
            base[key] = value


@dataclass
class Settings:
    data: dict
    explicit: dict

    @classmethod
    def load(cls, config=None):
        data = tomllib.loads(DEFAULTS)
        explicit = {}
        if config is not None:
            paths = [Path(config).expanduser()]
        else:
            global_file = (
                Path(os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config")))
                / "sldr-parity/config.toml"
            )
            global_file.parent.mkdir(parents=True, exist_ok=True)
            try:
                with global_file.open("x") as stream:
                    stream.write(DEFAULTS)
            except FileExistsError:
                pass
            paths = [global_file]
            if Path("parity.toml").is_file():
                paths.append(Path("parity.toml"))
        for path in paths:
            try:
                values = tomllib.loads(path.read_text())
            except (OSError, ValueError) as exc:
                raise ProviderError(
                    "config", f"Cannot read config {path}: {exc}"
                ) from exc
            _merge(data, values)
            if config is not None:
                explicit = values
        if not 0 < data["timeout_seconds"] <= 3600:
            raise ProviderError("config", "timeout_seconds must be > 0 and <= 3600")
        if any(
            not isinstance(v, str)
            for key in ("files", "expected")
            for v in data["fonts"][key]
        ):
            raise ProviderError(
                "config", "Font files/expected must be arrays of strings"
            )
        if any(
            p["sha256"] and not re.fullmatch(r"[a-fA-F0-9]{64}", p["sha256"])
            for p in data["providers"].values()
        ):
            raise ProviderError(
                "config", "Provider sha256 must be empty or 64 hexadecimal characters"
            )
        return cls(data, explicit)

    def binary(self, name, override=None):
        group = "providers" if name in self.data["providers"] else "tools"
        table = self.data[group]
        if group == "providers":
            selected = table[name]["binary"]
            explicit = "binary" in self.explicit.get(group, {}).get(name, {})
        else:
            selected = table[name]
            explicit = name in self.explicit.get(group, {})
        if override is not None and not override.strip():
            raise ProviderError(
                "executable_missing", "Explicit executable override is empty"
            )
        choice = (
            override
            if override is not None
            else selected
            if explicit and selected
            else os.environ.get("PARITY_" + name.upper() + "_BIN") or selected
        )
        return _locate_binary(name, choice)


def _locate_binary(name, choice):
    if choice:
        expanded = os.path.expandvars(os.path.expanduser(choice))
        found = shutil.which(expanded)
        if not found:
            raise ProviderError(
                "executable_missing",
                f"Selected executable is missing/non-executable: {choice}",
            )
    else:
        found = next((p for d in _BINARIES[name] if (p := shutil.which(d))), None)
        if not found:
            raise ProviderError(
                "unavailable",
                f"Optional executable unavailable: {name} ({', '.join(_BINARIES[name])})",
            )
    return Path(found).resolve()


def _stop(child):
    try:
        if os.name == "posix":
            os.killpg(child.pid, signal.SIGKILL)
        else:
            child.kill()
    except ProcessLookupError:
        pass  # The command/group already exited; still reap the direct child below.
    child.wait()


def process(argv, timeout, env=None):
    """Bound runtime and capture size; kill this command's process group, never the host session."""
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            child = subprocess.Popen(
                [str(a) for a in argv],
                stdout=stdout,
                stderr=stderr,
                env=env,
                start_new_session=(os.name == "posix"),
            )
            try:
                child.wait(timeout=timeout)
            except BaseException as exc:
                _stop(child)
                if isinstance(exc, subprocess.TimeoutExpired):
                    raise ProviderError(
                        "timeout",
                        f"Provider/tool timed out after {timeout}s: {argv[0]}",
                    ) from exc
                raise
        except OSError as exc:
            raise ProviderError(
                "provider_start", f"Cannot start executable {argv[0]}: {exc}"
            ) from exc
        stdout.seek(0)
        stderr.seek(0)
        output = stdout.read(2 * 1024 * 1024 + 1)
        if len(output) > 2 * 1024 * 1024:
            raise ProviderError("output_limit", f"Tool stdout exceeds 2 MiB: {argv[0]}")
        out, err = (
            output.decode(errors="replace"),
            stderr.read(8192).decode(errors="replace"),
        )
        if child.returncode:
            raise ProviderError(
                "provider_failed", f"{argv[0]} exited {child.returncode}: {err or out}"
            )
        return out, err


def toolchain(settings, env=None):
    result = {}
    for name in ("pdftoppm", "pdftotext", "pdffonts"):
        try:
            path = settings.binary(name)
        except ProviderError as exc:
            if exc.code == "unavailable":
                raise ProviderError("tool_unavailable", str(exc)) from exc
            raise
        out, err = process([path, "-v"], settings.data["timeout_seconds"], env)
        result[name] = {
            "path": str(path),
            "sha256": digest(path),
            "version": (out or err).strip(),
        }
    return result


def font_inputs(settings):
    result = []
    for filename in settings.data["fonts"]["files"]:
        path = Path(os.path.expandvars(filename)).expanduser().resolve()
        if not path.is_file():
            raise ProviderError("font_missing", f"Declared font file missing: {path}")
        result.append({"path": str(path), "sha256": digest(path)})
    return result


def _checked_cache(folder, key):
    try:
        if folder.is_symlink():
            raise ValueError("symlink cache directory")
        manifest = json.loads((folder / "manifest.json").read_text())
        if manifest["key"] != key or json_key(manifest["identity"]) != key:
            raise ValueError("key mismatch")
        count = manifest["page_count"]
        if type(count) is not int or not 1 <= count <= 200:
            raise ValueError("invalid page count")
        required = {"source.pdf", "normalized.pdf", "lines.json", "text.xml"}
        required.update(
            f"{kind}{i}.png" for kind in ("gray", "rgb") for i in range(count)
        )
        if not required.issubset(manifest["artifacts"]):
            raise ValueError("missing artifact inventory")
        for name, expected in manifest["artifacts"].items():
            path = folder / name
            if (
                path.parent != folder
                or path.is_symlink()
                or not path.is_file()
                or digest(path) != expected
            ):
                raise ValueError(f"artifact changed: {name}")
        return manifest
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as exc:
        raise ProviderError(
            "cache_corrupt", f"Invalid cache at {folder}: {exc}"
        ) from exc


def _result(folder, manifest, hit):
    return {
        **manifest,
        "pdf": str(folder / "normalized.pdf"),
        "directory": str(folder),
        "cache_hit": hit,
    }


@dataclass(frozen=True)
class RenderSpec:
    pages: PageSpec = PageSpec()
    override: str | None = None
    reference_version: str = "unspecified"


@dataclass
class RenderProfile:
    identity: dict
    env: dict
    timeout: float

    def call(self, argv):
        return process(argv, self.timeout, self.env)


def _provider(engine, settings, spec, env):
    if engine in ("deckcraft", "libreoffice"):
        config = settings.data["providers"][engine]
        path = settings.binary(engine, spec.override)
        binary_hash = digest(path)
        if config["sha256"] and binary_hash != config["sha256"].lower():
            raise ProviderError(
                "binary_hash", f"Provider binary hash mismatch: {engine}"
            )
        if config["home"]:
            env["HOME"] = str(Path(config["home"]).expanduser().resolve())
        out, err = process([path, "--version"], settings.data["timeout_seconds"], env)
        return {
            "engine": engine,
            "binary": str(path),
            "sha256": binary_hash,
            "version": (out or err).strip(),
            "revision": config["revision"],
            "revision_origin": "operator_supplied_label",
        }
    return {
        "engine": engine,
        "version": spec.reference_version,
        "origin": "supplied_pdf",
        "origin_verified": False,
    }


def _profile(engine, source, settings, spec):
    env = dict(os.environ)
    provider = _provider(engine, settings, spec, env)
    tools = toolchain(settings, env)
    fonts = font_inputs(settings)
    runtime_env = {
        k: env.get(k, "")
        for k in (
            "HOME",
            "FONTCONFIG_FILE",
            "FONTCONFIG_PATH",
            "SAL_FONTPATH",
            "XDG_DATA_HOME",
            "XDG_DATA_DIRS",
            "LANG",
            "LC_ALL",
            "LC_TIME",
            "TZ",
        )
    }
    identity = {
        "schema_version": 1,
        "input_sha256": digest(source),
        "input_name": source.name,
        "provider": provider,
        "tools": tools,
        "fonts": fonts,
        "expected_fonts": settings.data["fonts"]["expected"],
        "environment": runtime_env,
        "platform": platform.platform(),
        "runtime": {
            "python": sys.version,
            "packages": {p: version(p) for p in ("numpy", "pillow", "pypdf")},
        },
        "timeout_seconds": settings.data["timeout_seconds"],
        "cache_enabled": settings.data["cache_enabled"],
        "geometry": [
            float(spec.pages.width),
            float(spec.pages.height),
            int(spec.pages.dpi),
        ],
        "expected_pages": spec.pages.count,
        "adapter_sha256": digest(__file__),
        "metrics_sha256": digest(Path(__file__).with_name("pdf_metrics.py")),
    }
    return RenderProfile(identity, env, settings.data["timeout_seconds"])


def _produce_pdf(engine, source, stage, profile):
    produced = stage / "source.pdf"
    binary = profile.identity["provider"].get("binary")
    if engine == "deckcraft":
        profile.call([binary, "convert", source, produced])
    elif engine == "libreoffice":
        profile.call(
            [
                binary,
                "-env:UserInstallation=" + (stage / "lo-profile").as_uri(),
                "--headless",
                "--convert-to",
                "pdf",
                "--outdir",
                stage,
                source,
            ]
        )
        candidate = stage / (source.stem + ".pdf")
        if candidate != produced:
            if candidate.is_symlink() or not candidate.is_file():
                raise ProviderError(
                    "provider_output", "LibreOffice produced no regular PDF"
                )
            candidate.rename(produced)
    else:
        shutil.copyfile(source, produced)
    if produced.is_symlink() or not produced.is_file() or produced.stat().st_size == 0:
        raise ProviderError(
            "provider_output", "Provider produced no regular nonempty PDF"
        )
    return produced


def _verify_inputs(source, metadata, identity):
    missing = sorted(
        set(identity["expected_fonts"]) - {f["name"] for f in metadata["fonts"]}
    )
    if missing:
        observed = sorted({f["name"] for f in metadata["fonts"]})
        raise ProviderError(
            "font_mismatch",
            f"Expected PDF faces missing (possible substitution): {', '.join(missing)}; observed: {', '.join(observed[:32]) or 'none'}",
        )
    stable = [*identity["fonts"], *identity["tools"].values()]
    provider = identity["provider"]
    if "binary" in provider:
        stable.append({"path": provider["binary"], "sha256": provider["sha256"]})
    if digest(source) != identity["input_sha256"] or any(
        digest(item["path"]) != item["sha256"] for item in stable
    ):
        raise ProviderError(
            "inputs_changed",
            "Input, executable or declared fonts changed during rendering",
        )


def _publish_render(stage, folder, manifest):
    manifest["artifacts"] = {p.name: digest(p) for p in stage.iterdir() if p.is_file()}
    (stage / "manifest.json").write_text(json.dumps(manifest, indent=2))
    # Remove only the isolated LibreOffice profile, not any caller's runtime state.
    shutil.rmtree(stage / "lo-profile", ignore_errors=True)
    try:
        stage.rename(folder)
    except OSError as exc:
        if exc.errno not in (errno.EEXIST, errno.ENOTEMPTY):
            raise
        return _result(folder, _checked_cache(folder, manifest["key"]), True)
    return _result(folder, manifest, False)


def render_pdf(engine, source, cache, settings, spec=RenderSpec()):
    """Verified immutable render object out; failures never publish cache entries."""
    source, cache = Path(source).resolve(), Path(cache).resolve()
    if not source.is_file():
        raise ProviderError("input", f"Input missing: {source}")
    profile = _profile(engine, source, settings, spec)
    identity = profile.identity
    key = json_key(identity)
    # Without a declared font pack and expected faces, don't reuse host-font-dependent renders.
    reusable = identity["cache_enabled"] and bool(
        identity["fonts"] and identity["expected_fonts"]
    )
    folder = cache / (key if reusable else key + "-" + uuid.uuid4().hex)
    cache.mkdir(parents=True, exist_ok=True, mode=0o700)
    if folder.exists() or folder.is_symlink():
        return _result(folder, _checked_cache(folder, key), True)
    with tempfile.TemporaryDirectory(prefix=".render-", dir=cache) as stage:
        stage = Path(stage)
        produced = _produce_pdf(engine, source, stage, profile)
        try:
            metadata = prepare_pdf(
                produced, stage, identity["tools"], profile.call, spec.pages
            )
        except PdfError as exc:
            raise ProviderError(exc.code, str(exc)) from exc
        except (ValueError, OSError, PyPdfError) as exc:
            raise ProviderError("invalid_pdf", str(exc)) from exc
        _verify_inputs(source, metadata, identity)
        manifest = {
            **metadata,
            "key": key,
            "identity": identity,
            "origin": identity["provider"].get("origin", "rendered_here"),
            "font_verification": "declared_faces_present"
            if identity["expected_fonts"]
            else "unverified",
            "cacheable": reusable,
        }
        return _publish_render(stage, folder, manifest)
