//! Synthetic print regression deck. No network assets or proprietary fonts.
use sldr_core::{flavor::Flavor, slide::Slide};
use sldr_renderer::{HtmlRenderer, RenderConfig};

fn main() -> anyhow::Result<()> {
    let mut flavor = String::from("name = 'print-proof'\n[colors]\nbackground = '#ffffff'\ntext = '#111111'\n[typography]\nheading_font = 'Arial'\nbody_font = 'Arial'\n");
    for i in 0..5 {
        flavor.push_str(&format!("\n[[logos]]\nfile = 'data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxMDAiIGhlaWdodD0iNDAiPjxyZWN0IHdpZHRoPSIxMDAiIGhlaWdodD0iNDAiIGZpbGw9IiM0NDg4Y2MiLz48L3N2Zz4='\nlayouts = ['all']\nwidth = '80px'\nx = '{}px'\ny = '1020px'\n", 30 + i * 100));
    }
    let flavor: Flavor = toml::from_str(&flavor)?;
    let mut renderer = HtmlRenderer::new(RenderConfig {
        title: "Synthetic print proof".into(),
        languages: vec!["en".into(), "de".into()],
        default_language: "en".into(),
        ..Default::default()
    }).add_flavor(flavor);
    for (i, layout) in ["framed", "framed-cols"].iter().enumerate() {
        let body = if i == 0 { "Body sentinel" } else { "::left::\nBody sentinel\n\n::right::\nSecond column" };
        renderer.add_slide(&Slide::from_str(format!("proof-{i}"), format!("proof-{i}.md"), &format!("---\ntitle: Title sentinel\nfooter: Footer sentinel\nlayout: {layout}\ntranslations:\n  de:\n    title: Titelprüfung\n    footer: Fußzeilenprüfung\n---\n{body}\n")))?;
    }
    let path = std::env::args().nth(1).unwrap_or_else(|| "print-proof.html".into());
    std::fs::write(path, renderer.render()?)?;
    Ok(())
}
