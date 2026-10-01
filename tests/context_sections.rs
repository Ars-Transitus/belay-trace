use belay_trace::context::{self, ContextFormat};
use belay_trace::entry::{EntryStatus, EntryType, LinkRelation, MetadataValue};
use belay_trace::{evidence, markdown, repository, store};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn command(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_belay"))
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}
fn repo() -> (tempfile::TempDir, repository::Repository) {
    let dir = tempfile::tempdir().unwrap();
    repository::initialize(dir.path()).unwrap();
    let r = repository::discover(dir.path()).unwrap();
    (dir, r)
}
fn create(r: &repository::Repository, kind: EntryType, title: &str, body: &str) -> String {
    store::create(r, kind, title.into(), body.into())
        .unwrap()
        .display_id
        .clone()
}
fn plan_body(boundary: &str, state: &str) -> String {
    format!(
        "## Intent Brief\n### Constraints\n- PLAN FINAL constraint {boundary}\n### Non-goals\n- no sibling changes\n### Assumptions\n- assumption one\n### Unknowns / Decisions Needed\n- decision unresolved\n### Stop Conditions\n- STOP before publication\n## Delivery Map\n| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |\n| --- | --- | --- | --- | --- | --- |\n| T-001 | SC-001 | resume task | AI | {state} | pending |\n| T-002 | SC-002 | HIDDEN sibling | AI | not-started | pending |\n## T-001\n- Objective: objective one\n- Scope: only current task\n- Steps: steps one\n- Acceptance: acceptance one\n- Verification: tests one\n- Stop: FINAL TASK STOP\n## T-002\n- Objective: HIDDEN sibling details\n"
    )
}
fn goal(r: &repository::Repository, title: &str) -> String {
    create(
        r,
        EntryType::Goal,
        title,
        "## Summary\nintent\n## Success Criteria\n- [SC-001] verify criterion one\n- [SC-002] sibling criterion\n## Constraints\n- GOAL FINAL constraint\n## Non-goals\n- GOAL exclusion\n## Stop conditions\n- GOAL FINAL STOP\n## Verification\ntest\n## Risks\nunknown\n",
    )
}
fn record(
    r: &repository::Repository,
    target: &str,
    kind: &str,
    verdict: &str,
    captured: &str,
) -> String {
    evidence::record(
        r,
        evidence::RecordInput {
            kind: kind.into(),
            verdict: verdict.into(),
            commit_sha: Some("unknown".into()),
            captured_at: Some(captured.into()),
            source: "fixture".into(),
            issuer: "fixture".into(),
            summary: "fixture record".into(),
            detail: serde_json::json!({}),
            verifies: vec![target.into()],
        },
    )
    .unwrap()
    .display_id
    .clone()
}
fn scoped(r: &repository::Repository, id: &str, scope: &str) {
    let mut shown = store::show(r, id).unwrap().entry;
    shown
        .metadata
        .insert("scope".into(), MetadataValue::String(scope.into()));
    let path = r
        .entries_path()
        .join(shown.entry_type.directory())
        .join(format!("{id}.md"));
    fs::write(path, markdown::render(&shown).unwrap()).unwrap();
    let output = command(&r.root, &["sync"]);
    assert!(output.status.success(), "{output:?}");
}
#[test]
fn focus_preserves_plan_goal_task_boundary_and_all_evidence_or_fails() {
    let (_dir, r) = repo();
    let g = goal(&r, "goal-one");
    let p = create(
        &r,
        EntryType::Plan,
        "plan-one",
        &plan_body("short", "implemented"),
    );
    store::link(&r, &p, &g, LinkRelation::Fulfills).unwrap();
    let target = format!("{p}#t-001");
    let evd = record(&r, &target, "test", "fail", "2020-01-01T00:00:00Z");
    let agent = context::compile_focus(&r, &target, ContextFormat::Agent, 2500).unwrap();
    let human = context::compile_focus(&r, &target, ContextFormat::Human, 2500).unwrap();
    for required in [
        "PLAN FINAL constraint",
        "GOAL FINAL constraint",
        "GOAL exclusion",
        "GOAL FINAL STOP",
        "assumption one",
        "decision unresolved",
        "STOP before publication",
        "FINAL TASK STOP",
        "Objective:",
        "Scope:",
        "Acceptance:",
        "Verification:",
        &evd,
        "stale",
    ] {
        assert!(agent.text.contains(required), "{required}: {}", agent.text);
        assert!(human.text.contains(required));
    }
    assert!(!agent.text.contains("HIDDEN"));
    assert!(!agent.text.contains("verified]"));
    let ids = |text: &str| {
        text.lines()
            .filter(|l| l.starts_with("- ") && l.contains("202"))
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(ids(&agent.text), ids(&human.text));
    let failure = context::compile_focus(&r, &target, ContextFormat::Agent, 80)
        .unwrap_err()
        .to_string();
    assert!(failure.contains("shortfall="));
    assert!(failure.contains("goal-boundaries"));
}
#[test]
fn long_last_boundaries_fail_without_silent_partial_packet() {
    let (_dir, r) = repo();
    let g = goal(&r, "long-goal");
    let p = create(
        &r,
        EntryType::Plan,
        "long-plan",
        &plan_body(&"protected text ".repeat(200), "not-started"),
    );
    store::link(&r, &p, &g, LinkRelation::Fulfills).unwrap();
    let target = format!("{p}#t-001");
    assert!(context::compile_focus(&r, &target, ContextFormat::Agent, 300).is_err());
    let output = context::compile_focus(&r, &target, ContextFormat::Agent, 4000).unwrap();
    assert!(output.text.contains("FINAL TASK STOP"));
    assert!(output.text.contains("GOAL FINAL STOP"));
}
#[test]
fn ambiguous_mapping_fails_and_unmapped_legacy_packet_is_unknown() {
    let (_dir, r) = repo();
    let p = create(
        &r,
        EntryType::Plan,
        "map-plan",
        &plan_body("short", "not-started"),
    );
    let target = format!("{p}#t-001");
    let legacy = context::compile_focus(&r, &target, ContextFormat::Agent, 2500).unwrap();
    assert!(legacy.text.contains("Unknown (no linked Goal"));
    let a = goal(&r, "goal-a");
    let b = goal(&r, "goal-b");
    store::link(&r, &p, &a, LinkRelation::Fulfills).unwrap();
    store::link(&r, &p, &b, LinkRelation::Implements).unwrap();
    assert!(
        context::compile_focus(&r, &target, ContextFormat::Agent, 2500)
            .unwrap_err()
            .to_string()
            .contains("ambiguous Task mapping")
    );
}
#[test]
fn live_empty_archive_blocked_and_completion_do_not_claim_verification_or_authority() {
    let (_dir, r) = repo();
    let empty = context::compile_working_set(&r, ContextFormat::Agent, 2500, false).unwrap();
    assert!(empty.text.contains("No active Goal/Plan"));
    let g = goal(&r, "active-goal");
    store::set_status(&r, &g, EntryStatus::Active).unwrap();
    let p = create(
        &r,
        EntryType::Plan,
        "live-plan",
        &plan_body("short", "blocked"),
    );
    store::set_status(&r, &p, EntryStatus::Active).unwrap();
    let text = context::compile_working_set(&r, ContextFormat::Agent, 2500, false)
        .unwrap()
        .text;
    assert!(text.contains("#t-001: blocked"));
    assert!(text.contains("decision unresolved"));
    assert!(text.contains("missing (unverified)"));
    assert!(text.contains("execution authorization is Unknown"));
    assert!(!text.contains("verified]"));
    store::set_status(&r, &g, EntryStatus::Archived).unwrap();
    store::set_status(&r, &p, EntryStatus::Archived).unwrap();
    let archived = context::compile_working_set(&r, ContextFormat::Agent, 2500, false)
        .unwrap()
        .text;
    assert!(!archived.contains(&g));
    assert!(!archived.contains(&p));
}
#[test]
fn decisions_need_scope_adoption_and_no_superseding_or_conflicting_acceptance() {
    let (_dir, r) = repo();
    let g = goal(&r, "scoped-goal");
    store::set_status(&r, &g, EntryStatus::Active).unwrap();
    let a = create(
        &r,
        EntryType::Decision,
        "first-decision",
        "## Rationale\nfixture rationale",
    );
    store::set_status(&r, &a, EntryStatus::Accepted).unwrap();
    store::link(&r, &a, &g, LinkRelation::Fulfills).unwrap();
    let text = context::compile_working_set(&r, ContextFormat::Agent, 4000, false)
        .unwrap()
        .text;
    assert!(text.contains("unconfirmed; scope missing"));
    scoped(&r, &a, &g);
    let text = context::compile_working_set(&r, ContextFormat::Agent, 4000, false)
        .unwrap()
        .text;
    assert!(text.contains("human-approval Evidence missing"));
    let approval = record(&r, &a, "human-approval", "pass", "2020-01-01T00:00:00Z");
    let text = context::compile_working_set(&r, ContextFormat::Agent, 4000, false)
        .unwrap()
        .text;
    assert!(text.contains("explicit-scope; adoption recorded"));
    assert!(text.contains(&approval));
    assert!(text.contains("stale"));
    let b = create(
        &r,
        EntryType::Decision,
        "second-decision",
        "fixture rationale",
    );
    store::set_status(&r, &b, EntryStatus::Accepted).unwrap();
    scoped(&r, &b, &g);
    let unconfirmed = context::compile_working_set(&r, ContextFormat::Agent, 4000, false)
        .unwrap()
        .text;
    assert!(!unconfirmed.contains("conflict-candidate"));
    record(&r, &b, "human-approval", "pass", "2020-01-02T00:00:00Z");
    let text = context::compile_working_set(&r, ContextFormat::Agent, 4000, false)
        .unwrap()
        .text;
    assert!(text.contains("conflict-candidate"));
    store::link(&r, &b, &a, LinkRelation::Supersedes).unwrap();
    let text = context::compile_working_set(&r, ContextFormat::Agent, 4000, false)
        .unwrap()
        .text;
    assert!(text.contains(&format!("{a}: replaced")));
    assert!(text.contains(&b));
}
#[test]
fn query_selection_ignores_unrelated_new_failure_and_is_deterministic() {
    let (_dir, r) = repo();
    let d = create(
        &r,
        EntryType::Decision,
        "quartz-choice",
        "## Decision\nquartz persistence rationale",
    );
    let unrelated = create(
        &r,
        EntryType::Work,
        "unrelated-failure",
        "## Findings\nunrelated history",
    );
    store::set_status(&r, &unrelated, EntryStatus::Abandoned).unwrap();
    let a = context::compile(&r, "quartz", ContextFormat::Agent, 1500, &[], false).unwrap();
    let b = context::compile(&r, "quartz", ContextFormat::Agent, 1500, &[], false).unwrap();
    assert_eq!(a.text, b.text);
    assert!(a.text.contains(&d));
    assert!(!a.text.contains(&unrelated));
    assert!(a.text.contains("Why:"));
    assert!(a.text.contains("Unrelated scope excluded"));
}
#[test]
fn malformed_original_evidence_is_not_silently_treated_as_missing() {
    let (_dir, r) = repo();
    fs::create_dir_all(r.evidence_path()).unwrap();
    fs::write(r.evidence_path().join("invalid.ndjson"), "{malformed}\n").unwrap();
    assert!(context::compile_working_set(&r, ContextFormat::Agent, 2500, false).is_err());
}

#[test]
fn stale_or_missing_decision_original_never_keeps_an_applicability_claim() {
    let (_dir, r) = repo();
    let g = goal(&r, "bound-goal");
    store::set_status(&r, &g, EntryStatus::Active).unwrap();
    let decision = create(
        &r,
        EntryType::Decision,
        "bound-decision",
        "decision rationale",
    );
    store::set_status(&r, &decision, EntryStatus::Accepted).unwrap();
    scoped(&r, &decision, &g);
    record(
        &r,
        &decision,
        "human-approval",
        "pass",
        "2020-01-01T00:00:00Z",
    );
    assert!(
        context::compile_working_set(&r, ContextFormat::Agent, 4000, false)
            .unwrap()
            .text
            .contains("explicit-scope")
    );
    let path = r
        .entries_path()
        .join("decisions")
        .join(format!("{decision}.md"));
    let original = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        original.replace("decision rationale", "changed original rationale"),
    )
    .unwrap();
    assert!(
        context::compile_working_set(&r, ContextFormat::Agent, 4000, false)
            .unwrap_err()
            .to_string()
            .contains("stale indexed original")
    );
    fs::write(&path, &original).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(context::compile_working_set(&r, ContextFormat::Agent, 4000, false).is_err());
}
#[test]
fn focus_refuses_unsynced_plan_and_goal_boundaries() {
    let (_dir, r) = repo();
    let g = goal(&r, "bound-goal");
    let p = create(
        &r,
        EntryType::Plan,
        "bound-plan",
        &plan_body("short", "not-started"),
    );
    store::link(&r, &p, &g, LinkRelation::Fulfills).unwrap();
    let reference = format!("{p}#t-001");
    for (kind, id) in [("plans", &p), ("goals", &g)] {
        let path = r.entries_path().join(kind).join(format!("{id}.md"));
        let original = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            format!("{original}\n## Stop conditions\n- NEW UNSYNCED STOP\n"),
        )
        .unwrap();
        assert!(
            context::compile_focus(&r, &reference, ContextFormat::Agent, 4000)
                .unwrap_err()
                .to_string()
                .contains("stale indexed original")
        );
        fs::write(&path, original).unwrap();
    }
}

