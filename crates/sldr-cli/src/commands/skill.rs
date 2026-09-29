//! `sldr skill` — print or install the `use-sldr` agent skill.
//!
//! The skill is embedded in the binary (vendored from byteowlz/skillissues),
//! so what an agent loads always matches the CLI it drives. `install` writes
//! it where agents look for skills and is also how a stale copy is refreshed:
//! a differing file is replaced, the old one kept as `<file>.bak`.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use colored::Colorize;

const FILES: [(&str, &str); 3] = [
    ("SKILL.md", include_str!("../../assets/skill/use-sldr/SKILL.md")),
    ("REFERENCE.md", include_str!("../../assets/skill/use-sldr/REFERENCE.md")),
    ("EXAMPLES.md", include_str!("../../assets/skill/use-sldr/EXAMPLES.md")),
];
const SKILL_NAME: &str = "use-sldr";

/// Print one file of the skill (default `SKILL.md`) to stdout.
pub fn show(file: Option<&str>) -> Result<()> {
    let want = file.unwrap_or("skill").to_lowercase();
    let want = want.trim_end_matches(".md");
    match FILES.iter().find(|(n, _)| n.to_lowercase().trim_end_matches(".md") == want) {
        Some((_, body)) => {
            print!("{body}");
            Ok(())
        }
        None => bail!("No skill file '{want}'. Try: skill, reference, examples"),
    }
}

/// Default destinations: `~/.agents/skills/use-sldr`, plus
/// `~/.claude/skills/use-sldr` when a Claude config dir exists.
fn default_targets() -> Result<Vec<PathBuf>> {
    let home = std::env::var_os("HOME").map(PathBuf::from).context("HOME is not set")?;
    let mut out = vec![home.join(".agents/skills").join(SKILL_NAME)];
    if home.join(".claude").is_dir() {
        out.push(home.join(".claude/skills").join(SKILL_NAME));
    }
    Ok(out)
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Created,
    Unchanged,
    Updated,
}

fn install_into(dir: &Path) -> Result<Vec<(&'static str, Outcome)>> {
    std::fs::create_dir_all(dir).with_context(|| format!("Could not create {}", dir.display()))?;
    let mut out = Vec::new();
    for (name, body) in FILES {
        let path = dir.join(name);
        let outcome = match std::fs::read_to_string(&path) {
            Ok(existing) if existing == body => Outcome::Unchanged,
            Ok(existing) => {
                std::fs::write(dir.join(format!("{name}.bak")), existing)?;
                std::fs::write(&path, body)?;
                Outcome::Updated
            }
            Err(_) => {
                std::fs::write(&path, body)?;
                Outcome::Created
            }
        };
        out.push((name, outcome));
    }
    Ok(out)
}

pub fn install(dirs: &[PathBuf]) -> Result<()> {
    let targets = if dirs.is_empty() { default_targets()? } else { dirs.to_vec() };
    for dir in &targets {
        let results = install_into(dir)?;
        println!("{} {}", "skill".green().bold(), dir.display().to_string().cyan());
        for (name, o) in results {
            let note = match o {
                Outcome::Created => "installed".green().to_string(),
                Outcome::Unchanged => "up to date".dimmed().to_string(),
                Outcome::Updated => format!("updated (previous kept as {name}.bak)").yellow().to_string(),
            };
            println!("  {name:<14} {note}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_skill_has_frontmatter_name() {
        assert!(FILES[0].1.starts_with("---\nname: use-sldr\n"));
    }

    #[test]
    fn install_creates_then_is_idempotent_then_backs_up_edits() {
        let dir = std::env::temp_dir().join(format!("sldr-skill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let first = install_into(&dir).unwrap();
        assert!(first.iter().all(|(_, o)| *o == Outcome::Created));
        let again = install_into(&dir).unwrap();
        assert!(again.iter().all(|(_, o)| *o == Outcome::Unchanged));
        std::fs::write(dir.join("SKILL.md"), "local edit").unwrap();
        let third = install_into(&dir).unwrap();
        assert_eq!(third[0].1, Outcome::Updated);
        assert_eq!(std::fs::read_to_string(dir.join("SKILL.md.bak")).unwrap(), "local edit");
        assert_eq!(std::fs::read_to_string(dir.join("SKILL.md")).unwrap(), FILES[0].1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
