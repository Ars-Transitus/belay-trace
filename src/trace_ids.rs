use std::collections::BTreeSet;

use crate::entry::EntryType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceSpan {
    pub start: usize,
    pub end: usize,
    pub value: String,
    pub evidence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalIdFinding {
    pub field: &'static str,
    pub message: String,
    pub line: usize,
}

pub fn fragment_exists(entry_type: EntryType, body: &str, fragment: &str) -> bool {
    fragment_match_count(entry_type, body, fragment) == 1
}

pub fn local_fragments(entry_type: EntryType, body: &str) -> BTreeSet<String> {
    match entry_type {
        EntryType::Goal => goal_fragments(body),
        EntryType::Plan => plan_fragments(body),
        _ => BTreeSet::new(),
    }
}

pub fn fragment_match_count(entry_type: EntryType, body: &str, fragment: &str) -> usize {
    match entry_type {
        EntryType::Goal => goal_items(body)
            .into_iter()
            .filter(|(_, item)| {
                explicit_goal_id(item).is_some_and(|id| id.eq_ignore_ascii_case(fragment))
            })
            .count(),
        EntryType::Plan => delivery_map_ids(body)
            .into_iter()
            .filter(|id| id.eq_ignore_ascii_case(fragment))
            .count(),
        _ => 0,
    }
}

/// The line that defines a fragment: the Delivery Map row for a Plan task, or
/// the Success Criteria item for a Goal criterion. The definition site is the
/// same one `fragment_match_count` counts, so a fragment that does not resolve
/// uniquely has no definition line.
pub fn fragment_definition(entry_type: EntryType, body: &str, fragment: &str) -> Option<String> {
    if fragment_match_count(entry_type, body, fragment) != 1 {
        return None;
    }
    match entry_type {
        // A Delivery Map row is one table line, so it is complete on its own.
        EntryType::Plan => {
            let line = delivery_map_id_rows(body)
                .into_iter()
                .find(|(_, id)| id.eq_ignore_ascii_case(fragment))
                .map(|(line, _)| line)?;
            body.lines().nth(line - 1).map(str::to_owned)
        }
        // A Success Criterion is a list item that usually wraps, and half a
        // criterion reads as a whole one, so take its continuation lines too.
        EntryType::Goal => {
            let line = goal_items(body)
                .into_iter()
                .find(|(_, item)| {
                    explicit_goal_id(item).is_some_and(|id| id.eq_ignore_ascii_case(fragment))
                })
                .map(|(line, _)| line)?;
            let mut lines = body.lines().skip(line - 1);
            let mut item = vec![lines.next()?.to_owned()];
            for candidate in lines {
                if candidate.trim().is_empty() || !candidate.starts_with(char::is_whitespace) {
                    break;
                }
                item.push(candidate.to_owned());
            }
            Some(item.join("\n"))
        }
        _ => None,
    }
}

pub fn valid_reference_fragment(entry_type: EntryType, fragment: &str) -> bool {
    let upper = fragment.to_ascii_uppercase();
    match entry_type {
        EntryType::Goal => valid_goal_id(&upper),
        EntryType::Plan => valid_task_id(&upper),
        _ => false,
    }
}

pub fn goal_id_findings(body: &str) -> Vec<LocalIdFinding> {
    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();
    for (line, item) in goal_items(body) {
        let Some(id) = explicit_goal_id(item) else {
            findings.push(LocalIdFinding {
                field: "Success Criteria",
                message: "criterion must start with a stable ID such as `[SC-001]`".to_owned(),
                line,
            });
            continue;
        };
        let normalized = id.to_ascii_lowercase();
        if !seen.insert(normalized) {
            findings.push(LocalIdFinding {
                field: "Success Criteria",
                message: format!("duplicate criterion ID {id}"),
                line,
            });
        }
    }
    findings
}

pub fn plan_id_findings(body: &str) -> Vec<LocalIdFinding> {
    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();
    for (line, id) in delivery_map_id_rows(body) {
        if !valid_task_id(&id) {
            findings.push(LocalIdFinding {
                field: "Delivery Map",
                message: format!("task ID {id:?} must use fixed-width form such as `T-001`"),
                line,
            });
            continue;
        }
        if !seen.insert(id.to_ascii_lowercase()) {
            findings.push(LocalIdFinding {
                field: "Delivery Map",
                message: format!("duplicate task ID {id}"),
                line,
            });
        }
    }
    findings
}

pub fn valid_goal_id(value: &str) -> bool {
    valid_numbered_id(value, "SC-")
}

pub fn valid_task_id(value: &str) -> bool {
    valid_numbered_id(value, "T-")
}

pub fn reference_spans(text: &str) -> Vec<ReferenceSpan> {
    let mut spans = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        let Some((start, _)) = text[cursor..]
            .char_indices()
            .find(|(_, character)| character.is_ascii_uppercase())
        else {
            break;
        };
        let start = cursor + start;
        let previous_is_id = text[..start].chars().next_back().is_some_and(id_character);
        if previous_is_id {
            cursor = next_character(text, start);
            continue;
        }
        let end = text[start..]
            .char_indices()
            .find_map(|(offset, character)| {
                (!id_character(character) && character != '#').then_some(start + offset)
            })
            .unwrap_or(text.len());
        let candidate = &text[start..end];
        let entry = crate::entry::parse_entry_reference_id(candidate)
            .ok()
            .filter(|reference| {
                reference.fragment.as_deref().is_none_or(|fragment| {
                    let upper = fragment.to_ascii_uppercase();
                    valid_goal_id(&upper) || valid_task_id(&upper)
                })
            });
        let evidence = valid_evidence_id(candidate);
        if entry.is_some() || evidence {
            spans.push(ReferenceSpan {
                start,
                end,
                value: candidate.to_owned(),
                evidence,
            });
            cursor = end;
        } else {
            cursor = next_character(text, start);
        }
    }
    spans
}

