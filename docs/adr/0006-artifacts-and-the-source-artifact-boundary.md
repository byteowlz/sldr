# Artifacts: directory by default, single file by request, .sldr bundle for the work itself

**Source-land vs artifact-land.** In sources, media lives anywhere — a media library directory, the curation platform — and slides reference it like they reference layouts and flavors. The build boundary freezes this: the compiler resolves every reference, and a built artifact never points back into the machine or the platform. Self-contained means **offline-capable with zero runtime services** (the podium test); it does not mean one file.

**Artifact tiers:**

1. **Presentation directory (default).** `index.html` plus media siblings referenced relatively. This is the browser's *native* equivalent of "HTML + media": video streams and seeks over `file://` with no JS, no server, no size ceiling. Zip it to mail it.
2. **Single HTML file (`--single-file`).** All media inlined as data URIs — the universal handoff: recipient needs only a browser. Explicitly requested, because it carries real costs the default shouldn't impose silently: ~+33% media weight, and data-URI video must materialize fully in memory (viable to a few tens of MB, then playback fails). The build warns at the ceiling.
3. **`.sldr` bundle.** A plain zip with a unique extension, containing **sources**: the playlist, its slides, their layouts, the flavor, the media — optionally with the baked HTML cached inside (the build is deterministic, so the cache is convenience, never truth). Recipient needs sldr or the platform, and in exchange gets the work itself: restylable, editable, curatable on arrival. This is the exchange format between machines and the curation platform.

**Why `.sldr`, not `.zip`:** a renamed zip never stops being a zip (`unzip talk.sldr` works; the spec says so in its first paragraph), so universality is preserved — while the extension buys OS file association, a MIME type, semantic identity, and validatable structure. Precedent is unanimous: .pptx, .docx, .epub, .jar, .apk are all zips and none shipped as .zip.

**Rejected: browser-native single-file-with-media formats.** MHTML (Chrome-lineage only, still base64 inside), Safari .webarchive (proprietary), Web Bundles (retreated into isolated-web-app tooling). The web platform never shipped this format; data URIs in HTML are its actual answer.

**Future, additive only:** the HTML/ZIP polyglot — one file that browsers present (tier 2 baked HTML) and sldr unpacks (appended kilobyte-scale source zip; media recovered from the HTML's own data URIs, which determinism makes a faithful store). Universal and complete in one file; inherits tier 2's inline ceiling; never foundational, droppable at any time.

## Consequences

- Docs and README must stop headlining "a single HTML file" as the default output; the headline property is the podium test, with single-file as the universality option.
- The deterministic build is what makes baked HTML a faithful media store — another reason byte-identical builds (trx-36e5) are a contract, not a nicety.
- Media recovered from a bundle's baked HTML is build-optimized, not original; originals live in git and the platform.
