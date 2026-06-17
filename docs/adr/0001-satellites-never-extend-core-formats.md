# Satellites never extend core formats

sldr's durable value is its core: plain-text formats (slides, templates, flavors, skeletons) plus a deterministic compiler. Interactive layers — the server API, the flavor builder, Octo — are disposable satellites. When building the Octo template editor we had to decide whether satellites may add fields to core formats (e.g. drag anchors and `dragPos` values in templates). We decided they may not, ever: a satellite derives everything it reads from existing structure (slots and `sldr-*` classes are already enumerable anchors) and everything it writes must be expressible in the format as it stands (positions are plain CSS, not a new concept).

## Considered Options

- **Satellites may extend formats when the extension is standalone-meaningful** (rejected): the burden-of-proof test is subjective, and each satellite leaves sediment in the formats — once builder-only fields ship, existing files depend on them forever and the lean core erodes one defensible field at a time.
- **Satellites never extend formats** (chosen): a hard rule is enforceable at review time with no judgment call. The cost — the editor may only manipulate what the format already expresses (slot arrangement, alignment, grid; no free pixel positioning) — is acceptable and arguably a feature, since pixel-positioned templates break across aspect ratios.

## Consequences

- Editor-only state (snap grids, selection, UI hints) lives in the satellite's own files, never in core formats.
- A format change is legitimate only if it makes sense to a user who has never seen the satellite proposing it.
- Deleting any satellite must lose no data and no capability, only convenience.