#[test]
fn focus_retains_nested_task_markdown_and_rejects_required_overflow() {
    for heading in ["## T-001", "  ## **T-001** ##", "T-001\n-----"] {
        let (_dir, r) = repo();
        let g = goal(&r, "nested-goal");
        let prefix = plan_body("short", "not-started")
            .split("## T-001")
            .next()
            .unwrap()
            .to_owned();
        let nested = format!(
            "{prefix}\n{heading}\n- Objective: current task\n### Scope\nNESTED SCOPE\n### Constraints\nNESTED CONSTRAINT\n### Acceptance\nNESTED ACCEPTANCE\n### Stop conditions\nNESTED FINAL STOP\n```markdown\n## T-001\nFENCED TASK EXAMPLE\n## T-002\n```\n> ## T-001\n> QUOTED TASK EXAMPLE\n\nT-002\n-----\nSIBLING EXCLUDED\n## Constraints\nUNRELATED PARENT EXCLUDED\n"
        );
        let p = create(&r, EntryType::Plan, "nested-plan", &nested);
        store::link(&r, &p, &g, LinkRelation::Fulfills).unwrap();
        let target = format!("{p}#t-001");
        for format in [ContextFormat::Agent, ContextFormat::Human] {
            let packet = context::compile_focus(&r, &target, format, 5000).unwrap();
            for required in [
                "NESTED SCOPE",
                "NESTED CONSTRAINT",
                "NESTED ACCEPTANCE",
                "NESTED FINAL STOP",
                "FENCED TASK EXAMPLE",
                "QUOTED TASK EXAMPLE",
                "PLAN FINAL constraint",
                "GOAL FINAL constraint",
            ] {
                assert!(
                    packet.text.contains(required),
                    "{required}: {}",
                    packet.text
                );
            }
            assert!(!packet.text.contains("SIBLING EXCLUDED"));
            assert!(!packet.text.contains("UNRELATED PARENT EXCLUDED"));
        }
    }
    let (_dir, r) = repo();
    let body = plan_body("short", "not-started").replace(
        "- Stop: FINAL TASK STOP",
        &format!(
            "### Stop conditions\n{} FINAL NESTED STOP",
            "protected nested boundary ".repeat(1000)
        ),
    );
    let p = create(&r, EntryType::Plan, "large-nested-plan", &body);
    let failure = context::compile_focus(&r, &format!("{p}#t-001"), ContextFormat::Agent, 1500)
        .unwrap_err()
        .to_string();
    assert!(failure.contains("shortfall="), "{failure}");
    assert!(failure.contains("task"), "{failure}");
}