fn valid_numbered_id(value: &str, prefix: &str) -> bool {
    let Some(number) = value.strip_prefix(prefix) else {
        return false;
    };
    number.len() == 3 && number.bytes().all(|byte| byte.is_ascii_digit()) && number != "000"
}

fn valid_evidence_id(value: &str) -> bool {
    crate::evidence::validate_evidence_id(value).is_ok()
}

fn id_character(character: char) -> bool {
    character.is_alphanumeric() || character == '-'
}

fn next_character(text: &str, offset: usize) -> usize {
    offset + text[offset..].chars().next().map_or(1, char::len_utf8)
}

fn goal_fragments(body: &str) -> BTreeSet<String> {
    goal_items(body)
        .into_iter()
        .filter_map(|(_, item)| explicit_goal_id(item).map(str::to_ascii_lowercase))
        .collect()
}

fn plan_fragments(body: &str) -> BTreeSet<String> {
    delivery_map_ids(body)
        .into_iter()
        .map(|id| id.to_ascii_lowercase())
        .collect()
}

fn goal_items(body: &str) -> Vec<(usize, &str)> {
    let mut in_section = false;
    let mut items = Vec::new();
    for (index, line) in body.lines().enumerate() {
        if let Some(heading) = line.strip_prefix("## ") {
            in_section = heading.trim().eq_ignore_ascii_case("Success Criteria");
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(item) = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .or_else(|| line.strip_prefix("+ "))
        {
            items.push((index + 1, item));
        }
    }
    items
}

pub fn explicit_goal_id(item: &str) -> Option<&str> {
    let remainder = item.strip_prefix('[')?;
    let (id, _) = remainder.split_once(']')?;
    valid_goal_id(id).then_some(id)
}

/// One Delivery Map task row. `cells` is keyed by the lowercased column
/// heading, so a consumer can read a column belay does not mandate without a
/// second table parser existing anywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryMapRow {
    pub line: usize,
    pub id: String,
    pub cells: std::collections::BTreeMap<String, String>,
}

impl DeliveryMapRow {
    pub fn cell(&self, column: &str) -> Option<&str> {
        self.cells
            .get(&column.to_ascii_lowercase())
            .map(String::as_str)
    }
}

/// Whether the body has a Delivery Map heading at all. A Plan without one is
/// not malformed; it simply has nothing to check.
pub fn has_delivery_map(body: &str) -> bool {
    body.lines().any(|line| {
        line.strip_prefix("## ")
            .is_some_and(|heading| heading.trim().eq_ignore_ascii_case("Delivery Map"))
    })
}

fn delivery_map_ids(body: &str) -> Vec<String> {
    delivery_map_id_rows(body)
        .into_iter()
        .map(|(_, id)| id)
        .filter(|id| valid_task_id(id))
        .collect()
}

