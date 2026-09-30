//! Shared, lossless budgeting for context and execution-packet callers.
use crate::context::ContextFormat;
use crate::markdown::estimate_tokens;
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionRequirement {
    Required,
    Optional,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSection {
    pub id: String,
    pub title: String,
    pub requirement: SectionRequirement,
    pub order: u32,
    pub sources: Vec<String>,
    pub content: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionOmission {
    pub section_id: String,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedSections {
    pub text: String,
    pub estimated_tokens: usize,
    pub included_section_ids: Vec<String>,
    pub source_ids: Vec<String>,
    pub omitted: Vec<SectionOmission>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetFailure {
    pub budget: usize,
    pub required_tokens: usize,
    pub shortfall: usize,
    pub required_section_ids: Vec<String>,
    pub reason: String,
}
impl fmt::Display for BudgetFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "context packet unavailable: {}; budget={}, required={}, shortfall={} estimated tokens; required sections: {}",
            self.reason,
            self.budget,
            self.required_tokens,
            self.shortfall,
            self.required_section_ids.join(", ")
        )
    }
}
impl std::error::Error for BudgetFailure {}

pub fn render_sections(
    header: &str,
    sections: &[ContextSection],
    format: ContextFormat,
    budget: usize,
) -> Result<RenderedSections, BudgetFailure> {
    let mut ordered = sections.iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| (a.order, &a.id).cmp(&(b.order, &b.id)));
    let mut seen = BTreeSet::new();
    if ordered.iter().any(|s| !seen.insert(&s.id)) {
        return Err(BudgetFailure {
            budget,
            required_tokens: 0,
            shortfall: 0,
            required_section_ids: vec![],
            reason: "duplicate section ID".into(),
        });
    }
    let mut included = ordered
        .iter()
        .map(|s| s.requirement == SectionRequirement::Required)
        .collect::<Vec<_>>();
    let required_section_ids = ordered
        .iter()
        .filter(|s| s.requirement == SectionRequirement::Required)
        .map(|s| s.id.clone())
        .collect::<Vec<_>>();
    let required_tokens = selection_tokens(header, &ordered, &included);
    if required_tokens > budget {
        return Err(BudgetFailure { budget, required_tokens, shortfall: required_tokens - budget, required_section_ids, reason: "required content, source references and omission notices do not fit; increase budget or use a narrower focus".into() });
    }
    for index in 0..ordered.len() {
        if included[index] {
            continue;
        }
        included[index] = true;
        if selection_tokens(header, &ordered, &included) > budget {
            included[index] = false;
        }
    }
    let text = render(header, &ordered, &included, format);
    let estimated_tokens = estimate_tokens(&text);
    let omitted = ordered
        .iter()
        .zip(&included)
        .filter(|(_, yes)| !**yes)
        .map(|(s, _)| SectionOmission {
            section_id: s.id.clone(),
            reason: "budget".into(),
        })
        .collect();
    let included_section_ids = ordered
        .iter()
        .zip(&included)
        .filter(|(_, yes)| **yes)
        .map(|(s, _)| s.id.clone())
        .collect();
    let source_ids = sources(&ordered);
    Ok(RenderedSections {
        text,
        estimated_tokens,
        included_section_ids,
        source_ids,
        omitted,
    })
}
fn sources(sections: &[&ContextSection]) -> Vec<String> {
    sections
        .iter()
        .flat_map(|s| s.sources.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn selection_tokens(header: &str, sections: &[&ContextSection], included: &[bool]) -> usize {
    [ContextFormat::Agent, ContextFormat::Human]
        .into_iter()
        .map(|format| estimate_tokens(&render(header, sections, included, format)))
        .max()
        .unwrap_or(0)
}
fn render(
    header: &str,
    sections: &[&ContextSection],
    included: &[bool],
    format: ContextFormat,
) -> String {
    let mut output =
        format!("{header}\nToken estimate: ASCII bytes/4 (rounded up) + non-ASCII scalars.\n");
    for (section, yes) in sections.iter().zip(included) {
        if *yes {
            output.push_str(&format!("\n## {}\n{}\n", section.title, section.content));
        }
    }
    let omitted = sections
        .iter()
        .zip(included)
        .filter(|(_, yes)| !**yes)
        .map(|(s, _)| s.id.as_str())
        .collect::<Vec<_>>();
    if !omitted.is_empty() {
        let label = match format {
            ContextFormat::Human => "Details omitted for budget",
            ContextFormat::Agent => "Omitted (budget)",
        };
        output.push_str(&format!("\n{label}: {}.\n", omitted.join(", ")));
    }
    let refs = sources(sections);
    if !refs.is_empty() {
        output.push_str("\n## Sources\nRetrieve original: belay show <ID>\n");
        for id in refs {
            output.push_str(&format!("- {id}\n"));
        }
    }
    output
}
#[cfg(test)]
mod tests {
    use super::*;
    fn section(id: &str, requirement: SectionRequirement, content: &str) -> ContextSection {
        ContextSection {
            id: id.into(),
            title: id.into(),
            requirement,
            order: 0,
            sources: vec![format!("source-{id}")],
            content: content.into(),
        }
    }
    #[test]
    fn required_packet_is_whole_or_failure_with_shortfall() {
        let sections = [section(
            "boundary",
            SectionRequirement::Required,
            &format!("{} FINAL STOP 日本語", "constraint ".repeat(100)),
        )];
        let large = render_sections("packet", &sections, ContextFormat::Agent, 2000).unwrap();
        assert!(large.text.contains("FINAL STOP 日本語"));
        let small = render_sections(
            "packet",
            &sections,
            ContextFormat::Agent,
            large.estimated_tokens - 1,
        )
        .unwrap_err();
        assert!(small.shortfall >= 1);
        assert_eq!(small.required_section_ids, vec!["boundary"]);
        assert_eq!(small.required_tokens - small.budget, small.shortfall);
    }
    #[test]
    fn optional_omissions_and_sources_are_identical_between_formats() {
        let sections = [
            section("boundary", SectionRequirement::Required, "stop"),
            section(
                "history",
                SectionRequirement::Optional,
                &"old prose ".repeat(200),
            ),
        ];
        let agent = render_sections("packet", &sections, ContextFormat::Agent, 200).unwrap();
        let human = render_sections("packet", &sections, ContextFormat::Human, 200).unwrap();
        assert_eq!(agent.included_section_ids, human.included_section_ids);
        assert_eq!(agent.source_ids, human.source_ids);
        assert_eq!(agent.omitted, human.omitted);
        assert_eq!(agent.omitted[0].reason, "budget");
        assert!(agent.text.contains("source-history"));
        assert!(agent.estimated_tokens <= 200 && human.estimated_tokens <= 200);
    }
    #[test]
    fn equal_order_is_stable_and_duplicate_ids_fail() {
        let a = section("a", SectionRequirement::Optional, "first");
        let b = section("b", SectionRequirement::Optional, "second");
        let one = render_sections(
            "packet",
            &[b.clone(), a.clone()],
            ContextFormat::Agent,
            1000,
        )
        .unwrap();
        let two = render_sections("packet", &[a.clone(), b], ContextFormat::Agent, 1000).unwrap();
        assert_eq!(one.text, two.text);
        assert!(render_sections("packet", &[a.clone(), a], ContextFormat::Agent, 1000).is_err());
    }
}