#[test]
fn focus_duplicate_task_heading_or_mapping_fails_closed() {
    for body in [
        format!(
            "{}\n## **T-001** ##\nDUPLICATE TASK\n",
            plan_body("short", "not-started")
        ),
        plan_body("short", "not-started").replace("| T-002 | SC-002", "| T-001 | SC-002"),
    ] {
        let (_dir, r) = repo();
        let p = create(&r, EntryType::Plan, "duplicate-task", &body);
        let failure = context::compile_focus(&r, &format!("{p}#t-001"), ContextFormat::Agent, 5000)
            .unwrap_err()
            .to_string();
        assert!(failure.contains("ambiguous"), "{failure}");
    }
}

#[test]
fn canonical_decision_scope_excludes_linked_outside_decisions_and_discloses_seed_override() {
    let (_dir, r) = repo();
    let a = goal(&r, "query-goal");
    let b = goal(&r, "outside-goal");
    store::set_status(&r, &a, EntryStatus::Active).unwrap();
    for scope in [b.clone(), format!("{b}#sc-001")] {
        let outside = create(&r, EntryType::Decision, "outside-decision", "OUTSIDE PROSE");
        store::set_status(&r, &outside, EntryStatus::Accepted).unwrap();
        scoped(&r, &outside, &scope);
        store::link(&r, &outside, &a, LinkRelation::References).unwrap();
        record(
            &r,
            &outside,
            "human-approval",
            "pass",
            "2020-01-01T00:00:00Z",
        );
        let packet = context::compile(&r, &a, ContextFormat::Agent, 5000, &[], false).unwrap();
        assert!(!packet.text.contains("OUTSIDE PROSE"), "{}", packet.text);
        assert!(!packet.text.contains("Applicability: explicit-scope"));
        assert!(packet.text.contains("outside selected scope"));
        let explicit = context::compile(
            &r,
            &a,
            ContextFormat::Agent,
            5000,
            std::slice::from_ref(&outside),
            false,
        )
        .unwrap();
        assert!(explicit.text.contains("OUTSIDE PROSE"), "{}", explicit.text);
        assert!(explicit.text.contains(&format!("scope={scope}")));
        assert!(
            explicit
                .text
                .contains("included only by explicit seed; current applicability Unknown")
        );
        let live = context::compile_working_set(&r, ContextFormat::Agent, 5000, false).unwrap();
        assert!(!live.text.contains(&outside), "{}", live.text);
    }
    let inside = create(&r, EntryType::Decision, "inside-decision", "INSIDE PROSE");
    store::set_status(&r, &inside, EntryStatus::Accepted).unwrap();
    scoped(&r, &inside, &format!("{a}#sc-001"));
    store::link(&r, &inside, &a, LinkRelation::References).unwrap();
    record(
        &r,
        &inside,
        "human-approval",
        "pass",
        "2020-01-01T00:00:00Z",
    );
    let packet = context::compile(&r, &a, ContextFormat::Agent, 5000, &[], false).unwrap();
    assert!(packet.text.contains("INSIDE PROSE"));
    assert!(packet.text.contains("within selected scope"));
    assert!(packet.text.contains(&format!("scope={a}#sc-001")));
}