fn delivery_map_id_rows(body: &str) -> Vec<(usize, String)> {
    delivery_map_rows(body)
        .into_iter()
        .map(|row| (row.line, row.id))
        .collect()
}

pub fn delivery_map_rows(body: &str) -> Vec<DeliveryMapRow> {
    let mut in_section = false;
    let mut headers = None::<Vec<String>>;
    let mut id_index = None;
    let mut rows = Vec::new();
    for (line_number, line) in body.lines().enumerate() {
        if let Some(heading) = line.strip_prefix("## ") {
            in_section = heading.trim().eq_ignore_ascii_case("Delivery Map");
            headers = None;
            id_index = None;
            continue;
        }
        if !in_section || !line.trim_start().starts_with('|') {
            continue;
        }
        let cells = table_cells(line);
        if id_index.is_none() {
            id_index = cells
                .iter()
                .position(|cell| cell.eq_ignore_ascii_case("ID"));
            if id_index.is_some() {
                headers = Some(cells.iter().map(|cell| cell.to_ascii_lowercase()).collect());
            }
            continue;
        }
        let Some(index) = id_index else {
            continue;
        };
        let Some(id) = cells.get(index).map(String::as_str) else {
            continue;
        };
        // The separator row's cells are only dashes and colons, so it is not a
        // task even though it sits under the header.
        if !id
            .chars()
            .all(|character| character == '-' || character == ':')
        {
            let named = headers.as_ref().map_or_else(Default::default, |headers| {
                headers
                    .iter()
                    .cloned()
                    .zip(cells.iter().cloned())
                    .collect::<std::collections::BTreeMap<_, _>>()
            });
            rows.push(DeliveryMapRow {
                line: line_number + 1,
                id: id.to_owned(),
                cells: named,
            });
        }
    }
    rows
}

