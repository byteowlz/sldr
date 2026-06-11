//! Determinism contract tests (trx-36e5, ADR-0003/0006).
//!
//! Two invariants the rest of the architecture leans on:
//!
//! 1. Same inputs → byte-identical HTML. Agents verify changes by diffing,
//!    builds are cacheable, and a `.sldr` bundle's baked HTML is a faithful
//!    media store only because rebuilds reproduce it exactly.
//! 2. The one-command restyle guarantee: building with flavor A vs flavor B
//!    may differ only in the flavor style layer (`<style data-flavor>`
//!    blocks and flavor-declared font links), never in content markup.
//!
//! Known confinement exception: a flavor's `[code] syntax_theme` feeds
//! syntect, which emits inline-styled spans *inside* content markup. These
//! tests therefore use flavors with identical (default) syntax themes; the
//! leak itself is tracked separately (class-based highlighting).

use sldr_core::flavor::Flavor;
use sldr_renderer::render_sample;

fn test_flavor(name: &str, primary: &str, body_font: &str) -> Flavor {
    toml::from_str(&format!(
        r##"
name = "{name}"

[colors]
primary = "{primary}"
background = "#ffffff"
text = "#111111"

[typography]
body_font = "{body_font}"
"##
    ))
    .expect("test flavor TOML must parse")
}

/// Remove every `<style data-flavor="...">...</style>` block — the style
/// layer a flavor is allowed to own.
fn strip_flavor_styles(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<style data-flavor=") {
        out.push_str(&rest[..start]);
        // Consume surrounding whitespace too, so one stripped block and two
        // stripped blocks leave identical residue.
        out.truncate(out.trim_end().len());
        let after = &rest[start..];
        let end = after
            .find("</style>")
            .expect("flavor style block must be closed");
        rest = after[end + "</style>".len()..].trim_start();
    }
    out.push_str(rest);
    out
}

/// Assert equality, reporting a window around the first divergence instead
/// of dumping both full documents.
fn assert_same(a: &str, b: &str, msg: &str) {
    if a == b {
        return;
    }
    let pos = a
        .bytes()
        .zip(b.bytes())
        .position(|(x, y)| x != y)
        .unwrap_or_else(|| a.len().min(b.len()));
    let lo = pos.saturating_sub(120);
    let a_win = &a[lo..(pos + 120).min(a.len())];
    let b_win = &b[lo..(pos + 120).min(b.len())];
    panic!("{msg}\nfirst divergence at byte {pos}:\n--- a ---\n{a_win}\n--- b ---\n{b_win}");
}

#[test]
fn rebuild_is_byte_identical() {
    let a = render_sample(test_flavor("alpha", "#ff2200", "DM Sans"), &[]).unwrap();
    let b = render_sample(test_flavor("alpha", "#ff2200", "DM Sans"), &[]).unwrap();
    assert_eq!(a, b, "two builds of identical inputs must be byte-identical");
}

#[test]
fn flavor_swap_diff_is_confined_to_style_layer() {
    let red = render_sample(test_flavor("alpha", "#ff2200", "DM Sans"), &[]).unwrap();
    let blue = render_sample(test_flavor("beta", "#0022ff", "Instrument Serif"), &[]).unwrap();

    // Sanity: the builds actually differ before stripping.
    assert_ne!(red, blue, "different flavors must produce different output");

    let red_stripped = strip_flavor_styles(&red);
    let blue_stripped = strip_flavor_styles(&blue);

    assert_same(
        &red_stripped,
        &blue_stripped,
        "outside the <style data-flavor> layer the outputs must be \
         byte-identical — a flavor that changes content markup breaks the \
         one-command restyle guarantee",
    );
}

#[test]
fn multi_flavor_embed_keeps_content_identical_to_single() {
    // Embedding extra flavors for the runtime switcher must add style
    // blocks (and possibly font links), never touch content markup.
    let single = render_sample(test_flavor("alpha", "#ff2200", "DM Sans"), &[]).unwrap();
    let multi = render_sample(
        test_flavor("alpha", "#ff2200", "DM Sans"),
        &[test_flavor("beta", "#0022ff", "DM Sans")],
    )
    .unwrap();

    assert_same(
        &strip_flavor_styles(&single),
        &strip_flavor_styles(&multi),
        "embedding extra flavors must not alter anything outside the style layer",
    );
}
