//! Versioned interchange diagnostics, shared by CLI and library callers.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    Converted,
    Preserved,
    Baked,
    Unsupported,
    Conflicting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity { Info, Warning, Error }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub slide: Option<String>,
    pub part: String,
    pub element: String,
    pub feature: String,
    pub severity: Severity,
    pub disposition: Disposition,
    pub remedy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: u32,
    pub findings: Vec<Finding>,
}

impl Default for Report {
    fn default() -> Self { Self { schema_version: 1, findings: Vec::new() } }
}

impl Report {
    /// Add one disposition. Locators are package-relative, never local source paths.
    pub fn record(&mut self, slide: Option<&str>, part: &str, element: &str,
                  feature: &str, disposition: Disposition, remedy: &str) {
        let severity = match disposition {
            Disposition::Conflicting => Severity::Error,
            Disposition::Unsupported | Disposition::Baked => Severity::Warning,
            _ => Severity::Info,
        };
        self.findings.push(Finding {
            slide: slide.map(str::to_owned), part: part.into(), element: element.into(),
            feature: feature.into(), severity, disposition, remedy: remedy.into(),
        });
    }

    /// Explicit lossy mode permits reported omissions/baking, never malformed
    /// packages, ambiguous identities, or unsafe publication.
    pub fn enforce(&self, allow_lossy: bool) -> anyhow::Result<()> {
        if self.findings.iter().any(|f| f.disposition == Disposition::Conflicting ||
            (!allow_lossy && matches!(f.disposition, Disposition::Unsupported | Disposition::Baked))) {
            return Err(anyhow::Error::new(Rejected(self.clone())));
        }
        Ok(())
    }
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for item in &self.findings {
            if item.severity != Severity::Info {
                writeln!(f, "{:?}: {}#{}: {} — {}", item.disposition,
                         item.part, item.element, item.feature, item.remedy)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Rejected(pub Report);
impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PPTX conversion rejected; destination unchanged\n{}", self.0)
    }
}
impl std::error::Error for Rejected {}

#[derive(Debug)]
pub struct Conversion<T> {
    pub value: T,
    pub report: Report,
}