fn table_cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_fragments_include_only_canonical_anchors() {
        let body = "## Success Criteria\n\n- [SC-001] Stable result\n";
        let fragments = local_fragments(EntryType::Goal, body);
        assert_eq!(fragments, BTreeSet::from(["sc-001".to_owned()]));
        assert!(!fragment_exists(EntryType::Goal, body, "sc-1"));
    }

    #[test]
    fn goal_findings_require_unique_fixed_width_ids() {
        let body = "## Success Criteria\n\n- Missing\n- [SC-001] First\n- [SC-001] Duplicate\n";
        let findings = goal_id_findings(body);
        assert_eq!(findings.len(), 2);
        assert!(findings[0].message.contains("must start"));
        assert!(findings[1].message.contains("duplicate"));
    }

    #[test]
    fn nested_goal_details_do_not_define_criteria() {
        let body = "## Success Criteria\n\n- [SC-001] Stable result\n  - explanatory detail\n";
        assert!(goal_id_findings(body).is_empty());
        assert_eq!(fragment_match_count(EntryType::Goal, body, "sc-001"), 1);
    }

    #[test]
    fn plan_fragments_include_only_canonical_task_anchors() {
        let body = "## Delivery Map\n\n| ID | State |\n| --- | --- |\n| T-001 | verified |\n";
        let fragments = local_fragments(EntryType::Plan, body);
        assert_eq!(fragments, BTreeSet::from(["t-001".to_owned()]));
        assert!(!fragment_exists(EntryType::Plan, body, "task-t-001"));
    }

    #[test]
    fn plan_fragment_definition_is_the_delivery_map_row() {
        let body = "## Delivery Map\n\n| ID | State |\n| --- | --- |\n| T-001 | verified |\n| T-002 | blocked |\n";
        assert_eq!(
            fragment_definition(EntryType::Plan, body, "t-002").as_deref(),
            Some("| T-002 | blocked |")
        );
        assert_eq!(fragment_definition(EntryType::Plan, body, "t-003"), None);
    }

    #[test]
    fn goal_fragment_definition_keeps_the_whole_wrapped_item() {
        let body = concat!(
            "## Success Criteria\n\n",
            "- [SC-001] First line of the criterion\n",
            "  wraps onto a second line\n",
            "  and a third.\n",
            "- [SC-002] Next criterion\n",
        );
        assert_eq!(
            fragment_definition(EntryType::Goal, body, "sc-001").as_deref(),
            Some(
                "- [SC-001] First line of the criterion\n  wraps onto a second line\n  and a third."
            )
        );
    }

    #[test]
    fn delivery_map_rows_expose_columns_belay_does_not_mandate() {
        let body = concat!(
            "## Delivery Map\n\n",
            "| ID | Goal item | State | Difficulty |\n",
            "| --- | --- | --- | --- |\n",
            "| T-001 | SC-001 | verified | high |\n",
        );
        let rows = delivery_map_rows(body);
        assert_eq!(rows.len(), 1, "the separator row is not a task");
        assert_eq!(rows[0].id, "T-001");
        assert_eq!(rows[0].cell("state"), Some("verified"));
        assert_eq!(rows[0].cell("Goal item"), Some("SC-001"));
        assert_eq!(rows[0].cell("difficulty"), Some("high"));
        assert_eq!(rows[0].cell("actor"), None);
    }

    #[test]
    fn delivery_map_detection_is_independent_of_task_rows() {
        assert!(has_delivery_map("## Delivery Map\n\nnot a table yet\n"));
        assert!(!has_delivery_map("## Intent Brief\n\n- prose\n"));
    }

    #[test]
    fn ambiguous_fragment_has_no_definition() {
        let body = "## Delivery Map\n\n| ID | State |\n| --- | --- |\n| T-001 | verified |\n| T-001 | blocked |\n";
        assert_eq!(fragment_match_count(EntryType::Plan, body, "t-001"), 2);
        assert_eq!(fragment_definition(EntryType::Plan, body, "t-001"), None);
    }

    #[test]
    fn entry_types_without_a_fragment_standard_have_no_definition() {
        let body = "## Delivery Map\n\n| ID | State |\n| --- | --- |\n| T-001 | verified |\n";
        assert_eq!(fragment_definition(EntryType::Work, body, "t-001"), None);
    }

    #[test]
    fn reference_fragments_require_fixed_width_canonical_ids() {
        assert!(valid_reference_fragment(EntryType::Goal, "sc-001"));
        assert!(valid_reference_fragment(EntryType::Goal, "SC-001"));
        assert!(valid_reference_fragment(EntryType::Plan, "t-001"));
        assert!(!valid_reference_fragment(EntryType::Goal, "sc-1"));
        assert!(!valid_reference_fragment(EntryType::Plan, "task-t-1"));
        assert!(!valid_reference_fragment(EntryType::Work, "sc-001"));
    }

    #[test]
    fn duplicate_local_ids_are_ambiguous_and_do_not_resolve() {
        let goal = "## Success Criteria\n\n- [SC-001] First\n- [SC-001] Duplicate\n";
        let plan = "## Delivery Map\n\n| ID | State |\n| --- | --- |\n| T-001 | verified |\n| T-001 | blocked |\n";
        assert_eq!(fragment_match_count(EntryType::Goal, goal, "sc-001"), 2);
        assert!(!fragment_exists(EntryType::Goal, goal, "sc-001"));
        assert_eq!(fragment_match_count(EntryType::Plan, plan, "t-001"), 2);
        assert!(!fragment_exists(EntryType::Plan, plan, "t-001"));
    }

    #[test]
    fn plan_findings_require_unique_fixed_width_ids() {
        let body = "## Delivery Map\n\n| ID | State |\n| --- | --- |\n| T-1 | verified |\n| T-001 | verified |\n| T-001 | blocked |\n";
        let findings = plan_id_findings(body);
        assert_eq!(findings.len(), 2);
        assert!(findings[0].message.contains("fixed-width"));
        assert!(findings[1].message.contains("duplicate"));
    }

    #[test]
    fn reference_spans_find_entries_fragments_and_evidence() {
        let text = "See GOAL-20260723T120000-001-safe-sync#sc-001 and \
                    EVD-20260723T120500-001; ignore SC-001.";
        let spans = reference_spans(text);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].value, "GOAL-20260723T120000-001-safe-sync#sc-001");
        assert!(!spans[0].evidence);
        assert!(spans[1].evidence);
    }

    #[test]
    fn distributed_evidence_references_use_the_same_validator_as_storage() {
        let id = "EVD-0123456789abcdef0123456789abcdef";
        let spans = reference_spans(&format!(
            "Proof: {id}; malformed EVD-0123456 and {id}#sc-001"
        ));
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].value, id);
        assert!(spans[0].evidence);
        assert!(!valid_evidence_id("EVD-0123456789ABCDEF0123456789ABCDEF"));
    }
}