#[test]
fn human_decision_scope_is_preserved_with_unknown_current_applicability() {
    let (_dir, r) = repo();
    let g = goal(&r, "human-scope-goal");
    store::set_status(&r, &g, EntryStatus::Active).unwrap();
    let d = create(
        &r,
        EntryType::Decision,
        "human-scope-decision",
        "HUMAN LABEL PROSE",
    );
    store::set_status(&r, &d, EntryStatus::Accepted).unwrap();
    scoped(&r, &d, "production service policy");
    store::link(&r, &d, &g, LinkRelation::References).unwrap();
    record(&r, &d, "human-approval", "pass", "2020-01-01T00:00:00Z");
    for packet in [
        context::compile(&r, &g, ContextFormat::Agent, 5000, &[], false).unwrap(),
        context::compile_working_set(&r, ContextFormat::Agent, 5000, false).unwrap(),
    ] {
        assert!(packet.text.contains("scope=production service policy"));
        assert!(
            packet.text.contains(
                "current scope applicability Unknown; human scope meaning is not inferred"
            )
        );
    }
    assert_eq!(
        store::show(&r, &d).unwrap().entry.metadata["scope"],
        MetadataValue::String("production service policy".into())
    );
}

#[test]
fn fragment_seeds_keep_parent_goal_policies_without_admitting_sibling_criteria() {
    let (_dir, r) = repo();
    let g = goal(&r, "parent-policy-goal");
    let other = goal(&r, "outside-policy-goal");
    let p = create(
        &r,
        EntryType::Plan,
        "mapped-policy-plan",
        &plan_body("short", "not-started"),
    );
    store::link(&r, &p, &g, LinkRelation::Fulfills).unwrap();
    let mut outside_decision = String::new();
    for (title, scope, prose) in [
        ("parent-choice", g.clone(), "PARENT POLICY RETAINED"),
        (
            "criterion-choice",
            format!("{g}#sc-001"),
            "CURRENT CRITERION RETAINED",
        ),
        (
            "sibling-choice",
            format!("{g}#sc-002"),
            "SIBLING POLICY PROSE",
        ),
        ("outside-choice", other.clone(), "OTHER GOAL PROSE"),
    ] {
        let d = create(
            &r,
            EntryType::Decision,
            title,
            &format!("seed-policy-query {prose}"),
        );
        store::set_status(&r, &d, EntryStatus::Accepted).unwrap();
        scoped(&r, &d, &scope);
        store::link(&r, &d, &g, LinkRelation::References).unwrap();
        store::link(&r, &d, &p, LinkRelation::References).unwrap();
        store::link(&r, &d, &other, LinkRelation::References).unwrap();
        if title == "outside-choice" {
            outside_decision = d.clone();
        }
        record(&r, &d, "human-approval", "pass", "2020-01-01T00:00:00Z");
    }
    for (seed, fragment_selected) in [
        (p.clone(), false),
        (g.clone(), false),
        (format!("{p}#t-001"), true),
        (format!("{g}#sc-001"), true),
    ] {
        for (query, format) in [
            "seed-policy-query",
            "parent-policy-goal",
            "mapped-policy-plan",
            "outside-policy-goal",
        ]
        .into_iter()
        .flat_map(|query| [ContextFormat::Agent, ContextFormat::Human].map(|f| (query, f)))
        {
            let packet =
                context::compile(&r, query, format, 8000, std::slice::from_ref(&seed), false)
                    .unwrap();
            assert!(
                packet.text.contains("PARENT POLICY RETAINED"),
                "{query}, {seed}: {}",
                packet.text
            );
            assert!(
                packet.text.contains("CURRENT CRITERION RETAINED"),
                "{query}, {seed}: {}",
                packet.text
            );
            assert!(
                !packet.text.contains("OTHER GOAL PROSE"),
                "{query}, {seed}: {}",
                packet.text
            );
            assert_eq!(
                packet.text.contains("SIBLING POLICY PROSE"),
                !fragment_selected,
                "{query}, {seed}: {}",
                packet.text
            );
            assert!(packet.text.contains(&format!("scope={g}")));
        }
    }
    for format in [ContextFormat::Agent, ContextFormat::Human] {
        for query in [
            "parent-policy-goal",
            "mapped-policy-plan",
            "outside-policy-goal",
        ] {
            let packet = context::compile(&r, query, format, 8000, &[], false).unwrap();
            let parent_selected = query != "outside-policy-goal";
            for prose in [
                "PARENT POLICY RETAINED",
                "CURRENT CRITERION RETAINED",
                "SIBLING POLICY PROSE",
            ] {
                assert_eq!(
                    packet.text.contains(prose),
                    parent_selected,
                    "{query}: {}",
                    packet.text
                );
            }
            assert_eq!(
                packet.text.contains("OTHER GOAL PROSE"),
                !parent_selected,
                "{query}: {}",
                packet.text
            );
        }
        for (seeds, fragment_selected) in [
            (vec![g.clone(), other.clone()], false),
            (vec![format!("{g}#sc-001"), format!("{other}#sc-001")], true),
        ] {
            let packet =
                context::compile(&r, "mapped-policy-plan", format, 8000, &seeds, false).unwrap();
            assert!(packet.text.contains("PARENT POLICY RETAINED"));
            assert!(packet.text.contains("CURRENT CRITERION RETAINED"));
            assert!(packet.text.contains("OTHER GOAL PROSE"));
            assert_eq!(
                packet.text.contains("SIBLING POLICY PROSE"),
                !fragment_selected,
                "{}",
                packet.text
            );
        }
        let packet = context::compile(
            &r,
            "outside-policy-goal",
            format,
            8000,
            &[format!("{g}#sc-001"), outside_decision.clone()],
            false,
        )
        .unwrap();
        assert!(packet.text.contains("OTHER GOAL PROSE"));
        assert!(packet.text.contains(&format!("scope={other}")));
        assert!(
            packet
                .text
                .contains("included only by explicit seed; current applicability Unknown")
        );
        assert!(!packet.text.contains("SIBLING POLICY PROSE"));
    }
}
