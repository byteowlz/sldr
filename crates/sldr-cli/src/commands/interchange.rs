//! CLI policy/publication only. Fidelity decisions live in sldr-pptx.
use std::{io::Write, path::{Path, PathBuf}};
use anyhow::{Context, Result};

#[derive(clap::Args, Debug, Default)]
pub struct Options {
    /// Reject any reported loss (the default for native PPTX interchange)
    #[arg(long, conflicts_with = "allow_lossy")]
    pub strict: bool,
    /// Explicitly permit reported omissions/baking; conflicts still fail
    #[arg(long)]
    pub allow_lossy: bool,
    /// Write the versioned fidelity report as JSON, including strict failures
    #[arg(long, value_name = "FILE")]
    pub report_json: Option<PathBuf>,
}
impl Options {
    pub fn resolve<T>(&self, result: Result<sldr_pptx::Conversion<T>>, destination: &Path) -> Result<T> {
        let conversion = self.diagnose(result, destination)?;
        self.finish(&conversion.report, destination)?;
        Ok(conversion.value)
    }

    pub fn diagnose<T>(&self, result: Result<T>, destination: &Path) -> Result<T> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                let report = if let Some(rejected) = error.downcast_ref::<sldr_pptx::Rejected>() {
                    rejected.0.clone()
                } else {
                    let mut report = sldr_pptx::Report::default();
                    report.record(None, "package", "preflight", "invalid_input", sldr_pptx::Disposition::Conflicting,
                        &format!("Resolve the invalid input: {error:#}"));
                    report
                };
                self.finish(&report, destination)?;
                Err(error)
            }
        }
    }

    pub fn finish(&self, report: &sldr_pptx::Report, destination: &Path) -> Result<()> {
        if let Some(path) = &self.report_json {
            // A report must never overwrite, or appear inside, the destination
            // whose immutability strict failure promises.
            // Resolve through the deepest existing ancestor: neither the
            // report nor the destination has to exist yet (a fresh --out
            // directory is the normal case), and canonicalizing a missing
            // parent used to fail with a bare "No such file or directory".
            let absolute = |p: &Path| -> Result<PathBuf> {
                let p = std::path::absolute(p)?;
                let mut base = p.as_path();
                let mut rest = Vec::new();
                while !base.exists() {
                    rest.push(base.file_name().context("path has no name")?.to_owned());
                    base = base.parent().context("path has no existing ancestor")?;
                }
                let mut out = base.canonicalize()?;
                out.extend(rest.iter().rev());
                Ok(out)
            };
            let report_path = absolute(path)?;
            let dest_path = absolute(destination)?;
            if report_path == dest_path || report_path.starts_with(&dest_path) {
                anyhow::bail!("report path must be outside the output destination");
            }
            atomic_write(path, &serde_json::to_vec_pretty(report)?)?;
        }
        eprint!("{report}");
        report.enforce(self.allow_lossy)
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(bytes)?;
    staged.as_file().sync_all()?;
    staged.persist(path).with_context(|| format!("Failed to publish {}", path.display()))?;
    Ok(())
}
