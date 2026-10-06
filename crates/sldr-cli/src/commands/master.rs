//! `sldr master <file.pptx>`: inventory a PowerPoint master for template-backed
//! export (trx-4s9s.11). Read-only; the master is never modified or fetched from.

use anyhow::{Context, Result};
use colored::Colorize;

pub fn run(file: &str, skeleton: bool, json: bool) -> Result<()> {
    let bytes = std::fs::read(file).with_context(|| format!("cannot read {file}"))?;
    let inv = sldr_pptx::inspect_master(&bytes)?;
    if skeleton {
        let name = std::path::Path::new(file).file_name().and_then(|n| n.to_str()).unwrap_or(file);
        print!("{}", sldr_pptx::map_skeleton(&inv, name));
        return Ok(());
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&inv)?);
        return Ok(());
    }
    let (w, h) = inv.slide_size;
    println!(
        "{} {}  {:.2}\" x {:.2}\"  {} slide(s) (not carried over on export)",
        "Master".green().bold(),
        file.cyan(),
        w as f64 / 914_400.0,
        h as f64 / 914_400.0,
        inv.slides
    );
    for m in &inv.masters {
        println!(
            "\n{} {}  theme {:?}  fonts {:?} / {:?}",
            "slide_master =".bold(),
            m.index,
            m.theme.as_deref().unwrap_or(""),
            m.heading_font.as_deref().unwrap_or(""),
            m.body_font.as_deref().unwrap_or("")
        );
        for l in &m.layouts {
            println!("  {}", format!("{:?}", l.name).yellow());
            for p in &l.placeholders {
                let bbox = p
                    .bbox
                    .map(|[x, y, w, h]| format!("at {x:.1},{y:.1}  {w:.1}x{h:.1}%"))
                    .unwrap_or_else(|| "inherits master geometry".into());
                println!("    {:<8} idx {:<3} {:<28} {}", p.kind, p.idx, p.name, bbox.dimmed());
            }
        }
    }
    println!("\nMap sldr layouts onto these with `sldr master {file} --skeleton > map.toml`.");
    Ok(())
}
