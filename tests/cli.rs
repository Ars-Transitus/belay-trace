use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use rusqlite::Connection;
use tempfile::tempdir;

use belay_trace::entry::{Entry, EntryLink, EntryStatus, EntryType, LinkRelation};
use belay_trace::markdown::{self, estimate_tokens};

fn belay() -> Command {
    Command::new(env!("CARGO_BIN_EXE_belay"))
}

#[test]
fn cli_version_matches_the_package_and_lockfile() {
    let output = belay().arg("--version").output().expect("run --version");
    assert!(output.status.success(), "{output:?}");
    let expected = env!("CARGO_PKG_VERSION");
    assert_eq!(
        String::from_utf8(output.stdout)
            .expect("version stdout")
            .trim(),
        format!("belay {expected}")
    );

    let lockfile = fs::read_to_string(format!("{}/Cargo.lock", env!("CARGO_MANIFEST_DIR")))
        .expect("read Cargo.lock");
    let package = lockfile
        .split("[[package]]")
        .find(|package| package.trim_start().starts_with("name = \"belay-trace\""))
        .expect("belay-trace package in Cargo.lock");
    assert!(
        package.contains(&format!("version = \"{expected}\"")),
        "Cargo.lock package version must match Cargo.toml"
    );
}

#[cfg(unix)]
fn update_existing_project_script() -> Command {
    let mut command = Command::new("sh");
    command.arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/scripts/update-existing-project.sh"
    ));
    command
}

#[test]
fn top_level_help_describes_commands_workflow_and_exit_categories() {
    let output = belay().arg("--help").output().expect("run belay --help");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help is UTF-8");

    for expected in [
        "Usage:",
        "Commands:",
        "Behavior and Side Effects:",
        "Examples:",
        "Exit Status:",
        "Related Commands:",
        "Workflow groups:",
        "init",
        "add",
        "link",
        "status",
        "sync",
        "search",
        "browse",
        "context",
        "archive",
        "route",
        "doctor",
        "rebuild",
        "export",
    ] {
        assert!(stdout.contains(expected), "missing help text: {expected}");
    }
}

#[test]
fn every_command_help_has_the_required_structure() {
    for command in [
        "init", "add", "link", "status", "show", "search", "browse", "context", "archive", "route",
        "sync", "rebuild", "export", "doctor",
    ] {
        let output = belay()
            .args([command, "--help"])
            .output()
            .unwrap_or_else(|error| panic!("run help for {command}: {error}"));
        assert!(output.status.success(), "help failed for {command}");
        let stdout = String::from_utf8(output.stdout).expect("help is UTF-8");
        for expected in [
            "Usage:",
            "Behavior and Side Effects:",
            "Examples:",
            "Exit Status:",
            "Related Commands:",
        ] {
            assert!(
                stdout.contains(expected),
                "{command} help is missing {expected}"
            );
        }
        assert!(
            stdout.contains("Arguments:") || stdout.contains("Options:"),
            "{command} help is missing Arguments or Options"
        );
    }
}

#[test]
fn doctor_help_separates_repository_state_from_runtime_observations() {
    let output = belay()
        .args(["doctor", "--help"])
        .output()
        .expect("run doctor help");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help is UTF-8");
    for expected in [
        "Agent Integration States:",
        "generated present",
        "inactive",
        "active",
        "stale",
        "malformed",
        "repository observations",
        "does not claim that an agent runtime",
        "belay init --install-skill codex",
        "belay init --install-skill claude",
    ] {
        assert!(
            stdout.contains(expected),
            "missing doctor guidance: {expected}"
        );
    }
}

#[test]
fn browse_help_is_loopback_only_and_uninitialized_is_exit_three() {
    let help = belay()
        .args(["browse", "--help"])
        .output()
        .expect("run browse help");
    assert!(help.status.success());
    let stdout = String::from_utf8(help.stdout).expect("help is UTF-8");
    assert!(stdout.contains("127.0.0.1"));
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--open"));
    assert!(!stdout.contains("--host"));

    let temporary = tempdir().expect("create temporary repository");
    fs::create_dir(temporary.path().join(".git")).expect("create repository marker");
    let output = belay()
        .arg("browse")
        .current_dir(temporary.path())
        .output()
        .expect("run browse before init");
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn browse_fixed_port_bind_failure_is_exit_six() {
    let temporary = initialize_repository();
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("reserve loopback port");
    let port = listener.local_addr().expect("read bound address").port();
    let output = belay()
        .args(["browse", "--port", &port.to_string()])
        .current_dir(temporary.path())
        .output()
        .expect("run Browse on occupied port");
    assert_eq!(output.status.code(), Some(6));
    assert!(String::from_utf8_lossy(&output.stderr).contains("could not bind"));
}

#[test]
fn state_changing_command_help_has_realistic_examples_and_exit_categories() {
    for command in ["init", "add", "link", "status", "sync", "rebuild", "export"] {
        let output = belay()
            .args([command, "--help"])
            .output()
            .unwrap_or_else(|error| panic!("run help for {command}: {error}"));
        assert!(output.status.success(), "help failed for {command}");
        let stdout = String::from_utf8(output.stdout).expect("help is UTF-8");
        assert!(
            stdout.contains(&format!("belay {command}")),
            "{command} help lacks a realistic command example"
        );
        for exit_code in ["0 ", "2 ", "3 ", "4 ", "6 "] {
            assert!(
                stdout.contains(exit_code),
                "{command} help is missing exit category {exit_code:?}"
            );
        }
    }
}

#[test]
fn init_is_idempotent_and_does_not_modify_agents_md() {
    let temporary = tempdir().expect("create temp directory");
    fs::create_dir(temporary.path().join(".git")).expect("create repository marker");
    fs::write(temporary.path().join("AGENTS.md"), "keep me\n").expect("write AGENTS.md");

    let first = belay()
        .arg("init")
        .current_dir(temporary.path())
        .output()
        .expect("run first init");
    assert!(first.status.success(), "{:?}", first);

    let second = belay()
        .arg("init")
        .current_dir(temporary.path())
        .output()
        .expect("run second init");
    assert!(second.status.success(), "{:?}", second);
    assert_eq!(
        fs::read_to_string(temporary.path().join("AGENTS.md")).expect("read AGENTS.md"),
        "keep me\n"
    );
    assert!(!temporary.path().join(".agents").exists());
    let snippet = fs::read_to_string(temporary.path().join(".belay/agent/AGENTS.md.snippet"))
        .expect("read generated snippet");
    let skill = fs::read_to_string(temporary.path().join(".belay/agent/codex/SKILL.md"))
        .expect("read generated skill");
    let claude_skill = fs::read_to_string(temporary.path().join(".belay/agent/claude/SKILL.md"))
        .expect("read generated Claude skill");
    assert!(snippet.contains("For Tier 2 and Tier 3 work"));
    assert!(snippet.contains("repository-installed"));
    assert!(snippet.contains("belay context compile"));
    assert!(snippet.contains("human approval gates"));
    assert!(!snippet.contains("### Delivery assurance"));
    assert!(skill.contains("Repository-specific policy"));
    assert!(skill.contains("Use for Tier 2 or Tier 3 coding work"));
    assert!(skill.contains("## Command reference"));
    assert!(skill.contains("## Token discipline"));
    assert!(skill.contains("belay sync\n"));
    assert!(!skill.contains("grep -v"));
    assert!(skill.contains("## Frame"));
    assert!(skill.contains("## Map"));
    assert!(skill.contains("## Execute"));
    assert!(skill.contains("## Assure completion"));
    assert!(skill.contains("implemented, unverified"));
    assert!(skill.contains("None identified"));
    assert!(skill.contains("Entry titles must be short English kebab-case"));
    assert!(skill.contains("Use terminal statuses"));
    assert!(claude_skill.contains("`CLAUDE.md`"));
    assert_eq!(skill, claude_skill);
}

#[test]
fn repeated_init_refreshes_generated_assets_without_activating_them() {
    let temporary = initialize_repository();
    let snippet_path = temporary.path().join(".belay/agent/AGENTS.md.snippet");
    let skill_path = temporary.path().join(".belay/agent/codex/SKILL.md");
    let claude_skill_path = temporary.path().join(".belay/agent/claude/SKILL.md");
    let expected_snippet = fs::read_to_string(&snippet_path).expect("read generated snippet");
    let expected_skill = fs::read_to_string(&skill_path).expect("read generated skill");
    let expected_claude_skill =
        fs::read_to_string(&claude_skill_path).expect("read generated Claude skill");
    fs::write(&snippet_path, "stale snippet\n").expect("stale generated snippet");
    fs::write(&skill_path, "stale skill\n").expect("stale generated skill");
    fs::write(&claude_skill_path, "stale Claude skill\n").expect("stale generated Claude skill");

    let refreshed = belay()
        .arg("init")
        .current_dir(temporary.path())
        .output()
        .expect("refresh generated assets");
    assert!(refreshed.status.success(), "{refreshed:?}");
    assert_eq!(
        fs::read_to_string(snippet_path).expect("read refreshed snippet"),
        expected_snippet
    );
    assert_eq!(
        fs::read_to_string(skill_path).expect("read refreshed skill"),
        expected_skill
    );
    assert_eq!(
        fs::read_to_string(claude_skill_path).expect("read refreshed Claude skill"),
        expected_claude_skill
    );
    assert!(!temporary.path().join("AGENTS.md").exists());
    assert!(!temporary.path().join(".agents").exists());
}

#[cfg(unix)]
#[test]
fn update_script_refreshes_only_previously_active_integrations() {
    let temporary = tempdir().expect("create temp directory");
    fs::create_dir(temporary.path().join(".git")).expect("create repository marker");
    fs::write(temporary.path().join("AGENTS.md"), "# Project policy\n")
        .expect("write project policy");

    let initialized = belay()
        .args(["init", "--update-agents", "--install-skill", "codex"])
        .current_dir(temporary.path())
        .output()
        .expect("initialize integrations");
    assert!(initialized.status.success(), "{initialized:?}");

    fs::write(
        temporary.path().join(".belay/agent/AGENTS.md.snippet"),
        "stale generated snippet\n",
    )
    .expect("stale generated snippet");
    fs::write(
        temporary.path().join(".agents/skills/belay-trace/SKILL.md"),
        "stale installed skill\n",
    )
    .expect("stale installed skill");

    let tool_directory = temporary.path().join("tool directory");
    fs::create_dir(&tool_directory).expect("create spaced tool directory");
    std::os::unix::fs::symlink(
        env!("CARGO_BIN_EXE_belay"),
        tool_directory.join("belay executable"),
    )
    .expect("link belay through a spaced path");

    let updated = update_existing_project_script()
        .args(["--belay", "tool directory/belay executable", "."])
        .current_dir(temporary.path())
        .output()
        .expect("update existing project");
    assert!(updated.status.success(), "{updated:?}");

    let generated_snippet =
        fs::read_to_string(temporary.path().join(".belay/agent/AGENTS.md.snippet"))
            .expect("read generated snippet");
    let agents = fs::read_to_string(temporary.path().join("AGENTS.md")).expect("read AGENTS.md");
    assert!(generated_snippet.contains("For Tier 2 and Tier 3 work"));
    assert!(agents.starts_with("# Project policy\n"));
    assert!(agents.contains("For Tier 2 and Tier 3 work"));

    let generated_codex = fs::read_to_string(temporary.path().join(".belay/agent/codex/SKILL.md"))
        .expect("read generated Codex skill");
    let installed_codex =
        fs::read_to_string(temporary.path().join(".agents/skills/belay-trace/SKILL.md"))
            .expect("read installed Codex skill");
    assert_eq!(installed_codex, generated_codex);
    assert!(
        !temporary
            .path()
            .join(".claude/skills/belay-trace/SKILL.md")
            .exists()
    );
}

#[cfg(unix)]
#[test]
fn update_script_refuses_to_initialize_without_explicit_opt_in() {
    let temporary = tempdir().expect("create temp directory");
    fs::create_dir(temporary.path().join(".git")).expect("create repository marker");

    let rejected = update_existing_project_script()
        .args(["--belay", env!("CARGO_BIN_EXE_belay")])
        .arg(temporary.path())
        .output()
        .expect("reject uninitialized project");
    assert_eq!(rejected.status.code(), Some(2));
    assert!(!temporary.path().join(".belay").exists());
}

#[test]
fn update_agents_appends_replaces_and_stabilizes_only_the_managed_section() {
    let temporary = tempdir().expect("create temp directory");
    fs::create_dir(temporary.path().join(".git")).expect("create repository marker");
    fs::write(
        temporary.path().join("AGENTS.md"),
        "# Existing policy\n\nKeep this line.\n",
    )
    .expect("write AGENTS.md");

    let first = belay()
        .args(["init", "--update-agents"])
        .current_dir(temporary.path())
        .output()
        .expect("activate AGENTS integration");
    assert!(first.status.success(), "{first:?}");
    let activated =
        fs::read_to_string(temporary.path().join("AGENTS.md")).expect("read activated AGENTS.md");
    assert!(activated.starts_with("# Existing policy\n\nKeep this line.\n\n"));
    assert_eq!(activated.matches("<!-- belay-trace:start -->").count(), 1);
    assert_eq!(activated.matches("<!-- belay-trace:end -->").count(), 1);

    let stale = activated.replace(
        "Run `belay context",
        "This stale block says to run `belay context",
    );
    fs::write(temporary.path().join("AGENTS.md"), stale).expect("make managed section stale");
    let refreshed = belay()
        .args(["init", "--update-agents"])
        .current_dir(temporary.path())
        .output()
        .expect("refresh AGENTS integration");
    assert!(refreshed.status.success(), "{refreshed:?}");
    let refreshed =
        fs::read_to_string(temporary.path().join("AGENTS.md")).expect("read refreshed AGENTS.md");
    assert!(refreshed.starts_with("# Existing policy\n\nKeep this line.\n\n"));
    assert!(!refreshed.contains("This stale block"));

    let repeated = belay()
        .args(["init", "--update-agents"])
        .current_dir(temporary.path())
        .output()
        .expect("repeat AGENTS integration");
    assert!(repeated.status.success(), "{repeated:?}");
    assert_eq!(
        fs::read_to_string(temporary.path().join("AGENTS.md")).expect("read stable AGENTS.md"),
        refreshed
    );
    assert!(
        String::from_utf8(repeated.stdout)
            .expect("stdout is UTF-8")
            .contains("AGENTS.md integration unchanged")
    );
}

#[test]
fn update_agents_creates_missing_file_and_rejects_malformed_markers() {
    let temporary = tempdir().expect("create temp directory");
    fs::create_dir(temporary.path().join(".git")).expect("create repository marker");

    let created = belay()
        .args(["init", "--update-agents"])
        .current_dir(temporary.path())
        .output()
        .expect("create AGENTS.md");
    assert!(created.status.success(), "{created:?}");
    assert!(temporary.path().join("AGENTS.md").is_file());

    fs::write(
        temporary.path().join("AGENTS.md"),
        "keep\n<!-- belay-trace:start -->\nmalformed\n",
    )
    .expect("write malformed markers");
    let before =
        fs::read_to_string(temporary.path().join("AGENTS.md")).expect("read malformed AGENTS.md");
    let rejected = belay()
        .args(["init", "--update-agents"])
        .current_dir(temporary.path())
        .output()
        .expect("reject malformed markers");
    assert_eq!(rejected.status.code(), Some(4));
    assert_eq!(
        fs::read_to_string(temporary.path().join("AGENTS.md"))
            .expect("read unchanged malformed AGENTS.md"),
        before
    );
}

#[test]
fn install_codex_skill_is_explicit_repository_scoped_and_idempotent() {
    let temporary = tempdir().expect("create temp directory");
    fs::create_dir(temporary.path().join(".git")).expect("create repository marker");

    let installed = belay()
        .args(["init", "--install-skill", "codex"])
        .current_dir(temporary.path())
        .output()
        .expect("install Codex skill");
    assert!(installed.status.success(), "{installed:?}");
    let path = temporary.path().join(".agents/skills/belay-trace/SKILL.md");
    let generated = fs::read_to_string(temporary.path().join(".belay/agent/codex/SKILL.md"))
        .expect("read generated skill");
    assert_eq!(
        fs::read_to_string(&path).expect("read installed skill"),
        generated
    );

    let repeated = belay()
        .args(["init", "--install-skill", "codex"])
        .current_dir(temporary.path())
        .output()
        .expect("repeat Codex skill install");
    assert!(repeated.status.success(), "{repeated:?}");
    assert!(
        String::from_utf8(repeated.stdout)
            .expect("stdout is UTF-8")
            .contains("Codex skill unchanged")
    );
}

#[test]
fn install_claude_skill_is_explicit_repository_scoped_and_idempotent() {
    let temporary = tempdir().expect("create temp directory");
    fs::create_dir(temporary.path().join(".git")).expect("create repository marker");

    let installed = belay()
        .args(["init", "--install-skill", "claude"])
        .current_dir(temporary.path())
        .output()
        .expect("install Claude skill");
    assert!(installed.status.success(), "{installed:?}");
    let path = temporary.path().join(".claude/skills/belay-trace/SKILL.md");
    let generated = fs::read_to_string(temporary.path().join(".belay/agent/claude/SKILL.md"))
        .expect("read generated Claude skill");
    assert_eq!(
        fs::read_to_string(&path).expect("read installed Claude skill"),
        generated
    );

    let repeated = belay()
        .args(["init", "--install-skill", "claude"])
        .current_dir(temporary.path())
        .output()
        .expect("repeat Claude skill install");
    assert!(repeated.status.success(), "{repeated:?}");
    assert!(
        String::from_utf8(repeated.stdout)
            .expect("stdout is UTF-8")
            .contains("Claude skill unchanged")
    );
}

#[test]
fn install_skill_option_is_repeatable_for_multiple_explicit_targets() {
    let temporary = tempdir().expect("create temp directory");
    fs::create_dir(temporary.path().join(".git")).expect("create repository marker");

    let installed = belay()
        .args([
            "init",
            "--install-skill",
            "codex",
            "--install-skill",
            "claude",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("install both skills");
    assert!(installed.status.success(), "{installed:?}");
    assert!(
        temporary
            .path()
            .join(".agents/skills/belay-trace/SKILL.md")
            .is_file()
    );
    assert!(
        temporary
            .path()
            .join(".claude/skills/belay-trace/SKILL.md")
            .is_file()
    );
}

#[test]
fn init_reset_state_atomically_rebuilds_from_tracked_markdown() {
    let temporary = initialize_repository();
    let added = belay()
        .args([
            "add",
            "note",
            "--title",
            "Local-only ghost",
            "--body",
            "temporary",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("add note");
    assert!(added.status.success(), "{added:?}");

    let note_path = fs::read_dir(temporary.path().join(".belay/entries/notes"))
        .expect("read notes")
        .next()
        .expect("created note")
        .expect("read note path")
        .path();
    fs::remove_file(note_path).expect("remove tracked mirror");

    let reset = belay()
        .args(["init", "--reset-state"])
        .current_dir(temporary.path())
        .output()
        .expect("reset local state");
    assert!(reset.status.success(), "{reset:?}");
    assert!(
        String::from_utf8(reset.stdout)
            .expect("stdout is UTF-8")
            .contains("Rebuilt local state from 0 Markdown entries")
    );

    let database = Connection::open(temporary.path().join(".belay/state/belay.sqlite"))
        .expect("open rebuilt database");
    let count: i64 = database
        .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
        .expect("count rebuilt entries");
    assert_eq!(count, 0);
}

#[test]
fn doctor_reports_generated_active_inactive_stale_and_missing_agent_states() {
    let temporary = initialize_repository();

    let inactive = belay()
        .arg("doctor")
        .current_dir(temporary.path())
        .output()
        .expect("doctor inactive integration");
    assert!(inactive.status.success(), "{inactive:?}");
    let stdout = String::from_utf8(inactive.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("generated AGENTS snippet: present"));
    assert!(stdout.contains("canonical generated artifact exists"));
    assert!(stdout.contains("AGENTS.md integration: inactive"));
    assert!(stdout.contains("belay init --update-agents"));
    assert!(stdout.contains("generated Codex skill: present"));
    assert!(stdout.contains("installed Codex skill: inactive"));
    assert!(stdout.contains("belay init --install-skill codex"));
    assert!(stdout.contains("generated Claude skill: present"));
    assert!(stdout.contains("installed Claude skill: inactive"));
    assert!(stdout.contains("belay init --install-skill claude"));

    let activated = belay()
        .args(["init", "--update-agents", "--install-skill", "codex"])
        .current_dir(temporary.path())
        .output()
        .expect("activate integrations");
    assert!(activated.status.success(), "{activated:?}");
    let activated_claude = belay()
        .args(["init", "--install-skill", "claude"])
        .current_dir(temporary.path())
        .output()
        .expect("activate Claude integration");
    assert!(activated_claude.status.success(), "{activated_claude:?}");
    let active = belay()
        .arg("doctor")
        .current_dir(temporary.path())
        .output()
        .expect("doctor active integration");
    assert!(active.status.success(), "{active:?}");
    let stdout = String::from_utf8(active.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("AGENTS.md integration: active"));
    assert!(stdout.contains("installed Codex skill: active"));
    assert!(stdout.contains("installed Claude skill: active"));
    assert!(stdout.contains("runtime recognition and execution are not checked"));

    fs::write(
        temporary.path().join(".agents/skills/belay-trace/SKILL.md"),
        "stale\n",
    )
    .expect("stale installed skill");
    fs::write(
        temporary.path().join(".claude/skills/belay-trace/SKILL.md"),
        "stale\n",
    )
    .expect("stale installed Claude skill");
    fs::remove_file(temporary.path().join(".belay/agent/AGENTS.md.snippet"))
        .expect("remove generated snippet");
    let unhealthy = belay()
        .arg("doctor")
        .current_dir(temporary.path())
        .output()
        .expect("doctor unhealthy integration");
    assert_eq!(unhealthy.status.code(), Some(5));
    let stdout = String::from_utf8(unhealthy.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("generated AGENTS snippet: missing"));
    assert!(stdout.contains("run `belay init` to refresh canonical generated artifacts"));
    assert!(stdout.contains("installed Codex skill: stale"));
    assert!(stdout.contains("run `belay init --install-skill codex`"));
    assert!(stdout.contains("installed Claude skill: stale"));
    assert!(stdout.contains("run `belay init --install-skill claude`"));
    let stderr = String::from_utf8(unhealthy.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("belay init"));
}

#[test]
fn doctor_classifies_generated_and_agents_drift_and_malformed_markers() {
    let temporary = initialize_repository();
    let generated_skill = temporary.path().join(".belay/agent/codex/SKILL.md");

    fs::write(&generated_skill, "stale generated skill\n").expect("stale generated skill");
    let stale_generated = belay()
        .arg("doctor")
        .current_dir(temporary.path())
        .output()
        .expect("doctor stale generated skill");
    assert_eq!(stale_generated.status.code(), Some(5));
    assert!(
        String::from_utf8(stale_generated.stdout)
            .expect("stdout is UTF-8")
            .contains("generated Codex skill: stale")
    );

    let repaired = belay()
        .arg("init")
        .current_dir(temporary.path())
        .output()
        .expect("repair generated skill");
    assert!(repaired.status.success(), "{repaired:?}");

    let activated = belay()
        .args(["init", "--update-agents"])
        .current_dir(temporary.path())
        .output()
        .expect("activate AGENTS integration");
    assert!(activated.status.success(), "{activated:?}");
    let agents_path = temporary.path().join("AGENTS.md");
    let stale_agents = fs::read_to_string(&agents_path)
        .expect("read AGENTS.md")
        .replace(
            "For Tier 2 and Tier 3 work",
            "For stale Tier 2 and Tier 3 work",
        );
    fs::write(&agents_path, stale_agents).expect("stale AGENTS integration");
    let stale = belay()
        .arg("doctor")
        .current_dir(temporary.path())
        .output()
        .expect("doctor stale AGENTS integration");
    assert_eq!(stale.status.code(), Some(5));
    assert!(
        String::from_utf8(stale.stdout)
            .expect("stdout is UTF-8")
            .contains("AGENTS.md integration: stale")
    );

    fs::write(
        &agents_path,
        "keep\n<!-- belay-trace:start -->\nmalformed\n",
    )
    .expect("write malformed AGENTS markers");
    let malformed = belay()
        .arg("doctor")
        .current_dir(temporary.path())
        .output()
        .expect("doctor malformed AGENTS integration");
    assert_eq!(malformed.status.code(), Some(4));
    let stdout = String::from_utf8(malformed.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("AGENTS.md integration: malformed"));
    assert!(stdout.contains("repair to exactly one ordered belay-trace marker pair"));
    assert!(
        String::from_utf8(malformed.stderr)
            .expect("stderr is UTF-8")
            .contains("repair the marker pair")
    );
}

#[cfg(unix)]
#[test]
fn agent_integration_rejects_symlinks_and_non_regular_files() {
    use std::os::unix::fs::symlink;

    let temporary = initialize_repository();
    let external = temporary.path().join("external-agents.md");
    fs::write(&external, "external\n").expect("write external file");
    symlink(&external, temporary.path().join("AGENTS.md")).expect("symlink AGENTS.md");

    let update = belay()
        .args(["init", "--update-agents"])
        .current_dir(temporary.path())
        .output()
        .expect("reject symlinked AGENTS.md");
    assert_eq!(update.status.code(), Some(4));
    assert_eq!(
        fs::read_to_string(&external).expect("read untouched external file"),
        "external\n"
    );

    fs::remove_file(temporary.path().join("AGENTS.md")).expect("remove AGENTS symlink");
    fs::create_dir(temporary.path().join("AGENTS.md")).expect("create non-regular AGENTS path");
    let doctor = belay()
        .arg("doctor")
        .current_dir(temporary.path())
        .output()
        .expect("reject non-regular AGENTS.md");
    assert_eq!(doctor.status.code(), Some(4));

    fs::remove_dir(temporary.path().join("AGENTS.md")).expect("remove AGENTS directory");
    let external_directory = temporary.path().join("external-skill-directory");
    fs::create_dir(&external_directory).expect("create external directory");
    symlink(&external_directory, temporary.path().join(".agents"))
        .expect("symlink skill directory");
    let install = belay()
        .args(["init", "--install-skill", "codex"])
        .current_dir(temporary.path())
        .output()
        .expect("reject symlinked skill directory");
    assert_eq!(install.status.code(), Some(4));
    assert!(
        fs::read_dir(&external_directory)
            .expect("read untouched external directory")
            .next()
            .is_none()
    );

    fs::remove_file(temporary.path().join(".agents")).expect("remove skill directory symlink");
    symlink(&external_directory, temporary.path().join(".claude"))
        .expect("symlink Claude skill directory");
    let install = belay()
        .args(["init", "--install-skill", "claude"])
        .current_dir(temporary.path())
        .output()
        .expect("reject symlinked Claude skill directory");
    assert_eq!(install.status.code(), Some(4));
    assert!(
        fs::read_dir(&external_directory)
            .expect("read untouched external directory")
            .next()
            .is_none()
    );
}

#[test]
fn uninitialized_project_command_fails_without_mutation_and_suggests_init() {
    let temporary = tempdir().expect("create temp directory");
    fs::create_dir(temporary.path().join(".git")).expect("create repository marker");

    let output = belay()
        .arg("doctor")
        .current_dir(temporary.path())
        .output()
        .expect("run doctor");

    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains(&temporary.path().display().to_string()));
    assert!(stderr.contains("belay init"));
    assert!(!temporary.path().join(".belay").exists());
}

#[test]
fn project_command_does_not_recreate_a_missing_database() {
    let temporary = initialize_repository();
    let database_path = temporary.path().join(".belay/state/belay.sqlite");
    fs::remove_file(&database_path).expect("remove database");

    let output = belay()
        .args(["search", "sqlite"])
        .current_dir(temporary.path())
        .output()
        .expect("search without database");

    assert_eq!(output.status.code(), Some(6));
    assert!(!database_path.exists());
}

#[test]
fn invalid_config_uses_validation_exit_category() {
    let temporary = tempdir().expect("create temp directory");
    fs::create_dir(temporary.path().join(".git")).expect("create repository marker");
    fs::create_dir(temporary.path().join(".belay")).expect("create belay directory");
    fs::write(
        temporary.path().join(".belay/config.toml"),
        "schema_version = 99\n",
    )
    .expect("write invalid config");

    let output = belay()
        .arg("init")
        .current_dir(temporary.path())
        .output()
        .expect("run init");

    assert_eq!(output.status.code(), Some(4));
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("configuration"));
    assert!(stderr.contains("invalid"));
}

#[test]
fn newer_database_schema_uses_validation_exit_category() {
    let temporary = tempdir().expect("create temp directory");
    fs::create_dir(temporary.path().join(".git")).expect("create repository marker");

    let initialized = belay()
        .arg("init")
        .current_dir(temporary.path())
        .output()
        .expect("run init");
    assert!(initialized.status.success());

    let database_path = temporary.path().join(".belay/state/belay.sqlite");
    let connection = Connection::open(database_path).expect("open database");
    connection
        .execute(
            "
            INSERT INTO schema_migrations(version, name, applied_at)
            VALUES (99, 'future schema', '2026-06-06T00:00:00Z')
            ",
            [],
        )
        .expect("insert future migration");

    let output = belay()
        .arg("init")
        .current_dir(temporary.path())
        .output()
        .expect("run init again");

    assert_eq!(output.status.code(), Some(4));
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("newer than supported"));
}

fn initialize_repository() -> tempfile::TempDir {
    let temporary = tempdir().expect("create temp directory");
    fs::create_dir(temporary.path().join(".git")).expect("create repository marker");
    let output = belay()
        .arg("init")
        .current_dir(temporary.path())
        .output()
        .expect("run init");
    assert!(output.status.success(), "{output:?}");
    temporary
}

fn created_id(output: &std::process::Output) -> String {
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout.clone())
        .expect("stdout is UTF-8")
        .trim()
        .strip_prefix("Created ")
        .expect("created output")
        .to_owned()
}

fn mirror_path(
    repository: &std::path::Path,
    entry_type: &str,
    display_id: &str,
) -> std::path::PathBuf {
    repository
        .join(".belay/entries")
        .join(entry_type)
        .join(format!("{display_id}.md"))
}

#[test]
fn add_supports_every_type_and_body_source() {
    let temporary = initialize_repository();
    let body_path = temporary.path().join("body.md");
    fs::write(&body_path, "Body from file\n").expect("write body file");

    for entry_type in ["plan", "decision", "work", "review", "note"] {
        let inline = belay()
            .args([
                "add",
                entry_type,
                "--title",
                &format!("{entry_type} inline"),
                "--body",
                "Inline body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add inline entry");
        created_id(&inline);

        let file = belay()
            .args([
                "add",
                entry_type,
                "--title",
                &format!("{entry_type} file"),
                "--body-file",
            ])
            .arg(&body_path)
            .current_dir(temporary.path())
            .output()
            .expect("add file entry");
        created_id(&file);

        let mut child = belay()
            .args([
                "add",
                entry_type,
                "--title",
                &format!("{entry_type} stdin"),
                "--stdin",
            ])
            .current_dir(temporary.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn stdin entry");
        child
            .stdin
            .take()
            .expect("stdin pipe")
            .write_all(b"Body from stdin\n")
            .expect("write stdin");
        let stdin = child.wait_with_output().expect("wait for stdin entry");
        created_id(&stdin);
    }

    let connection =
        Connection::open(temporary.path().join(".belay/state/belay.sqlite")).expect("open DB");
    let entry_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
        .expect("count entries");
    let sync_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM sync_state", [], |row| row.get(0))
        .expect("count sync states");
    let fts_count: i64 = connection
        .query_row(
            "SELECT COUNT(DISTINCT entry_id) FROM entry_fts",
            [],
            |row| row.get(0),
        )
        .expect("count FTS entries");
    let mismatched_baselines: i64 = connection
        .query_row(
            "
            SELECT COUNT(*)
            FROM sync_state
            WHERE sqlite_content_hash_at_last_sync != mirror_content_hash_at_last_sync
            ",
            [],
            |row| row.get(0),
        )
        .expect("count mismatched sync baselines");
    assert_eq!(entry_count, 15);
    assert_eq!(sync_count, 15);
    assert_eq!(fts_count, 15);
    assert_eq!(mismatched_baselines, 0);
}

#[test]
fn invalid_add_does_not_allocate_or_write() {
    let temporary = initialize_repository();
    let invalid = belay()
        .args(["add", "decision", "--title", " ", "--body", "invalid title"])
        .current_dir(temporary.path())
        .output()
        .expect("run invalid add");
    assert_eq!(invalid.status.code(), Some(4));

    let valid = belay()
        .args(["add", "decision", "--title", "Valid", "--body", "body"])
        .current_dir(temporary.path())
        .output()
        .expect("run valid add");
    let id = created_id(&valid);
    assert!(id.contains("-001-"));

    let mirror_count = fs::read_dir(temporary.path().join(".belay/entries/decisions"))
        .expect("read mirrors")
        .count();
    assert_eq!(mirror_count, 1);
}

const CONFORMING_PLAN: &str = concat!(
    "## Delivery Map\n\n",
    "| ID | Goal item | Outcome / Task | State |\n",
    "| --- | --- | --- | --- |\n",
    "| T-001 | SC-001 | Do the thing | verified |\n\n",
    "## T-001\n\n",
    "- **Objective**: do the thing.\n",
    "- **Scope**: in — one file. Out — everything else.\n",
    "- **Steps**:\n  1. Do it.\n",
    "- **Acceptance**: SC-001 holds.\n",
    "- **Verification**: `cargo test`.\n",
);

fn add_plan(temporary: &tempfile::TempDir, title: &str, body: &str) -> String {
    created_id(
        &belay()
            .args(["add", "plan", "--title", title, "--body", body])
            .current_dir(temporary.path())
            .output()
            .expect("add plan"),
    )
}

fn plan_lint(temporary: &tempfile::TempDir, args: &[&str]) -> std::process::Output {
    let mut command = belay();
    command.arg("plan").arg("lint");
    for argument in args {
        command.arg(argument);
    }
    command
        .current_dir(temporary.path())
        .output()
        .expect("plan lint")
}

/// Characterizes what several processes writing at once actually do, so a
/// consumer can cite a behavior instead of guessing one. `create` opens an
/// IMMEDIATE transaction, writes the Markdown mirror while still holding that
/// write lock, and only then commits, so writers serialize rather than
/// interleave. This test is the evidence for that claim; if it starts failing,
/// the documented guarantee in `docs/id-reference-standard.md` is wrong.
#[test]
fn concurrent_writers_serialize_without_losing_entries_or_drifting() {
    let temporary = initialize_repository();
    let writers = 8;

    let children = (0..writers)
        .map(|index| {
            belay()
                .args([
                    "add",
                    "note",
                    "--title",
                    &format!("concurrent writer {index}"),
                    "--body",
                    "Written concurrently.",
                ])
                .current_dir(temporary.path())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn concurrent writer")
        })
        .collect::<Vec<_>>();
    let outcomes = children
        .into_iter()
        .map(|child| child.wait_with_output().expect("writer finished"))
        .collect::<Vec<_>>();

    for outcome in &outcomes {
        assert!(
            outcome.status.success(),
            "a concurrent writer must block on the lock, not fail: {outcome:?}"
        );
    }

    let ids = outcomes.iter().map(created_id).collect::<BTreeSet<_>>();
    assert_eq!(
        ids.len(),
        writers,
        "display IDs are allocated inside the transaction, so they must not collide"
    );

    let mirrored = fs::read_dir(temporary.path().join(".belay/entries/notes"))
        .expect("notes directory")
        .filter_map(Result::ok)
        .count();
    assert_eq!(
        mirrored, writers,
        "every committed entry must have a mirror"
    );

    let doctor = belay()
        .arg("doctor")
        .current_dir(temporary.path())
        .output()
        .expect("doctor");
    let report = String::from_utf8_lossy(&doctor.stdout).to_string();
    assert!(
        report.contains("SQLite/Markdown drift: ok"),
        "concurrent writes must leave SQLite and the mirror in agreement: {report}"
    );
}

/// The `add` case has writers touching different rows. This one has them
/// contending for the same one, which is where a lost update would show.
#[test]
fn concurrent_mutations_of_one_entry_leave_a_consistent_result() {
    let temporary = initialize_repository();
    let target = created_id(
        &belay()
            .args(["add", "decision", "--title", "Contended", "--body", "Body"])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let sources = (0..4)
        .map(|index| {
            created_id(
                &belay()
                    .args([
                        "add",
                        "work",
                        "--title",
                        &format!("linker {index}"),
                        "--body",
                        "Body",
                    ])
                    .current_dir(temporary.path())
                    .output()
                    .expect("add work"),
            )
        })
        .collect::<Vec<_>>();

    let mut children = sources
        .iter()
        .map(|source| {
            belay()
                .args(["link", source, &target, "--relation", "supports"])
                .current_dir(temporary.path())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn linker")
        })
        .collect::<Vec<_>>();
    children.push(
        belay()
            .args(["status", &target, "accepted"])
            .current_dir(temporary.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn status change"),
    );
    for child in children {
        let outcome = child.wait_with_output().expect("mutation finished");
        assert!(
            outcome.status.success(),
            "a contended mutation must serialize, not fail: {outcome:?}"
        );
    }

    let shown = belay()
        .args(["show", &target])
        .current_dir(temporary.path())
        .output()
        .expect("show target");
    let stdout = String::from_utf8(shown.stdout).expect("stdout");
    assert!(stdout.contains("Status: accepted"), "{stdout}");
    for source in &sources {
        assert!(
            stdout.contains(source.as_str()),
            "no link may be lost to a concurrent write: {stdout}"
        );
    }

    let doctor = belay()
        .arg("doctor")
        .current_dir(temporary.path())
        .output()
        .expect("doctor");
    let report = String::from_utf8_lossy(&doctor.stdout).to_string();
    assert!(
        report.contains("SQLite/Markdown drift: ok"),
        "contended writes must leave SQLite and the mirror in agreement: {report}"
    );
}

#[test]
fn sync_accepts_a_link_that_targets_a_task_fragment() {
    // `docs/id-reference-standard.md` §3 has Work link to a Plan task with
    // `implements`, so the documented usage must survive a sync.
    let temporary = initialize_repository();
    let plan = add_plan(&temporary, "Deliver sync", CONFORMING_PLAN);
    let work = created_id(
        &belay()
            .args(["add", "work", "--title", "Do it", "--body", "Work body"])
            .current_dir(temporary.path())
            .output()
            .expect("add work"),
    );
    let linked = belay()
        .args([
            "link",
            &work,
            &format!("{plan}#t-001"),
            "--relation",
            "implements",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("link to fragment");
    assert!(linked.status.success(), "{linked:?}");

    let synced = belay()
        .arg("sync")
        .current_dir(temporary.path())
        .output()
        .expect("sync");
    let stderr = String::from_utf8_lossy(&synced.stderr);
    assert!(
        synced.status.success() && !stderr.contains("links to missing entry"),
        "sync must not treat a fragment target as a missing entry: {synced:?}"
    );
}

#[test]
fn plan_lint_passes_a_conforming_plan_and_tolerates_extra_fields() {
    let temporary = initialize_repository();
    let extended = format!("{CONFORMING_PLAN}- **Difficulty**: medium.\n- **Owner**: someone.\n");
    let plan = add_plan(&temporary, "Deliver sync", &extended);

    let output = plan_lint(&temporary, &[&plan]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("No deterministic findings"), "{stdout}");
    // An extension field is a consumer's business, not a finding.
    assert!(!stdout.contains("Difficulty"), "{stdout}");
}

#[test]
fn plan_lint_reports_a_task_row_no_section_explains() {
    let temporary = initialize_repository();
    let body = concat!(
        "## Delivery Map\n\n",
        "| ID | Goal item | Outcome / Task | State |\n",
        "| --- | --- | --- | --- |\n",
        "| T-001 | SC-001 | Explained | verified |\n",
        "| T-002 | SC-001 | Unexplained | not-started |\n\n",
        "## T-001\n\n",
        "- **Objective**: do the thing.\n",
        "- **Scope**: in — one file.\n",
        "- **Steps**:\n  1. Do it.\n",
        "- **Acceptance**: SC-001 holds.\n",
        "- **Verification**: `cargo test`.\n",
    );
    let plan = add_plan(&temporary, "Partly mapped", body);

    let stdout = String::from_utf8(plan_lint(&temporary, &[&plan]).stdout).expect("stdout");
    assert!(
        stdout.contains("Delivery Map T-002: no `## T-002` section"),
        "{stdout}"
    );
    assert!(!stdout.contains("T-001: no `## T-001`"), "{stdout}");
}

#[test]
fn plan_lint_reports_missing_baseline_fields_and_unknown_states() {
    let temporary = initialize_repository();
    let body = concat!(
        "## Delivery Map\n\n",
        "| ID | Goal item | Outcome / Task | State |\n",
        "| --- | --- | --- | --- |\n",
        "| T-001 | SC-001 | Thin section | almost-done |\n\n",
        "## T-001\n\n",
        "- **Objective**: do the thing.\n",
    );
    let plan = add_plan(&temporary, "Thin plan", body);

    let stdout = String::from_utf8(plan_lint(&temporary, &[&plan]).stdout).expect("stdout");
    for field in ["Scope", "Steps", "Acceptance", "Verification"] {
        assert!(
            stdout.contains(&format!("T-001 {field}: task section is missing")),
            "{field} must be reported: {stdout}"
        );
    }
    assert!(!stdout.contains("T-001 Objective:"), "{stdout}");
    assert!(
        stdout.contains("state \"almost-done\" is not one of"),
        "{stdout}"
    );
}

#[test]
fn plan_lint_skips_a_plan_with_no_delivery_map() {
    let temporary = initialize_repository();
    let plan = add_plan(
        &temporary,
        "Prose only",
        "## Intent Brief\n\n- Just prose.\n",
    );

    let output = plan_lint(&temporary, &[&plan]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("structural checks skipped"), "{stdout}");
    assert!(!stdout.contains("Checklist:"), "{stdout}");
}

#[test]
fn plan_lint_matches_goal_lint_interface_shape() {
    let temporary = initialize_repository();
    let good = add_plan(&temporary, "Conforming", CONFORMING_PLAN);
    let bad = add_plan(
        &temporary,
        "Bare",
        "## Delivery Map\n\n| ID | Goal item | State |\n| --- | --- | --- |\n| T-001 | SC-001 | verified |\n",
    );

    let all = plan_lint(&temporary, &["--all"]);
    assert!(all.status.success(), "{all:?}");
    let stdout = String::from_utf8(all.stdout).expect("stdout");
    assert!(stdout.contains(&good) && stdout.contains(&bad), "{stdout}");

    let json = plan_lint(&temporary, &["--all", "--format", "json"]);
    assert!(json.status.success(), "{json:?}");
    let parsed: serde_json::Value =
        serde_json::from_slice(&json.stdout).expect("plan lint json must parse");
    let reports = parsed.as_array().expect("array of reports");
    assert_eq!(reports.len(), 2);
    assert!(reports[0].get("plan_id").is_some());
    assert!(reports[0].get("checklist_total").is_some());

    // Findings alone do not fail; --strict is what turns them into an error.
    let lenient = plan_lint(&temporary, &[&bad]);
    assert!(lenient.status.success(), "{lenient:?}");
    let strict = plan_lint(&temporary, &[&bad, "--strict"]);
    assert_eq!(strict.status.code(), Some(4), "{strict:?}");

    let both = plan_lint(&temporary, &[&good, "--all"]);
    assert_eq!(both.status.code(), Some(4), "{both:?}");
}

#[test]
fn show_with_a_fragment_returns_one_task_instead_of_the_entry() {
    let temporary = initialize_repository();
    let body = concat!(
        "## Delivery Map\n\n",
        "| ID | Outcome / Task | State |\n",
        "| --- | --- | --- |\n",
        "| T-001 | Documented task | not-started |\n",
        "| T-002 | Bare row with no section | not-started |\n\n",
        "## T-001\n\n",
        "- **Objective**: the detail a worker needs.\n",
    );
    let plan = created_id(
        &belay()
            .args(["add", "plan", "--title", "Deliver sync", "--body", body])
            .current_dir(temporary.path())
            .output()
            .expect("add plan"),
    );

    let scoped = belay()
        .args(["show", &format!("{plan}#t-001")])
        .current_dir(temporary.path())
        .output()
        .expect("show fragment");
    assert!(scoped.status.success(), "{scoped:?}");
    let scoped = String::from_utf8(scoped.stdout).expect("stdout");
    assert!(scoped.contains(&format!("ID: {plan}#t-001")), "{scoped}");
    assert!(scoped.contains("| T-001 | Documented task | not-started |"));
    assert!(scoped.contains("the detail a worker needs"));
    // The point of the fragment is that the rest of the entry stays unread.
    assert!(!scoped.contains("Bare row with no section"), "{scoped}");
    assert!(!scoped.contains("Outbound Links:"), "{scoped}");

    let whole = belay()
        .args(["show", &plan])
        .current_dir(temporary.path())
        .output()
        .expect("show entry");
    let whole = String::from_utf8(whole.stdout).expect("stdout");
    assert!(whole.contains("Bare row with no section"));
    assert!(whole.contains("Outbound Links:"));
    assert!(
        scoped.len() < whole.len(),
        "fragment output must be smaller than the entry: {} vs {}",
        scoped.len(),
        whole.len()
    );
}

#[test]
fn show_reports_a_resolvable_fragment_that_has_no_body_section() {
    let temporary = initialize_repository();
    let body = concat!(
        "## Delivery Map\n\n",
        "| ID | Outcome / Task | State |\n",
        "| --- | --- | --- |\n",
        "| T-001 | Bare row | not-started |\n",
    );
    let plan = created_id(
        &belay()
            .args(["add", "plan", "--title", "Bare map", "--body", body])
            .current_dir(temporary.path())
            .output()
            .expect("add plan"),
    );

    let output = belay()
        .args(["show", &format!("{plan}#t-001")])
        .current_dir(temporary.path())
        .output()
        .expect("show fragment");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("| T-001 | Bare row | not-started |"),
        "{stdout}"
    );
    assert!(stdout.contains("Section: none"), "{stdout}");
}

#[test]
fn show_rejects_a_fragment_instead_of_falling_back_to_the_entry() {
    let temporary = initialize_repository();
    let body = concat!(
        "## Delivery Map\n\n",
        "| ID | Outcome / Task | State |\n",
        "| --- | --- | --- |\n",
        "| T-001 | Documented task | not-started |\n",
    );
    let plan = created_id(
        &belay()
            .args(["add", "plan", "--title", "Deliver sync", "--body", body])
            .current_dir(temporary.path())
            .output()
            .expect("add plan"),
    );

    for fragment in ["t-099", "sc-001", "objective", "t-1"] {
        let output = belay()
            .args(["show", &format!("{plan}#{fragment}")])
            .current_dir(temporary.path())
            .output()
            .expect("show fragment");
        assert_eq!(
            output.status.code(),
            Some(4),
            "fragment {fragment} must fail: {output:?}"
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(
            !stdout.contains("Documented task"),
            "fragment {fragment} must not fall back to the entry: {stdout}"
        );
    }
}

#[test]
fn show_link_and_status_use_display_ids_and_keep_mirror_in_sync() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Use SQLite",
                "--body",
                "Decision body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let work = created_id(
        &belay()
            .args([
                "add",
                "work",
                "--title",
                "Implement storage",
                "--body",
                "Work body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add work"),
    );

    let linked = belay()
        .args(["link", &work, &decision, "--relation", "implements"])
        .current_dir(temporary.path())
        .output()
        .expect("link entries");
    assert!(linked.status.success(), "{linked:?}");
    let duplicate = belay()
        .args(["link", &work, &decision, "--relation", "implements"])
        .current_dir(temporary.path())
        .output()
        .expect("link entries again");
    assert!(duplicate.status.success());
    assert!(
        String::from_utf8(duplicate.stdout)
            .expect("stdout")
            .contains("already exists")
    );

    let status = belay()
        .args(["status", &decision, "accepted"])
        .current_dir(temporary.path())
        .output()
        .expect("update status");
    assert!(status.status.success(), "{status:?}");
    let same_status = belay()
        .args(["status", &decision, "accepted"])
        .current_dir(temporary.path())
        .output()
        .expect("repeat status");
    assert!(same_status.status.success());
    assert!(
        String::from_utf8(same_status.stdout)
            .expect("stdout")
            .contains("already has status")
    );

    let shown = belay()
        .args(["show", &decision])
        .current_dir(temporary.path())
        .output()
        .expect("show decision");
    assert!(shown.status.success(), "{shown:?}");
    let stdout = String::from_utf8(shown.stdout).expect("stdout");
    assert!(stdout.contains(&format!("ID: {decision}")));
    assert!(stdout.contains("Status: accepted"));
    assert!(stdout.contains("Metadata: {}"));
    assert!(stdout.contains(&format!("- {work} implements")));
    assert!(!stdout.contains("entry_id"));

    let connection =
        Connection::open(temporary.path().join(".belay/state/belay.sqlite")).expect("open DB");
    let link_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM entry_links", [], |row| row.get(0))
        .expect("count links");
    assert_eq!(link_count, 1);
    let link_id_types: (String, String) = connection
        .query_row(
            "
            SELECT typeof(from_entry_id), typeof(to_entry_id)
            FROM entry_links
            ",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read link ID types");
    assert_eq!(link_id_types, ("integer".to_owned(), "integer".to_owned()));
    let revision: i64 = connection
        .query_row(
            "SELECT revision FROM entries WHERE display_id = ?1",
            [&decision],
            |row| row.get(0),
        )
        .expect("read decision revision");
    assert_eq!(revision, 2);
    let work_revision: i64 = connection
        .query_row(
            "SELECT revision FROM entries WHERE display_id = ?1",
            [&work],
            |row| row.get(0),
        )
        .expect("read work revision");
    assert_eq!(work_revision, 2);

    let decision_mirror = temporary
        .path()
        .join(".belay/entries/decisions")
        .join(format!("{decision}.md"));
    let mirror = fs::read_to_string(decision_mirror).expect("read decision mirror");
    assert!(mirror.contains("status: accepted"));
    assert!(mirror.contains("revision: 2"));
    let work_mirror = temporary
        .path()
        .join(".belay/entries/work")
        .join(format!("{work}.md"));
    let mirror = fs::read_to_string(work_mirror).expect("read work mirror");
    assert!(mirror.contains(&format!("id: {decision}")));
    assert!(mirror.contains("relation: implements"));
}

#[test]
fn links_and_evidence_require_defined_goal_and_plan_fragments() {
    let temporary = initialize_repository();
    let goal = created_id(
        &belay()
            .args(["add", "goal", "--title", "Stable local IDs"])
            .current_dir(temporary.path())
            .output()
            .expect("add goal"),
    );
    let plan_body = "## Delivery Map\n\n| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |\n| --- | --- | --- | --- | --- | --- |\n| T-001 | SC-001 | Preserve IDs | AI | implemented | pending |\n";
    let plan = created_id(
        &belay()
            .args([
                "add",
                "plan",
                "--title",
                "Deliver stable local IDs",
                "--body",
                plan_body,
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add plan"),
    );
    let work = created_id(
        &belay()
            .args([
                "add",
                "work",
                "--title",
                "Implement stable local IDs",
                "--body",
                "Implementation",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add work"),
    );

    let criterion = format!("{goal}#SC-001");
    let task = format!("{plan}#t-001");
    for (target, relation) in [(&criterion, "fulfills"), (&task, "implements")] {
        let linked = belay()
            .args(["link", &work, target, "--relation", relation])
            .current_dir(temporary.path())
            .output()
            .expect("link fragment");
        assert!(linked.status.success(), "{linked:?}");
    }

    let work_mirror = temporary
        .path()
        .join(".belay/entries/work")
        .join(format!("{work}.md"));
    let mirror = fs::read_to_string(&work_mirror).expect("read work mirror");
    assert!(mirror.contains(&format!("id: {goal}#sc-001")));
    assert!(!mirror.contains(&format!("id: {goal}#SC-001")));

    for legacy in [format!("{goal}#sc-1"), format!("{plan}#task-t-1")] {
        let rejected = belay()
            .args(["link", &work, &legacy, "--relation", "references"])
            .current_dir(temporary.path())
            .output()
            .expect("reject legacy fragment");
        assert_eq!(rejected.status.code(), Some(4));
        assert!(
            String::from_utf8(rejected.stderr)
                .expect("legacy rejection is UTF-8")
                .contains("is not canonical")
        );
    }

    let rejected_evidence = belay()
        .args([
            "verify",
            "record",
            "--kind",
            "test",
            "--verdict",
            "pass",
            "--source",
            "cargo test",
            "--summary",
            "reject legacy fragment",
            "--verifies",
            &format!("{goal}#sc-1"),
        ])
        .current_dir(temporary.path())
        .output()
        .expect("reject evidence with legacy fragment");
    assert_eq!(rejected_evidence.status.code(), Some(4));
    assert!(
        String::from_utf8(rejected_evidence.stderr)
            .expect("legacy Evidence rejection is UTF-8")
            .contains("is not canonical")
    );

    let duplicate = belay()
        .args([
            "link",
            &work,
            &format!("{goal}#sc-001"),
            "--relation",
            "fulfills",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("link canonical duplicate fragment");
    assert!(duplicate.status.success(), "{duplicate:?}");
    let connection = Connection::open(temporary.path().join(".belay/state/belay.sqlite"))
        .expect("open database");
    let link_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM entry_links WHERE to_fragment = 'sc-001'",
            [],
            |row| row.get(0),
        )
        .expect("count canonical fragment links");
    assert_eq!(link_count, 1);

    let recorded = belay()
        .args([
            "verify",
            "record",
            "--kind",
            "test",
            "--verdict",
            "pass",
            "--source",
            "cargo test",
            "--summary",
            "canonical fragment",
            "--verifies",
            &criterion,
        ])
        .current_dir(temporary.path())
        .output()
        .expect("record evidence for uppercase fragment");
    assert!(recorded.status.success(), "{recorded:?}");
    let evidence_target: String = connection
        .query_row("SELECT target FROM evidence_links", [], |row| row.get(0))
        .expect("read evidence target");
    assert_eq!(evidence_target, format!("{goal}#sc-001"));
    let evidence_mirror = fs::read_dir(temporary.path().join(".belay/evidence"))
        .expect("read evidence directory")
        .next()
        .expect("evidence mirror")
        .expect("read evidence mirror entry")
        .path();
    let evidence_json = fs::read_to_string(evidence_mirror).expect("read evidence mirror");
    assert!(evidence_json.contains(&format!("\"target\":\"{goal}#sc-001\"")));
    let evidence_status = belay()
        .args(["verify", "status", &criterion])
        .current_dir(temporary.path())
        .output()
        .expect("show evidence for uppercase fragment");
    assert!(evidence_status.status.success(), "{evidence_status:?}");
    let evidence_status =
        String::from_utf8(evidence_status.stdout).expect("evidence status is UTF-8");
    assert!(evidence_status.starts_with(&format!("{goal}#sc-001\n")));
    assert!(evidence_status.contains("canonical fragment"));

    let missing = format!("{goal}#sc-999");
    let rejected_link = belay()
        .args(["link", &work, &missing, "--relation", "fulfills"])
        .current_dir(temporary.path())
        .output()
        .expect("reject missing link fragment");
    assert_eq!(rejected_link.status.code(), Some(4));
    assert!(
        String::from_utf8(rejected_link.stderr)
            .expect("link stderr")
            .contains("fragment #sc-999 was not found")
    );

    let evidence_files_before_rejection = fs::read_dir(temporary.path().join(".belay/evidence"))
        .expect("read evidence directory")
        .count();
    let rejected_evidence = belay()
        .args([
            "verify",
            "record",
            "--kind",
            "test",
            "--verdict",
            "pass",
            "--source",
            "cargo test",
            "--summary",
            "fragment validation",
            "--verifies",
            &missing,
        ])
        .current_dir(temporary.path())
        .output()
        .expect("reject missing evidence fragment");
    assert_eq!(rejected_evidence.status.code(), Some(4));
    assert_eq!(
        fs::read_dir(temporary.path().join(".belay/evidence"))
            .expect("read evidence directory")
            .count(),
        evidence_files_before_rejection
    );
}

#[test]
fn link_and_status_validation_do_not_mutate_entries() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args(["add", "decision", "--title", "Decision", "--body", "Body"])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );

    for output in [
        belay()
            .args(["link", &decision, &decision, "--relation", "references"])
            .current_dir(temporary.path())
            .output()
            .expect("self link"),
        belay()
            .args(["link", &decision, &decision, "--relation", "unknown"])
            .current_dir(temporary.path())
            .output()
            .expect("unknown relation"),
        belay()
            .args([
                "link",
                &decision,
                "NOTE-20260606T120000-001-missing",
                "--relation",
                "references",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("missing target"),
        belay()
            .args(["status", &decision, "in-progress"])
            .current_dir(temporary.path())
            .output()
            .expect("invalid status"),
    ] {
        assert_eq!(output.status.code(), Some(4), "{output:?}");
    }

    let connection =
        Connection::open(temporary.path().join(".belay/state/belay.sqlite")).expect("open DB");
    let revision: i64 = connection
        .query_row(
            "SELECT revision FROM entries WHERE display_id = ?1",
            [&decision],
            |row| row.get(0),
        )
        .expect("read revision");
    let link_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM entry_links", [], |row| row.get(0))
        .expect("count links");
    assert_eq!(revision, 1);
    assert_eq!(link_count, 0);
}

#[test]
fn status_accepts_an_allowed_value_for_every_entry_type() {
    let temporary = initialize_repository();
    for (entry_type, status) in [
        ("plan", "approved"),
        ("decision", "accepted"),
        ("work", "completed"),
        ("review", "completed"),
        ("note", "archived"),
    ] {
        let id = created_id(
            &belay()
                .args([
                    "add",
                    entry_type,
                    "--title",
                    &format!("{entry_type} status"),
                    "--body",
                    "Body",
                ])
                .current_dir(temporary.path())
                .output()
                .expect("add entry"),
        );
        let output = belay()
            .args(["status", &id, status])
            .current_dir(temporary.path())
            .output()
            .expect("update status");
        assert!(output.status.success(), "{output:?}");
        let shown = belay()
            .args(["show", &id])
            .current_dir(temporary.path())
            .output()
            .expect("show entry");
        assert!(
            String::from_utf8(shown.stdout)
                .expect("stdout")
                .contains(&format!("Status: {status}"))
        );
    }
}

#[test]
fn search_supports_exact_id_structured_filters_and_bm25_deduplication() {
    let temporary = initialize_repository();
    let primary = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "SQLite migration strategy",
                "--body",
                "## Storage\n\nsqlite sqlite sqlite migration\n\n## Rollout\n\nmigration validation",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add primary decision"),
    );
    let secondary = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "SQLite fallback",
                "--body",
                "sqlite fallback",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add secondary decision"),
    );
    let unrelated = created_id(
        &belay()
            .args([
                "add",
                "work",
                "--title",
                "Documentation",
                "--body",
                "write usage examples",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add unrelated work"),
    );

    let database_path = temporary.path().join(".belay/state/belay.sqlite");
    let connection = Connection::open(&database_path).expect("open DB");
    connection
        .execute(
            "
            INSERT INTO entry_tags(entry_id, tag)
            SELECT id, 'storage' FROM entries WHERE display_id = ?1
            ",
            [&primary],
        )
        .expect("tag primary");
    let query_plan = connection
        .prepare("EXPLAIN QUERY PLAN SELECT id FROM entries WHERE display_id = ?1")
        .expect("prepare query plan")
        .query_map([&primary], |row| row.get::<_, String>(3))
        .expect("query plan")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect query plan")
        .join("\n");
    assert!(query_plan.contains("idx_entries_display_id"));

    let exact = belay()
        .args(["search", &primary])
        .current_dir(temporary.path())
        .output()
        .expect("search exact ID");
    assert!(exact.status.success(), "{exact:?}");
    let exact_stdout = String::from_utf8(exact.stdout).expect("exact stdout");
    assert!(exact_stdout.contains("Matches: 1"));
    assert!(exact_stdout.contains("Why: exact display-ID match"));
    assert!(!exact_stdout.contains("BM25 score:"));

    let structured = belay()
        .args([
            "search", "--type", "decision", "--status", "proposed", "--tag", "storage",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("structured search");
    assert!(structured.status.success(), "{structured:?}");
    let structured_stdout = String::from_utf8(structured.stdout).expect("structured stdout");
    assert!(structured_stdout.contains("Matches: 1"));
    assert!(structured_stdout.contains(&primary));
    assert!(!structured_stdout.contains(&secondary));
    assert!(!structured_stdout.contains(&unrelated));

    let keyword = belay()
        .args(["search", "sqlite migration"])
        .current_dir(temporary.path())
        .output()
        .expect("keyword search");
    assert!(keyword.status.success(), "{keyword:?}");
    let keyword_stdout = String::from_utf8(keyword.stdout).expect("keyword stdout");
    assert!(keyword_stdout.contains("BM25 score:"));
    assert!(keyword_stdout.contains("Matching chunks: 2"));
    assert!(keyword_stdout.contains("Section: Storage"));
    assert!(
        keyword_stdout.find(&primary).expect("primary result")
            < keyword_stdout.find(&secondary).expect("secondary result")
    );
    assert!(!keyword_stdout.contains(&unrelated));
    assert!(!keyword_stdout.contains("entry_id"));
}

#[test]
fn search_supports_repeated_status_include_and_exclude() {
    let temporary = initialize_repository();
    let proposed = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Proposed storage",
                "--body",
                "keep proposed",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add proposed"),
    );
    let accepted = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Accepted storage",
                "--body",
                "keep accepted",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add accepted"),
    );
    let status = belay()
        .args(["status", &accepted, "accepted"])
        .current_dir(temporary.path())
        .output()
        .expect("accept decision");
    assert!(status.status.success(), "{status:?}");

    let help = belay()
        .args(["search", "--help"])
        .output()
        .expect("search help");
    let help_stdout = String::from_utf8(help.stdout).expect("help stdout");
    assert!(help_stdout.contains("--exclude-status"));
    assert!(help_stdout.contains("--status draft --status active"));

    let included = belay()
        .args(["search", "--type", "decision", "--status", "accepted"])
        .current_dir(temporary.path())
        .output()
        .expect("include accepted");
    assert!(included.status.success(), "{included:?}");
    let included_stdout = String::from_utf8(included.stdout).expect("include stdout");
    assert!(included_stdout.contains("Status include: accepted"));
    assert!(included_stdout.contains(&accepted));
    assert!(!included_stdout.contains(&proposed));

    let excluded = belay()
        .args([
            "search",
            "--type",
            "decision",
            "--exclude-status",
            "proposed",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("exclude proposed");
    assert!(excluded.status.success(), "{excluded:?}");
    let excluded_stdout = String::from_utf8(excluded.stdout).expect("exclude stdout");
    assert!(excluded_stdout.contains("Status exclude: proposed"));
    assert!(excluded_stdout.contains(&accepted));
    assert!(!excluded_stdout.contains(&proposed));

    let overlap = belay()
        .args([
            "search",
            "--status",
            "accepted",
            "--exclude-status",
            "accepted",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("overlap");
    assert_eq!(overlap.status.code(), Some(4));
}

#[test]
fn search_rejects_non_contiguous_migration_history() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "SQLite upgrade",
                "--body",
                "automatic migration",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let database_path = temporary.path().join(".belay/state/belay.sqlite");
    {
        let connection = Connection::open(&database_path).expect("open DB");
        connection
            .execute_batch(
                "
                DROP TABLE entry_fts;
                CREATE VIRTUAL TABLE entry_fts USING fts5(
                    entry_id UNINDEXED,
                    title,
                    body,
                    section,
                    chunk_text,
                    tokenize = 'unicode61'
                );
                INSERT INTO entry_fts(entry_id, title, body, section, chunk_text)
                SELECT chunks.entry_id, entries.title, entries.body,
                       chunks.section, chunks.text
                FROM entry_chunks chunks
                JOIN entries ON entries.id = chunks.entry_id;
                DELETE FROM schema_migrations WHERE version = 2;
                ",
            )
            .expect("downgrade to schema v1");
    }

    let output = belay()
        .args(["search", "automatic migration"])
        .current_dir(temporary.path())
        .output()
        .expect("search and migrate");
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8(output.stderr)
            .expect("search stderr")
            .contains(
        "migration history is inconsistent: version 2 is missing while version 4 is recorded"
            )
    );

    let connection = Connection::open(database_path).expect("reopen DB");
    let version: i64 = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("read schema version");
    let fts_sql: String = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'entry_fts'",
            [],
            |row| row.get(0),
        )
        .expect("read FTS schema");
    assert_eq!(version, 4);
    assert!(!fts_sql.contains("chunk_ordinal UNINDEXED"));
    assert!(!decision.is_empty());
}

#[test]
fn concurrent_searches_reject_non_contiguous_migration_history() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Concurrent SQLite upgrade",
                "--body",
                "serialized automatic migration",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let database_path = temporary.path().join(".belay/state/belay.sqlite");
    {
        let connection = Connection::open(&database_path).expect("open DB");
        connection
            .execute_batch(
                "
                DROP TABLE entry_fts;
                CREATE VIRTUAL TABLE entry_fts USING fts5(
                    entry_id UNINDEXED,
                    title,
                    body,
                    section,
                    chunk_text,
                    tokenize = 'unicode61'
                );
                INSERT INTO entry_fts(entry_id, title, body, section, chunk_text)
                SELECT chunks.entry_id, entries.title, entries.body,
                       chunks.section, chunks.text
                FROM entry_chunks chunks
                JOIN entries ON entries.id = chunks.entry_id;
                DELETE FROM schema_migrations WHERE version = 2;
                ",
            )
            .expect("downgrade to schema v1");
    }

    let mut children = Vec::new();
    for _ in 0..4 {
        children.push(
            belay()
                .args(["search", "serialized migration"])
                .current_dir(temporary.path())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn concurrent search"),
        );
    }
    for child in children {
        let output = child
            .wait_with_output()
            .expect("wait for concurrent search");
        assert!(!output.status.success(), "{output:?}");
        assert!(
            String::from_utf8(output.stderr)
                .expect("search stderr")
                .contains(
            "migration history is inconsistent: version 2 is missing while version 4 is recorded"
                )
        );
    }

    let connection = Connection::open(database_path).expect("reopen DB");
    let migration_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 2",
            [],
            |row| row.get(0),
        )
        .expect("count migration rows");
    assert_eq!(migration_count, 0);
    assert!(!decision.is_empty());
}

#[test]
fn context_is_linked_source_attributed_and_within_the_estimated_budget() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Use SQLite migration",
                "--body",
                "SQLite migration defines the durable storage approach. "
                    .repeat(20)
                    .as_str(),
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let work = created_id(
        &belay()
            .args([
                "add",
                "work",
                "--title",
                "Implement storage",
                "--body",
                "Follow the accepted decision and validate the implementation.",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add work"),
    );
    let note = created_id(
        &belay()
            .args([
                "add",
                "note",
                "--title",
                "Second-hop note",
                "--body",
                "This note is intentionally two links away from the ranked decision.",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add note"),
    );
    let linked = belay()
        .args(["link", &work, &decision, "--relation", "implements"])
        .current_dir(temporary.path())
        .output()
        .expect("link work to decision");
    assert!(linked.status.success(), "{linked:?}");
    let second_hop = belay()
        .args(["link", &note, &work, "--relation", "references"])
        .current_dir(temporary.path())
        .output()
        .expect("link second-hop note");
    assert!(second_hop.status.success(), "{second_hop:?}");

    for format in ["agent", "human"] {
        let output = belay()
            .args([
                "context",
                "sqlite migration",
                "--format",
                format,
                "--budget",
                "500",
            ])
            .current_dir(temporary.path())
            .output()
            .unwrap_or_else(|error| panic!("generate {format} context: {error}"));
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8(output.stdout).expect("context stdout");
        assert!(stdout.contains(&decision));
        assert!(stdout.contains(&work));
        assert!(stdout.contains(&format!("belay show {decision}")));
        assert!(stdout.contains("entries/"));
        assert!(stdout.contains("linked from") || stdout.contains("links to"));
        assert!(estimate_tokens(&stdout) <= 500);
        assert!(!stdout.contains(&note));
        assert!(!stdout.contains("entry_id"));
    }

    let default_budget = belay()
        .args(["context", "sqlite migration", "--format", "agent"])
        .current_dir(temporary.path())
        .output()
        .expect("generate context with default budget");
    assert!(default_budget.status.success(), "{default_budget:?}");
    assert!(
        String::from_utf8(default_budget.stdout)
            .expect("default context stdout")
            .contains("Budget: 2500 estimated tokens")
    );
}

#[test]
fn context_rejects_a_budget_too_small_for_attributed_output() {
    let temporary = initialize_repository();
    let output = belay()
        .args([
            "context",
            "sqlite migration",
            "--format",
            "agent",
            "--budget",
            "32",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("generate undersized context");
    assert_eq!(output.status.code(), Some(4));
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr")
            .contains("at least 64")
    );
}

#[test]
fn context_keeps_late_matching_evidence_under_a_small_budget() {
    let temporary = initialize_repository();
    let body = format!(
        "## Decision\n\n{}The durable choice is quartz-indexed storage.",
        "Background material without the target term. ".repeat(30)
    );
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Choose a storage strategy",
                "--body",
                &body,
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );

    let output = belay()
        .args([
            "context",
            "quartz-indexed",
            "--format",
            "agent",
            "--budget",
            "250",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("generate compact context");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("context stdout");
    assert!(stdout.contains(&decision));
    assert!(stdout.contains("The durable choice is quartz-indexed storage."));
    assert!(stdout.contains("Evidence:"));
    assert!(estimate_tokens(&stdout) <= 225);
}

#[test]
fn context_truncates_oversized_minimum_evidence_instead_of_dropping_the_entry() {
    let temporary = initialize_repository();
    let body = format!(
        "## Decision\n\nThe quartz-indexed choice remains required because {}",
        "its durability evidence is extensive ".repeat(80)
    );
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Oversized evidence",
                "--body",
                &body,
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );

    let output = belay()
        .args([
            "context",
            "quartz-indexed",
            "--format",
            "agent",
            "--budget",
            "250",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("generate compact context");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("context stdout");
    assert!(stdout.contains(&decision));
    assert!(stdout.contains("The quartz-indexed choice remains required"));
    assert!(stdout.contains("..."));
    assert!(estimate_tokens(&stdout) <= 225);
}

#[test]
fn context_prefers_text_matches_over_heading_only_matches() {
    let temporary = initialize_repository();
    let body = format!(
        "## Quartz\n\n{}The actual quartz requirement is retained.",
        "Generic background sentence. ".repeat(20)
    );
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Heading collision",
                "--body",
                &body,
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );

    let output = belay()
        .args(["context", "quartz", "--format", "agent", "--budget", "250"])
        .current_dir(temporary.path())
        .output()
        .expect("generate compact context");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("context stdout");
    assert!(stdout.contains(&decision));
    assert!(stdout.contains("The actual quartz requirement is retained."));
    assert!(estimate_tokens(&stdout) <= 225);
}

#[test]
fn context_preserves_late_match_when_truncating_one_long_sentence() {
    let temporary = initialize_repository();
    let body = format!(
        "## Decision\n\n{}the quartz-indexed requirement is retained because durability matters.",
        "generic background words ".repeat(60)
    );
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Late term in long sentence",
                "--body",
                &body,
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );

    let output = belay()
        .args([
            "context",
            "quartz-indexed",
            "--format",
            "agent",
            "--budget",
            "250",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("generate compact context");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("context stdout");
    assert!(stdout.contains(&decision));
    assert!(stdout.contains("quartz-indexed"));
    assert!(estimate_tokens(&stdout) <= 225);
}

#[test]
fn context_selects_breadth_before_expanding_oversized_top_evidence() {
    let temporary = initialize_repository();
    let decision_body = format!(
        "## Decision\n\nThe quartz-indexed choice is required because {}",
        "durability evidence remains extensive ".repeat(80)
    );
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Primary storage decision",
                "--body",
                &decision_body,
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let work = created_id(
        &belay()
            .args([
                "add",
                "work",
                "--title",
                "Implement storage decision",
                "--body",
                "## Changes\n\nApply the selected storage approach and validate it.",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add work"),
    );
    let linked = belay()
        .args(["link", &work, &decision, "--relation", "implements"])
        .current_dir(temporary.path())
        .output()
        .expect("link work");
    assert!(linked.status.success(), "{linked:?}");

    let output = belay()
        .args([
            "context",
            "quartz-indexed",
            "--format",
            "agent",
            "--budget",
            "500",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("generate broad context");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("context stdout");
    assert!(stdout.contains(&decision));
    assert!(stdout.contains(&work));
    assert!(estimate_tokens(&stdout) <= 450);
}

#[test]
fn failed_mirror_replacement_rolls_back_sqlite_mutation() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args(["add", "decision", "--title", "Decision", "--body", "Body"])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let mirror = temporary
        .path()
        .join(".belay/entries/decisions")
        .join(format!("{decision}.md"));
    fs::remove_file(&mirror).expect("remove mirror");

    let output = belay()
        .args(["status", &decision, "accepted"])
        .current_dir(temporary.path())
        .output()
        .expect("update status without mirror");
    assert_eq!(output.status.code(), Some(6));

    let connection =
        Connection::open(temporary.path().join(".belay/state/belay.sqlite")).expect("open DB");
    let state: (String, i64) = connection
        .query_row(
            "SELECT status, revision FROM entries WHERE display_id = ?1",
            [&decision],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read entry state");
    assert_eq!(state, ("proposed".to_owned(), 1));
    let baseline_count: i64 = connection
        .query_row(
            "
            SELECT COUNT(*)
            FROM sync_state state
            JOIN entries entry ON entry.id = state.entry_id
            WHERE entry.display_id = ?1
              AND state.sqlite_content_hash_at_last_sync =
                  state.mirror_content_hash_at_last_sync
            ",
            [&decision],
            |row| row.get(0),
        )
        .expect("read sync baseline");
    assert_eq!(baseline_count, 1);
}

#[test]
fn unsynchronized_markdown_is_not_overwritten_by_status_or_link() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args(["add", "decision", "--title", "Decision", "--body", "Body"])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let work = created_id(
        &belay()
            .args(["add", "work", "--title", "Work", "--body", "Work body"])
            .current_dir(temporary.path())
            .output()
            .expect("add work"),
    );
    let initial_link = belay()
        .args(["link", &decision, &work, "--relation", "references"])
        .current_dir(temporary.path())
        .output()
        .expect("create initial link");
    assert!(initial_link.status.success(), "{initial_link:?}");
    let mirror = temporary
        .path()
        .join(".belay/entries/decisions")
        .join(format!("{decision}.md"));
    let edited = fs::read_to_string(&mirror)
        .expect("read mirror")
        .replace("Body\n", "Direct Markdown edit\n");
    fs::write(&mirror, &edited).expect("edit mirror");

    let no_op_status = belay()
        .args(["status", &decision, "proposed"])
        .current_dir(temporary.path())
        .output()
        .expect("repeat current status");
    assert_eq!(no_op_status.status.code(), Some(5));
    let changed_status = belay()
        .args(["status", &decision, "accepted"])
        .current_dir(temporary.path())
        .output()
        .expect("update status");
    assert_eq!(changed_status.status.code(), Some(5));
    let duplicate_link = belay()
        .args(["link", &decision, &work, "--relation", "references"])
        .current_dir(temporary.path())
        .output()
        .expect("repeat link");
    assert_eq!(duplicate_link.status.code(), Some(5));
    assert_eq!(
        fs::read_to_string(&mirror).expect("read edited mirror"),
        edited
    );

    let connection =
        Connection::open(temporary.path().join(".belay/state/belay.sqlite")).expect("open DB");
    let state: (String, i64) = connection
        .query_row(
            "SELECT status, revision FROM entries WHERE display_id = ?1",
            [&decision],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read entry state");
    let link_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM entry_links", [], |row| row.get(0))
        .expect("count links");
    assert_eq!(state, ("proposed".to_owned(), 2));
    assert_eq!(link_count, 1);
}

#[cfg(unix)]
#[test]
fn add_rejects_a_symlinked_managed_parent_without_external_write() {
    use std::os::unix::fs::symlink;

    let temporary = initialize_repository();
    let external = tempdir().expect("create external directory");
    let decisions = temporary.path().join(".belay/entries/decisions");
    fs::remove_dir(&decisions).expect("remove decisions directory");
    symlink(external.path(), &decisions).expect("symlink decisions directory");

    let output = belay()
        .args(["add", "decision", "--title", "Escaped", "--body", "Body"])
        .current_dir(temporary.path())
        .output()
        .expect("add decision");
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(
        fs::read_dir(external.path())
            .expect("read external directory")
            .count(),
        0
    );
    let connection =
        Connection::open(temporary.path().join(".belay/state/belay.sqlite")).expect("open DB");
    let entry_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
        .expect("count entries");
    assert_eq!(entry_count, 0);
}

#[test]
fn sync_imports_markdown_only_changes_and_normalizes_managed_fields() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Original title",
                "--body",
                "Original body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let mirror = mirror_path(temporary.path(), "decisions", &decision);
    let original =
        markdown::parse(&fs::read_to_string(&mirror).expect("read mirror")).expect("parse mirror");
    let edited = fs::read_to_string(&mirror)
        .expect("read mirror")
        .replace("title: Original title", "title: Markdown title")
        .replace("revision: 1", "revision: 99")
        .replace("Original body", "Markdown body");
    fs::write(&mirror, edited).expect("edit mirror");

    let output = belay()
        .args(["sync", &decision])
        .current_dir(temporary.path())
        .output()
        .expect("sync Markdown edit");
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8(output.stdout)
            .expect("stdout is UTF-8")
            .contains("imported Markdown")
    );

    let shown = belay()
        .args(["show", &decision])
        .current_dir(temporary.path())
        .output()
        .expect("show imported entry");
    let stdout = String::from_utf8(shown.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("Title: Markdown title"));
    assert!(stdout.contains("Markdown body"));
    assert!(stdout.contains("Revision: 2"));
    let normalized = markdown::parse(&fs::read_to_string(&mirror).expect("read normalized mirror"))
        .expect("parse normalized mirror");
    assert_eq!(normalized.revision, 2);
    assert_eq!(normalized.created_at, original.created_at);
}

#[test]
fn sync_renders_sqlite_only_changes_and_resolves_both_preferences() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Original",
                "--body",
                "Original body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let database = temporary.path().join(".belay/state/belay.sqlite");
    let mirror = mirror_path(temporary.path(), "decisions", &decision);
    let connection = Connection::open(&database).expect("open database");
    connection
        .execute(
            "UPDATE entries SET title = 'SQLite title' WHERE display_id = ?1",
            [&decision],
        )
        .expect("edit SQLite");
    drop(connection);

    let sqlite_only = belay()
        .args(["sync", &decision])
        .current_dir(temporary.path())
        .output()
        .expect("sync SQLite edit");
    assert!(sqlite_only.status.success(), "{sqlite_only:?}");
    assert!(
        fs::read_to_string(&mirror)
            .expect("read rendered mirror")
            .contains("title: SQLite title")
    );

    let markdown_conflict = fs::read_to_string(&mirror)
        .expect("read mirror")
        .replace("Original body", "Markdown conflict body");
    fs::write(&mirror, markdown_conflict).expect("edit Markdown");
    let connection = Connection::open(&database).expect("open database");
    connection
        .execute(
            "UPDATE entries SET title = 'SQLite conflict title' WHERE display_id = ?1",
            [&decision],
        )
        .expect("edit SQLite again");
    drop(connection);

    let conflict = belay()
        .args(["sync", &decision])
        .current_dir(temporary.path())
        .output()
        .expect("detect conflict");
    assert_eq!(conflict.status.code(), Some(5));

    let keep_markdown = belay()
        .args(["sync", "--prefer", "markdown", &decision])
        .current_dir(temporary.path())
        .output()
        .expect("prefer Markdown");
    assert!(keep_markdown.status.success(), "{keep_markdown:?}");
    let shown = belay()
        .args(["show", &decision])
        .current_dir(temporary.path())
        .output()
        .expect("show Markdown preference");
    let stdout = String::from_utf8(shown.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("Title: SQLite title"));
    assert!(stdout.contains("Markdown conflict body"));

    let edited = fs::read_to_string(&mirror)
        .expect("read mirror")
        .replace("Markdown conflict body", "Second Markdown conflict");
    fs::write(&mirror, edited).expect("edit Markdown again");
    let connection = Connection::open(&database).expect("open database");
    connection
        .execute(
            "UPDATE entries SET title = 'Final SQLite title' WHERE display_id = ?1",
            [&decision],
        )
        .expect("edit SQLite final");
    drop(connection);
    let keep_sqlite = belay()
        .args(["sync", "--prefer", "sqlite", &decision])
        .current_dir(temporary.path())
        .output()
        .expect("prefer SQLite");
    assert!(keep_sqlite.status.success(), "{keep_sqlite:?}");
    let rendered = fs::read_to_string(&mirror).expect("read SQLite preference mirror");
    assert!(rendered.contains("title: Final SQLite title"));
    assert!(rendered.contains("Markdown conflict body"));
    assert!(!rendered.contains("Second Markdown conflict"));
}

#[test]
fn sync_imports_new_markdown_and_restores_missing_counterparts() {
    let temporary = initialize_repository();
    let entry = Entry {
        display_id: "NOTE-20260607T030000-001-direct-note".to_owned(),
        entry_type: EntryType::Note,
        title: "Direct note".to_owned(),
        status: EntryStatus::Active,
        created_at: "2026-06-07T03:00:00+09:00".to_owned(),
        updated_at: "2026-06-07T03:00:00+09:00".to_owned(),
        revision: 1,
        tags: vec!["direct".to_owned()],
        links: Vec::new(),
        metadata: BTreeMap::new(),
        body: "Directly authored Markdown".to_owned(),
    };
    let mirror = mirror_path(temporary.path(), "notes", &entry.display_id);
    fs::write(
        &mirror,
        markdown::render(&entry).expect("render direct entry"),
    )
    .expect("write direct entry");

    let imported = belay()
        .arg("sync")
        .current_dir(temporary.path())
        .output()
        .expect("import direct Markdown");
    assert!(imported.status.success(), "{imported:?}");
    let database = temporary.path().join(".belay/state/belay.sqlite");
    let connection = Connection::open(&database).expect("open database");
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM entries WHERE display_id = ?1",
            [&entry.display_id],
            |row| row.get(0),
        )
        .expect("count imported entry");
    assert_eq!(count, 1);
    drop(connection);

    fs::remove_file(&mirror).expect("remove mirror");
    let restored_mirror = belay()
        .args(["sync", &entry.display_id])
        .current_dir(temporary.path())
        .output()
        .expect("restore mirror");
    assert!(restored_mirror.status.success(), "{restored_mirror:?}");
    assert!(mirror.exists());

    let connection = Connection::open(&database).expect("open database");
    connection
        .execute("PRAGMA foreign_keys = OFF", [])
        .expect("disable foreign keys");
    connection
        .execute(
            "DELETE FROM entries WHERE display_id = ?1",
            [&entry.display_id],
        )
        .expect("remove SQLite row");
    drop(connection);
    let restored_sqlite = belay()
        .args(["sync", &entry.display_id])
        .current_dir(temporary.path())
        .output()
        .expect("restore SQLite row");
    assert!(restored_sqlite.status.success(), "{restored_sqlite:?}");
    let connection = Connection::open(&database).expect("open database");
    let restored: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM entries WHERE display_id = ?1",
            [&entry.display_id],
            |row| row.get(0),
        )
        .expect("count restored SQLite entry");
    assert_eq!(restored, 1);
}

#[test]
fn sync_restores_deleted_sqlite_targets_without_propagating_link_deletion() {
    let temporary = initialize_repository();
    let source = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Link source",
                "--body",
                "Source body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add source"),
    );
    let target = created_id(
        &belay()
            .args([
                "add",
                "work",
                "--title",
                "Link target",
                "--body",
                "Target body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add target"),
    );
    let linked = belay()
        .args(["link", &source, &target, "--relation", "references"])
        .current_dir(temporary.path())
        .output()
        .expect("link entries");
    assert!(linked.status.success(), "{linked:?}");
    let source_mirror = mirror_path(temporary.path(), "decisions", &source);
    let expected_mirror = fs::read_to_string(&source_mirror).expect("read linked mirror");

    let database = temporary.path().join(".belay/state/belay.sqlite");
    let connection = Connection::open(&database).expect("open database");
    connection
        .execute("DELETE FROM entries WHERE display_id = ?1", [&target])
        .expect("delete target row");
    drop(connection);

    let output = belay()
        .args(["sync", &source])
        .current_dir(temporary.path())
        .output()
        .expect("restore target and relationships from targeted source sync");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read_to_string(&source_mirror).expect("read preserved source mirror"),
        expected_mirror
    );
    let connection = Connection::open(&database).expect("open restored database");
    let link_count: i64 = connection
        .query_row(
            "
            SELECT COUNT(*)
            FROM entry_links links
            JOIN entries source ON source.id = links.from_entry_id
            JOIN entries target ON target.id = links.to_entry_id
            WHERE source.display_id = ?1 AND target.display_id = ?2
            ",
            [&source, &target],
            |row| row.get(0),
        )
        .expect("count restored link");
    assert_eq!(link_count, 1);
}

#[test]
fn sync_does_not_repair_links_from_markdown_before_conflict_classification() {
    let temporary = initialize_repository();
    let source = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Conflict source",
                "--body",
                "Source body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add source"),
    );
    let target = created_id(
        &belay()
            .args([
                "add",
                "work",
                "--title",
                "Conflict target",
                "--body",
                "Target body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add target"),
    );
    let linked = belay()
        .args(["link", &source, &target, "--relation", "references"])
        .current_dir(temporary.path())
        .output()
        .expect("link entries");
    assert!(linked.status.success(), "{linked:?}");
    let source_mirror = mirror_path(temporary.path(), "decisions", &source);
    let edited_mirror = fs::read_to_string(&source_mirror)
        .expect("read linked mirror")
        .replace("relation: references", "relation: supersedes");
    fs::write(&source_mirror, &edited_mirror).expect("edit Markdown relationship");

    let database = temporary.path().join(".belay/state/belay.sqlite");
    let connection = Connection::open(&database).expect("open database");
    connection
        .execute(
            "UPDATE entries SET title = 'SQLite conflict title' WHERE display_id = ?1",
            [&source],
        )
        .expect("edit SQLite source");
    connection
        .execute("DELETE FROM entries WHERE display_id = ?1", [&target])
        .expect("delete target row");
    drop(connection);

    let output = belay()
        .args(["sync", &source])
        .current_dir(temporary.path())
        .output()
        .expect("detect conflict after target restoration");
    assert_eq!(output.status.code(), Some(5));
    assert_eq!(
        fs::read_to_string(&source_mirror).expect("read preserved source mirror"),
        edited_mirror
    );
    let connection = Connection::open(&database).expect("open restored database");
    let supersedes_count: i64 = connection
        .query_row(
            "
            SELECT COUNT(*)
            FROM entry_links links
            JOIN entries source ON source.id = links.from_entry_id
            WHERE source.display_id = ?1 AND links.relation = 'supersedes'
            ",
            [&source],
            |row| row.get(0),
        )
        .expect("count prematurely repaired links");
    let sqlite_title: String = connection
        .query_row(
            "SELECT title FROM entries WHERE display_id = ?1",
            [&source],
            |row| row.get(0),
        )
        .expect("read preserved SQLite title");
    assert_eq!(supersedes_count, 0);
    assert_eq!(sqlite_title, "SQLite conflict title");
}

#[test]
fn sync_imports_mutually_linked_new_markdown_entries_without_order_dependency() {
    let temporary = initialize_repository();
    let first_id = "NOTE-20260607T031000-001-first".to_owned();
    let second_id = "NOTE-20260607T031000-002-second".to_owned();
    let first = Entry {
        display_id: first_id.clone(),
        entry_type: EntryType::Note,
        title: "First note".to_owned(),
        status: EntryStatus::Active,
        created_at: "2026-06-07T03:10:00+09:00".to_owned(),
        updated_at: "2026-06-07T03:10:00+09:00".to_owned(),
        revision: 1,
        tags: Vec::new(),
        links: vec![EntryLink {
            relation: LinkRelation::References,
            id: second_id.clone(),
            metadata: BTreeMap::new(),
        }],
        metadata: BTreeMap::new(),
        body: "First body".to_owned(),
    };
    let second = Entry {
        display_id: second_id.clone(),
        entry_type: EntryType::Note,
        title: "Second note".to_owned(),
        status: EntryStatus::Active,
        created_at: "2026-06-07T03:10:00+09:00".to_owned(),
        updated_at: "2026-06-07T03:10:00+09:00".to_owned(),
        revision: 1,
        tags: Vec::new(),
        links: vec![EntryLink {
            relation: LinkRelation::References,
            id: first_id.clone(),
            metadata: BTreeMap::new(),
        }],
        metadata: BTreeMap::new(),
        body: "Second body".to_owned(),
    };
    for entry in [&first, &second] {
        fs::write(
            mirror_path(temporary.path(), "notes", &entry.display_id),
            markdown::render(entry).expect("render new linked entry"),
        )
        .expect("write new linked entry");
    }

    let output = belay()
        .args(["sync", &first_id])
        .current_dir(temporary.path())
        .output()
        .expect("import linked dependency closure");
    assert!(output.status.success(), "{output:?}");
    let connection =
        Connection::open(temporary.path().join(".belay/state/belay.sqlite")).expect("open DB");
    let entry_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM entries WHERE display_id IN (?1, ?2)",
            [&first_id, &second_id],
            |row| row.get(0),
        )
        .expect("count imported entries");
    let link_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM entry_links", [], |row| row.get(0))
        .expect("count imported links");
    assert_eq!(entry_count, 2);
    assert_eq!(link_count, 2);
}

#[test]
fn sync_accepts_nested_path_only_rename_without_revision_churn() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Rename path",
                "--body",
                "Body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let original = mirror_path(temporary.path(), "decisions", &decision);
    let nested_dir = temporary.path().join(".belay/entries/decisions/archive");
    fs::create_dir(&nested_dir).expect("create nested directory");
    let renamed = nested_dir.join(format!("{decision}.md"));
    fs::rename(&original, &renamed).expect("rename mirror");

    let output = belay()
        .args(["sync", &decision])
        .current_dir(temporary.path())
        .output()
        .expect("sync path rename");
    assert!(output.status.success(), "{output:?}");
    let shown = belay()
        .args(["show", &decision])
        .current_dir(temporary.path())
        .output()
        .expect("show renamed entry");
    let stdout = String::from_utf8(shown.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains(&format!("Source: entries/decisions/archive/{decision}.md")));
    assert!(stdout.contains("Revision: 1"));
}

#[test]
fn rebuild_restores_entries_links_search_and_sync_baselines() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Rebuild decision",
                "--body",
                "Searchable rebuild body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let work = created_id(
        &belay()
            .args([
                "add",
                "work",
                "--title",
                "Rebuild work",
                "--body",
                "Implement rebuild",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add work"),
    );
    let linked = belay()
        .args(["link", &work, &decision, "--relation", "implements"])
        .current_dir(temporary.path())
        .output()
        .expect("link entries");
    assert!(linked.status.success(), "{linked:?}");
    let database = temporary.path().join(".belay/state/belay.sqlite");
    fs::remove_file(&database).expect("remove database");

    let rebuilt = belay()
        .arg("rebuild")
        .current_dir(temporary.path())
        .output()
        .expect("rebuild database");
    assert!(rebuilt.status.success(), "{rebuilt:?}");
    let shown = belay()
        .args(["show", &work])
        .current_dir(temporary.path())
        .output()
        .expect("show rebuilt work");
    assert!(
        String::from_utf8(shown.stdout)
            .expect("stdout is UTF-8")
            .contains(&format!("implements {decision}"))
    );
    let search = belay()
        .args(["search", "Searchable rebuild"])
        .current_dir(temporary.path())
        .output()
        .expect("search rebuilt database");
    assert!(
        String::from_utf8(search.stdout)
            .expect("stdout is UTF-8")
            .contains(&decision)
    );
    let connection = Connection::open(&database).expect("open rebuilt database");
    let counts: (i64, i64, i64) = connection
        .query_row(
            "
            SELECT
              (SELECT COUNT(*) FROM entries),
              (SELECT COUNT(*) FROM entry_links),
              (SELECT COUNT(*) FROM sync_state)
            ",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read rebuilt counts");
    assert_eq!(counts, (2, 1, 2));
}

#[test]
fn rebuild_upgrades_a_pre_route_schema_without_receipts() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Legacy rebuild",
                "--body",
                "A schema v3 database remains rebuildable.",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let database = temporary.path().join(".belay/state/belay.sqlite");
    let connection = Connection::open(&database).expect("open database");
    connection
        .execute_batch(
            "
            DROP INDEX idx_route_operation_receipts_preview;
            DROP TABLE route_operation_receipts;
            DELETE FROM schema_migrations WHERE version = 4;
            ",
        )
        .expect("downgrade to schema v3");
    drop(connection);

    let rebuilt = belay()
        .arg("rebuild")
        .current_dir(temporary.path())
        .output()
        .expect("rebuild legacy database");
    assert!(rebuilt.status.success(), "{rebuilt:?}");
    let rebuilt_database = Connection::open(&database).expect("open rebuilt database");
    let version: i64 = rebuilt_database
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("read rebuilt schema version");
    assert_eq!(version, 4);
    let restored: String = rebuilt_database
        .query_row(
            "SELECT title FROM entries WHERE display_id = ?1",
            [&decision],
            |row| row.get(0),
        )
        .expect("read rebuilt entry");
    assert_eq!(restored, "Legacy rebuild");
}

#[test]
fn rebuild_validation_failure_preserves_existing_database() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Keep database",
                "--body",
                "Body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let mirror = mirror_path(temporary.path(), "decisions", &decision);
    fs::write(&mirror, "invalid Markdown\n").expect("invalidate mirror");

    let rebuilt = belay()
        .arg("rebuild")
        .current_dir(temporary.path())
        .output()
        .expect("reject invalid rebuild");
    assert_eq!(rebuilt.status.code(), Some(4));
    let connection =
        Connection::open(temporary.path().join(".belay/state/belay.sqlite")).expect("open DB");
    let title: String = connection
        .query_row(
            "SELECT title FROM entries WHERE display_id = ?1",
            [&decision],
            |row| row.get(0),
        )
        .expect("read preserved database");
    assert_eq!(title, "Keep database");
}

#[test]
fn doctor_reports_sync_drift_temporary_files_and_invalid_mirrors() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args(["add", "decision", "--title", "Doctor", "--body", "Body"])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let mirror = mirror_path(temporary.path(), "decisions", &decision);
    let edited = fs::read_to_string(&mirror)
        .expect("read mirror")
        .replace("Body", "Drifted body");
    fs::write(&mirror, edited).expect("edit mirror");
    let temporary_file = temporary
        .path()
        .join(".belay/entries/decisions/.orphan.tmp-1");
    fs::write(&temporary_file, "temporary").expect("write orphan temp");

    let drift = belay()
        .arg("doctor")
        .current_dir(temporary.path())
        .output()
        .expect("doctor drift");
    assert_eq!(drift.status.code(), Some(5));
    let stdout = String::from_utf8(drift.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("SQLite/Markdown drift: drift"));
    assert!(stdout.contains("orphaned temporary files: drift"));

    fs::remove_file(&temporary_file).expect("remove temp");
    let wrong = temporary
        .path()
        .join(".belay/entries/decisions/wrong-name.md");
    fs::rename(&mirror, &wrong).expect("create invalid path");
    let invalid = belay()
        .arg("doctor")
        .current_dir(temporary.path())
        .output()
        .expect("doctor invalid mirror");
    assert_eq!(invalid.status.code(), Some(4));
    assert!(
        String::from_utf8(invalid.stdout)
            .expect("stdout is UTF-8")
            .contains("managed Markdown: invalid")
    );
}

#[test]
fn sync_suppresses_unchanged_details_and_summarizes_them() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Quiet sync",
                "--body",
                "No changes",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );

    let unchanged = belay()
        .arg("sync")
        .current_dir(temporary.path())
        .output()
        .expect("sync unchanged entry");
    assert!(unchanged.status.success(), "{unchanged:?}");
    let stdout = String::from_utf8(unchanged.stdout).expect("stdout is UTF-8");
    assert!(!stdout.contains(&format!("{decision}: unchanged")));
    assert!(stdout.contains("Sync completed: 1 entries (1 unchanged)"));

    let mirror = mirror_path(temporary.path(), "decisions", &decision);
    let edited = fs::read_to_string(&mirror)
        .expect("read decision mirror")
        .replace("No changes", "Markdown changed");
    fs::write(&mirror, edited).expect("edit decision mirror");
    let changed = belay()
        .args(["sync", &decision])
        .current_dir(temporary.path())
        .output()
        .expect("sync changed entry");
    assert!(changed.status.success(), "{changed:?}");
    let stdout = String::from_utf8(changed.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains(&decision));
    assert!(stdout.contains("imported Markdown"));
    assert!(stdout.contains("Sync completed: 1 entries (0 unchanged)"));
}

#[test]
fn doctor_rejects_missing_operational_fts_table() {
    let temporary = initialize_repository();
    let database = temporary.path().join(".belay/state/belay.sqlite");
    let connection = Connection::open(&database).expect("open database");
    connection
        .execute_batch("DROP TABLE entry_fts;")
        .expect("drop operational FTS table");
    drop(connection);

    let output = belay()
        .arg("doctor")
        .current_dir(temporary.path())
        .output()
        .expect("diagnose missing FTS table");
    assert_eq!(output.status.code(), Some(4));
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("SQLite schema: invalid"));
    assert!(stdout.contains("entry_fts"));
}

#[test]
fn sync_rejects_duplicate_and_ambiguous_unbaselined_entries_without_overwrite() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Duplicate guard",
                "--body",
                "Original body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let mirror = mirror_path(temporary.path(), "decisions", &decision);
    let nested = temporary.path().join(".belay/entries/decisions/copy");
    fs::create_dir(&nested).expect("create duplicate directory");
    fs::copy(&mirror, nested.join(format!("{decision}.md"))).expect("copy duplicate mirror");
    let duplicate = belay()
        .arg("sync")
        .current_dir(temporary.path())
        .output()
        .expect("reject duplicate");
    assert_eq!(duplicate.status.code(), Some(4));
    fs::remove_dir_all(&nested).expect("remove duplicate");

    let database = temporary.path().join(".belay/state/belay.sqlite");
    let connection = Connection::open(&database).expect("open database");
    connection
        .execute(
            "DELETE FROM sync_state WHERE entry_id = (SELECT id FROM entries WHERE display_id = ?1)",
            [&decision],
        )
        .expect("remove baseline");
    drop(connection);
    let edited = fs::read_to_string(&mirror)
        .expect("read mirror")
        .replace("Original body", "Independent Markdown body");
    fs::write(&mirror, &edited).expect("edit unbaselined mirror");
    let ambiguous = belay()
        .args(["sync", &decision])
        .current_dir(temporary.path())
        .output()
        .expect("reject ambiguity");
    assert_eq!(ambiguous.status.code(), Some(5));
    assert_eq!(
        fs::read_to_string(&mirror).expect("read preserved mirror"),
        edited
    );
    let connection = Connection::open(&database).expect("open database");
    let body: String = connection
        .query_row(
            "SELECT body FROM entries WHERE display_id = ?1",
            [&decision],
            |row| row.get(0),
        )
        .expect("read preserved SQLite body");
    assert_eq!(body, "Original body");
}

#[test]
fn batch_sync_reports_conflict_after_completing_independent_entries() {
    let temporary = initialize_repository();
    let conflict_id = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Conflict entry",
                "--body",
                "Conflict body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add conflict entry"),
    );
    let independent_id = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Independent entry",
                "--body",
                "Independent body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add independent entry"),
    );
    let conflict_mirror = mirror_path(temporary.path(), "decisions", &conflict_id);
    let edited = fs::read_to_string(&conflict_mirror)
        .expect("read conflict mirror")
        .replace("Conflict body", "Markdown conflict");
    fs::write(&conflict_mirror, &edited).expect("edit conflict mirror");
    let database = temporary.path().join(".belay/state/belay.sqlite");
    let connection = Connection::open(&database).expect("open database");
    connection
        .execute(
            "UPDATE entries SET title = 'SQLite conflict' WHERE display_id = ?1",
            [&conflict_id],
        )
        .expect("edit conflict SQLite");
    connection
        .execute(
            "UPDATE entries SET title = 'Independent SQLite update' WHERE display_id = ?1",
            [&independent_id],
        )
        .expect("edit independent SQLite");
    drop(connection);

    let output = belay()
        .arg("sync")
        .current_dir(temporary.path())
        .output()
        .expect("run batch sync");
    assert_eq!(output.status.code(), Some(5));
    let independent_mirror = mirror_path(temporary.path(), "decisions", &independent_id);
    assert!(
        fs::read_to_string(independent_mirror)
            .expect("read completed independent mirror")
            .contains("title: Independent SQLite update")
    );
    assert_eq!(
        fs::read_to_string(conflict_mirror).expect("read preserved conflict mirror"),
        edited
    );
}

#[cfg(unix)]
#[test]
fn batch_sync_reports_completed_entries_before_a_storage_failure() {
    use std::os::unix::fs::PermissionsExt;

    let temporary = initialize_repository();
    let first = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "First storage entry",
                "--body",
                "First body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add first entry"),
    );
    let second = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Second storage entry",
                "--body",
                "Second body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add second entry"),
    );
    let second_original = mirror_path(temporary.path(), "decisions", &second);
    let protected_directory = temporary.path().join(".belay/entries/decisions/protected");
    fs::create_dir(&protected_directory).expect("create protected directory");
    let second_nested = protected_directory.join(format!("{second}.md"));
    fs::rename(&second_original, &second_nested).expect("move second mirror");
    let renamed = belay()
        .args(["sync", &second])
        .current_dir(temporary.path())
        .output()
        .expect("accept second path rename");
    assert!(renamed.status.success(), "{renamed:?}");

    let database = temporary.path().join(".belay/state/belay.sqlite");
    let connection = Connection::open(&database).expect("open database");
    connection
        .execute(
            "UPDATE entries SET title = 'First completed update' WHERE display_id = ?1",
            [&first],
        )
        .expect("update first entry");
    connection
        .execute(
            "UPDATE entries SET title = 'Second failed update' WHERE display_id = ?1",
            [&second],
        )
        .expect("update second entry");
    drop(connection);

    fs::set_permissions(&protected_directory, fs::Permissions::from_mode(0o500))
        .expect("protect second mirror directory");
    let output = belay()
        .arg("sync")
        .current_dir(temporary.path())
        .output()
        .expect("run batch with storage failure");
    fs::set_permissions(&protected_directory, fs::Permissions::from_mode(0o700))
        .expect("restore directory permissions");

    assert_eq!(output.status.code(), Some(6));
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stdout.contains(&first));
    assert!(stdout.contains("rendered SQLite"));
    assert!(stderr.contains(&second));
    assert!(
        fs::read_to_string(mirror_path(temporary.path(), "decisions", &first))
            .expect("read completed first mirror")
            .contains("title: First completed update")
    );
    assert!(
        fs::read_to_string(&second_nested)
            .expect("read preserved second mirror")
            .contains("title: Second storage entry")
    );
}

#[test]
fn export_writes_parseable_filtered_snapshots_without_internal_ids() {
    let temporary = initialize_repository();
    let decision = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Exported decision",
                "--body",
                "Decision body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let work = created_id(
        &belay()
            .args([
                "add",
                "work",
                "--title",
                "Excluded work",
                "--body",
                "Work body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add work"),
    );
    let status = belay()
        .args(["status", &decision, "accepted"])
        .current_dir(temporary.path())
        .output()
        .expect("accept decision");
    assert!(status.status.success(), "{status:?}");
    let database = temporary.path().join(".belay/state/belay.sqlite");
    let connection = Connection::open(&database).expect("open database");
    connection
        .execute(
            "
            INSERT INTO entry_tags(entry_id, tag)
            SELECT id, 'release' FROM entries WHERE display_id = ?1
            ",
            [&decision],
        )
        .expect("add export filter tag");
    let before: (i64, i64, String) = connection
        .query_row(
            "
            SELECT
                (SELECT COUNT(*) FROM entries),
                (SELECT COUNT(*) FROM sync_state),
                (SELECT GROUP_CONCAT(display_id || ':' || revision || ':' || updated_at, '|')
                 FROM (SELECT display_id, revision, updated_at FROM entries ORDER BY display_id))
            ",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("snapshot state before export");
    drop(connection);
    let decision_mirror = mirror_path(temporary.path(), "decisions", &decision);
    let work_mirror = mirror_path(temporary.path(), "work", &work);
    let mirrors_before = (
        fs::read_to_string(&decision_mirror).expect("read decision mirror"),
        fs::read_to_string(&work_mirror).expect("read work mirror"),
    );

    let json_path = temporary.path().join("artifacts/accepted.json");
    let json_output = belay()
        .args([
            "export",
            "json",
            "--type",
            "decision",
            "--status",
            "accepted",
            "--tag",
            "release",
            "--output",
            json_path.to_str().expect("UTF-8 path"),
        ])
        .current_dir(temporary.path())
        .output()
        .expect("export JSON");
    assert!(json_output.status.success(), "{json_output:?}");
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(&json_path).expect("read JSON export"))
            .expect("parse JSON export");
    let entries = json["entries"].as_array().expect("entries array");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["id"], decision);
    assert_eq!(entries[0]["type"], "decision");
    assert!(entries[0].get("internal_id").is_none());
    assert!(
        !fs::read_to_string(&json_path)
            .expect("read JSON text")
            .contains("\"entry_id\"")
    );

    let ndjson_path = temporary.path().join("artifacts/all.ndjson");
    let ndjson_output = belay()
        .args([
            "export",
            "ndjson",
            "--output",
            ndjson_path.to_str().expect("UTF-8 path"),
        ])
        .current_dir(temporary.path())
        .output()
        .expect("export NDJSON");
    assert!(ndjson_output.status.success(), "{ndjson_output:?}");
    let ndjson = fs::read_to_string(&ndjson_path).expect("read NDJSON export");
    let ndjson_entries = ndjson
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("parse NDJSON line"))
        .collect::<Vec<_>>();
    assert_eq!(ndjson_entries.len(), 2);
    assert!(ndjson_entries.iter().all(|entry| entry["id"].is_string()));
    assert!(
        ndjson_entries
            .iter()
            .all(|entry| entry.get("internal_id").is_none())
    );

    let markdown_path = temporary.path().join("artifacts/decision.md");
    let markdown_output = belay()
        .args([
            "export",
            "markdown",
            "--id",
            &decision,
            "--output",
            markdown_path.to_str().expect("UTF-8 path"),
        ])
        .current_dir(temporary.path())
        .output()
        .expect("export Markdown");
    assert!(markdown_output.status.success(), "{markdown_output:?}");
    let markdown = fs::read_to_string(markdown_path).expect("read Markdown export");
    assert!(markdown.contains("# belay-trace export"));
    assert!(markdown.contains("not a managed mirror"));
    assert!(markdown.contains(&format!("# {decision}: Exported decision")));
    assert!(!markdown.contains(&work));

    let connection = Connection::open(&database).expect("reopen database");
    let after: (i64, i64, String) = connection
        .query_row(
            "
            SELECT
                (SELECT COUNT(*) FROM entries),
                (SELECT COUNT(*) FROM sync_state),
                (SELECT GROUP_CONCAT(display_id || ':' || revision || ':' || updated_at, '|')
                 FROM (SELECT display_id, revision, updated_at FROM entries ORDER BY display_id))
            ",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("snapshot state after export");
    assert_eq!(after, before);
    assert_eq!(
        fs::read_to_string(decision_mirror).expect("read unchanged decision mirror"),
        mirrors_before.0
    );
    assert_eq!(
        fs::read_to_string(work_mirror).expect("read unchanged work mirror"),
        mirrors_before.1
    );
}

#[test]
fn export_rejects_managed_mirror_destinations_and_is_deterministic() {
    let temporary = initialize_repository();
    created_id(
        &belay()
            .args([
                "add",
                "note",
                "--title",
                "Stable export",
                "--body",
                "Stable body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add note"),
    );
    let managed_output = temporary.path().join(".belay/entries/notes/export.json");
    let rejected = belay()
        .args([
            "export",
            "json",
            "--output",
            managed_output.to_str().expect("UTF-8 path"),
        ])
        .current_dir(temporary.path())
        .output()
        .expect("reject managed destination");
    assert_eq!(rejected.status.code(), Some(4));
    assert!(!managed_output.exists());
    let managed_new_directory = temporary.path().join(".belay/exported/new");
    let rejected_new_directory = belay()
        .args([
            "export",
            "json",
            "--output",
            managed_new_directory
                .join("snapshot.json")
                .to_str()
                .expect("UTF-8 path"),
        ])
        .current_dir(temporary.path())
        .output()
        .expect("reject new managed destination");
    assert_eq!(rejected_new_directory.status.code(), Some(4));
    assert!(!managed_new_directory.exists());
    let state_output = temporary.path().join(".belay/state/belay.sqlite");
    let state_before = fs::metadata(&state_output).expect("inspect database").len();
    let state_rejected = belay()
        .args([
            "export",
            "json",
            "--output",
            state_output.to_str().expect("UTF-8 path"),
        ])
        .current_dir(temporary.path())
        .output()
        .expect("reject managed state destination");
    assert_eq!(state_rejected.status.code(), Some(4));
    assert_eq!(
        fs::metadata(state_output)
            .expect("inspect preserved database")
            .len(),
        state_before
    );

    let output = temporary.path().join("snapshot.json");
    for _ in 0..2 {
        let exported = belay()
            .args([
                "export",
                "json",
                "--output",
                output.to_str().expect("UTF-8 path"),
            ])
            .current_dir(temporary.path())
            .output()
            .expect("write deterministic export");
        assert!(exported.status.success(), "{exported:?}");
        let current = fs::read(&output).expect("read export");
        if output.with_extension("first").exists() {
            assert_eq!(
                current,
                fs::read(output.with_extension("first")).expect("read first export")
            );
        } else {
            fs::write(output.with_extension("first"), current).expect("save first export");
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        fs::set_permissions(&output, fs::Permissions::from_mode(0o640))
            .expect("set export permissions");
        let exported = belay()
            .args([
                "export",
                "json",
                "--output",
                output.to_str().expect("UTF-8 path"),
            ])
            .current_dir(temporary.path())
            .output()
            .expect("replace export with preserved permissions");
        assert!(exported.status.success(), "{exported:?}");
        assert_eq!(
            fs::metadata(&output).expect("inspect export mode").mode() & 0o777,
            0o640
        );
    }
}

#[test]
fn goal_verify_coverage_and_compile_work_together() {
    let temporary = initialize_repository();
    let goal = created_id(
        &belay()
            .args(["add", "goal", "--title", "Reliable sync"])
            .current_dir(temporary.path())
            .output()
            .expect("add goal"),
    );
    let work = created_id(
        &belay()
            .args([
                "add",
                "work",
                "--title",
                "Implement reliable sync",
                "--body",
                "## Changes\n\nImplement reliable sync behavior.",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add work"),
    );
    let activated = belay()
        .args(["status", &goal, "active"])
        .current_dir(temporary.path())
        .output()
        .expect("activate goal");
    assert!(activated.status.success(), "{activated:?}");
    let linked = belay()
        .args(["link", &work, &goal, "--relation", "fulfills"])
        .current_dir(temporary.path())
        .output()
        .expect("link work to goal");
    assert!(linked.status.success(), "{linked:?}");

    let lint = belay()
        .args(["goal", "lint", &goal])
        .current_dir(temporary.path())
        .output()
        .expect("lint goal");
    assert!(lint.status.success(), "{lint:?}");
    let lint_stdout = String::from_utf8(lint.stdout).expect("lint stdout");
    assert!(lint_stdout.contains("Checklist:"));

    let evidence = belay()
        .args([
            "verify",
            "record",
            "--kind",
            "test",
            "--verdict",
            "pass",
            "--source",
            "cargo test",
            "--summary",
            "all tests passed",
            "--verifies",
            &goal,
            "--verifies",
            &work,
        ])
        .current_dir(temporary.path())
        .output()
        .expect("record evidence");
    assert!(evidence.status.success(), "{evidence:?}");
    assert!(temporary.path().join(".belay/evidence").exists());

    let coverage = belay()
        .args(["coverage", "--fail-under", "verified=0"])
        .current_dir(temporary.path())
        .output()
        .expect("coverage");
    assert!(coverage.status.success(), "{coverage:?}");
    let coverage_stdout = String::from_utf8(coverage.stdout).expect("coverage stdout");
    assert!(coverage_stdout.contains("Active goals: 1"));
    assert!(coverage_stdout.contains("traceability"));
    assert!(coverage_stdout.contains("verified"));

    let compiled = belay()
        .args(["context", "compile", "reliable sync", "--format", "agent"])
        .current_dir(temporary.path())
        .output()
        .expect("compile context");
    assert!(compiled.status.success(), "{compiled:?}");
    let compiled_stdout = String::from_utf8(compiled.stdout).expect("compiled stdout");
    assert!(compiled_stdout.contains("# Context: reliable sync"));
    assert!(compiled_stdout.contains("(compiled by belay, budget=2500)"));
    assert!(!compiled_stdout.contains("profile="));
    assert!(compiled_stdout.contains("## Goals"));
    assert!(compiled_stdout.contains(&goal));

    let removed_profile = belay()
        .args([
            "context",
            "compile",
            "reliable sync",
            "--profile",
            "task-start",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("reject removed profile option");
    assert_eq!(removed_profile.status.code(), Some(2));
    assert!(
        String::from_utf8(removed_profile.stderr)
            .expect("profile rejection stderr")
            .contains("--profile")
    );
}

#[test]
fn time_based_freshness_drives_status_doctor_and_coverage_without_git_metadata() {
    let temporary = initialize_repository();
    let goal = created_id(
        &belay()
            .args(["add", "goal", "--title", "Time-based freshness"])
            .current_dir(temporary.path())
            .output()
            .expect("add goal"),
    );
    let activated = belay()
        .args(["status", &goal, "active"])
        .current_dir(temporary.path())
        .output()
        .expect("activate goal");
    assert!(activated.status.success(), "{activated:?}");

    let recorded = belay()
        .args([
            "verify",
            "record",
            "--kind",
            "test",
            "--verdict",
            "pass",
            "--commit",
            "unknown",
            "--captured-at",
            "2000-01-01T00:00:00Z",
            "--source",
            "historical test",
            "--summary",
            "old passing evidence",
            "--verifies",
            &goal,
        ])
        .current_dir(temporary.path())
        .output()
        .expect("record old evidence");
    assert!(recorded.status.success(), "{recorded:?}");

    let status = belay()
        .args(["verify", "status", &goal])
        .current_dir(temporary.path())
        .output()
        .expect("show evidence status");
    assert!(status.status.success(), "{status:?}");
    assert!(
        String::from_utf8(status.stdout)
            .expect("status stdout")
            .contains("stale (older than 14 days (commit unknown))")
    );

    let doctor = belay()
        .arg("doctor")
        .current_dir(temporary.path())
        .output()
        .expect("check stale evidence");
    assert!(doctor.status.success(), "{doctor:?}");
    assert!(
        String::from_utf8(doctor.stdout)
            .expect("doctor stdout")
            .contains("depends on stale evidence (older than 14 days (commit unknown))")
    );

    let coverage = belay()
        .args(["coverage", "--format", "json"])
        .current_dir(temporary.path())
        .output()
        .expect("compute coverage");
    assert!(coverage.status.success(), "{coverage:?}");
    let report: serde_json::Value =
        serde_json::from_slice(&coverage.stdout).expect("coverage JSON");
    let test_dimension = report["dimensions"]
        .as_array()
        .expect("coverage dimensions")
        .iter()
        .find(|dimension| dimension["name"] == "test")
        .expect("test dimension");
    assert_eq!(test_dimension["verified"]["covered"], 0);
}

#[test]
fn evidence_status_orders_offset_timestamps_by_instant() {
    let temporary = initialize_repository();
    let goal = created_id(
        &belay()
            .args(["add", "goal", "--title", "Evidence ordering"])
            .current_dir(temporary.path())
            .output()
            .expect("add goal"),
    );

    for (captured_at, source) in [
        ("2026-07-22T01:00:00+02:00", "older-offset-evidence"),
        ("2026-07-22T00:30:00Z", "newer-utc-evidence"),
    ] {
        let recorded = belay()
            .args([
                "verify",
                "record",
                "--kind",
                "test",
                "--verdict",
                "pass",
                "--commit",
                "unknown",
                "--captured-at",
                captured_at,
                "--source",
                source,
                "--summary",
                "ordering evidence",
                "--verifies",
                &goal,
            ])
            .current_dir(temporary.path())
            .output()
            .expect("record ordered evidence");
        assert!(recorded.status.success(), "{recorded:?}");
    }

    let status = belay()
        .args(["verify", "status", &goal])
        .current_dir(temporary.path())
        .output()
        .expect("show ordered evidence");
    assert!(status.status.success(), "{status:?}");
    let stdout = String::from_utf8(status.stdout).expect("status stdout");
    let newer = stdout.find("newer-utc-evidence").expect("newer evidence");
    let older = stdout
        .find("older-offset-evidence")
        .expect("older evidence");
    assert!(newer < older, "{stdout}");
}

#[test]
fn route_cli_materializes_only_the_exact_accepted_preview_and_retries_idempotently() {
    let temporary = initialize_repository();
    let goal = created_id(
        &belay()
            .args(["add", "goal", "--title", "Route seed"])
            .current_dir(temporary.path())
            .output()
            .expect("add Route seed"),
    );

    let started = belay()
        .args(["route", "start", "--seed", &goal])
        .current_dir(temporary.path())
        .output()
        .expect("start Route run");
    assert!(started.status.success(), "{started:?}");
    let stdout = String::from_utf8(started.stdout).expect("start stdout");
    let run_id = stdout
        .lines()
        .find_map(|line| line.strip_prefix("Created Route run "))
        .expect("Route run ID")
        .to_owned();
    let run_path = temporary.path().join(".belay/state/route").join(&run_id);
    let input: serde_json::Value = serde_json::from_slice(
        &fs::read(run_path.join("input-001.json")).expect("read Route Input"),
    )
    .expect("parse Route Input");
    let fingerprint = input["fingerprint"].as_str().expect("input fingerprint");
    let assessment_template = belay()
        .args(["route", "template", &run_id, "assessment"])
        .current_dir(temporary.path())
        .output()
        .expect("render Assessment template");
    assert!(assessment_template.status.success());
    let assessment_template: serde_json::Value =
        serde_json::from_slice(&assessment_template.stdout).expect("parse Assessment template");
    assert_eq!(assessment_template["input_fingerprint"], fingerprint);

    let assessment_path = temporary.path().join("assessment.json");
    fs::write(
        &assessment_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1,
            "artifact_type": "route-assessment",
            "run_id": run_id,
            "revision": 1,
            "input_fingerprint": fingerprint,
            "outcome": "continue",
            "items": [{
                "id": "fact-001",
                "classification": "fact",
                "summary": "The seed Goal exists.",
                "references": [goal]
            }],
            "limitations": []
        }))
        .expect("serialize Assessment"),
    )
    .expect("write Assessment");
    let submitted = belay()
        .args([
            "route",
            "submit",
            &run_id,
            "assessment",
            "--file",
            assessment_path.to_str().expect("assessment path"),
        ])
        .current_dir(temporary.path())
        .output()
        .expect("submit Assessment");
    assert!(submitted.status.success(), "{submitted:?}");

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(run_path.join("manifest.json")).expect("read manifest"))
            .expect("parse manifest");
    let assessment_hash = manifest["assessment"]["sha256"]
        .as_str()
        .expect("assessment hash");
    let proposal_path = temporary.path().join("proposal.json");
    fs::write(
        &proposal_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1,
            "artifact_type": "route-proposal",
            "run_id": run_id,
            "revision": 1,
            "input_fingerprint": fingerprint,
            "assessment_hash": assessment_hash,
            "outcome": "continue",
            "summary": "Record the accepted Route decision.",
            "operations": [{
                "kind": "create-entry",
                "operation_id": "op-001",
                "alias": "decision",
                "type": "decision",
                "title": "Route-created decision",
                "body": "Created from an explicitly accepted Route preview."
            }, {
                "kind": "link",
                "operation_id": "op-002",
                "from": "$decision",
                "to": goal,
                "relation": "supports"
            }, {
                "kind": "link",
                "operation_id": "op-003",
                "from": "$decision",
                "to": goal,
                "relation": "supports"
            }, {
                "kind": "set-status",
                "operation_id": "op-004",
                "target": goal,
                "status": "active"
            }]
        }))
        .expect("serialize Proposal"),
    )
    .expect("write Proposal");
    let submitted = belay()
        .args([
            "route",
            "submit",
            &run_id,
            "proposal",
            "--file",
            proposal_path.to_str().expect("proposal path"),
        ])
        .current_dir(temporary.path())
        .output()
        .expect("submit Proposal");
    assert!(submitted.status.success(), "{submitted:?}");

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(run_path.join("manifest.json")).expect("read manifest"))
            .expect("parse manifest");
    let proposal_hash = manifest["proposal"]["sha256"]
        .as_str()
        .expect("proposal hash");
    let response_path = temporary.path().join("response.json");
    fs::write(
        &response_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1,
            "artifact_type": "human-response",
            "run_id": run_id,
            "revision": 1,
            "input_fingerprint": fingerprint,
            "proposal_revision": 1,
            "proposal_hash": proposal_hash,
            "action": "accept",
            "selected_operation_ids": ["op-001", "op-002", "op-003", "op-004"],
            "reason": "Approved in the test.",
            "requested_changes": [],
            "issuer": "test:user",
            "responded_at": "2026-07-29T21:00:00+09:00"
        }))
        .expect("serialize Human Response"),
    )
    .expect("write Human Response");
    let submitted = belay()
        .args([
            "route",
            "submit",
            &run_id,
            "response",
            "--file",
            response_path.to_str().expect("response path"),
        ])
        .current_dir(temporary.path())
        .output()
        .expect("submit Human Response");
    assert!(submitted.status.success(), "{submitted:?}");
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(run_path.join("manifest.json")).expect("read manifest"))
            .expect("parse manifest");
    let response_hash = manifest["response"]["sha256"]
        .as_str()
        .expect("response hash");

    let previewed = belay()
        .args(["route", "preview", &run_id])
        .current_dir(temporary.path())
        .output()
        .expect("preview Route materialization");
    assert!(previewed.status.success(), "{previewed:?}");
    let preview_stdout = String::from_utf8(previewed.stdout).expect("preview stdout");
    let preview_hash = preview_stdout
        .lines()
        .find_map(|line| line.strip_prefix("Approve exact preview hash: "))
        .expect("preview hash");

    let pending = belay()
        .args(["route", "pending", &run_id])
        .current_dir(temporary.path())
        .output()
        .expect("show pending Preview");
    assert!(pending.status.success(), "{pending:?}");
    let pending: serde_json::Value =
        serde_json::from_slice(&pending.stdout).expect("parse pending Preview");
    assert_eq!(pending["run_id"], run_id);
    assert_eq!(pending["preview_revision"], 1);
    assert_eq!(pending["preview_hash"], preview_hash);
    assert_eq!(pending["input_fingerprint"], fingerprint);
    assert_eq!(pending["proposal_revision"], 1);
    assert_eq!(pending["proposal_hash"], proposal_hash);
    assert_eq!(pending["response_revision"], 1);
    assert_eq!(pending["response_hash"], response_hash);
    assert_eq!(pending["operations"].as_array().map(Vec::len), Some(4));

    let replacement = belay()
        .args(["route", "preview", &run_id])
        .current_dir(temporary.path())
        .output()
        .expect("replace pending Preview");
    assert!(replacement.status.success(), "{replacement:?}");
    let replacement_stdout = String::from_utf8(replacement.stdout).expect("replacement stdout");
    let replacement_hash = replacement_stdout
        .lines()
        .find_map(|line| line.strip_prefix("Approve exact preview hash: "))
        .expect("replacement preview hash");
    assert_ne!(replacement_hash, preview_hash);
    let pending = belay()
        .args(["route", "pending", &run_id])
        .current_dir(temporary.path())
        .output()
        .expect("show replacement pending Preview");
    assert!(pending.status.success(), "{pending:?}");
    let pending: serde_json::Value =
        serde_json::from_slice(&pending.stdout).expect("parse replacement pending Preview");
    assert_eq!(pending["preview_revision"], 2);
    assert_eq!(pending["preview_hash"], replacement_hash);
    assert_eq!(pending["input_fingerprint"], fingerprint);
    assert_eq!(pending["proposal_hash"], proposal_hash);
    assert_eq!(pending["response_hash"], response_hash);
    assert_eq!(pending["operations"].as_array().map(Vec::len), Some(4));

    let replaced = belay()
        .args(["route", "apply", &run_id, "--approve", preview_hash])
        .current_dir(temporary.path())
        .output()
        .expect("reject replaced pending Preview");
    assert_eq!(replaced.status.code(), Some(4));

    let rejected = belay()
        .args(["route", "apply", &run_id, "--approve", "wrong-hash"])
        .current_dir(temporary.path())
        .output()
        .expect("reject wrong preview hash");
    assert_eq!(rejected.status.code(), Some(4));

    let applied = belay()
        .args(["route", "apply", &run_id, "--approve", replacement_hash])
        .current_dir(temporary.path())
        .output()
        .expect("apply accepted preview");
    assert!(applied.status.success(), "{applied:?}");
    let first_result: serde_json::Value = serde_json::from_slice(
        &fs::read(run_path.join("reconciliation-001.json")).expect("read reconciliation"),
    )
    .expect("parse reconciliation");
    assert_eq!(first_result["operations"][0]["state"], "applied");
    assert_eq!(first_result["operations"][1]["state"], "applied");
    assert_eq!(first_result["operations"][2]["state"], "unchanged");
    assert_eq!(first_result["operations"][3]["state"], "applied");
    let created_id = first_result["aliases"]["decision"]
        .as_str()
        .expect("created alias");
    assert!(
        temporary
            .path()
            .join(".belay/entries/decisions")
            .join(format!("{created_id}.md"))
            .is_file()
    );

    fs::remove_file(run_path.join("reconciliation-001.json"))
        .expect("simulate crash before Reconciliation persistence");
    let mut applying_manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(run_path.join("manifest.json")).expect("read manifest"))
            .expect("parse manifest");
    applying_manifest["phase"] = serde_json::json!("applying");
    applying_manifest["reconciliation"] = serde_json::Value::Null;
    fs::write(
        run_path.join("manifest.json"),
        serde_json::to_vec_pretty(&applying_manifest).expect("serialize applying manifest"),
    )
    .expect("restore applying manifest");
    let recovered = belay()
        .args(["route", "apply", &run_id, "--approve", replacement_hash])
        .current_dir(temporary.path())
        .output()
        .expect("resume after Reconciliation persistence crash");
    assert!(recovered.status.success(), "{recovered:?}");
    let recovered_result: serde_json::Value = serde_json::from_slice(
        &fs::read(run_path.join("reconciliation-001.json")).expect("read recovered reconciliation"),
    )
    .expect("parse recovered reconciliation");
    assert_eq!(recovered_result["operations"][0]["state"], "unchanged");
    assert_eq!(recovered_result["operations"][1]["state"], "unchanged");
    assert_eq!(recovered_result["operations"][2]["state"], "unchanged");
    assert_eq!(recovered_result["operations"][3]["state"], "unchanged");

    let rebuilt = belay()
        .arg("rebuild")
        .current_dir(temporary.path())
        .output()
        .expect("rebuild while preserving Route receipts");
    assert!(rebuilt.status.success(), "{rebuilt:?}");

    let retried = belay()
        .args(["route", "apply", &run_id, "--approve", replacement_hash])
        .current_dir(temporary.path())
        .output()
        .expect("retry accepted preview");
    assert!(retried.status.success(), "{retried:?}");
    let second_result: serde_json::Value = serde_json::from_slice(
        &fs::read(run_path.join("reconciliation-002.json")).expect("read retry reconciliation"),
    )
    .expect("parse retry reconciliation");
    assert_eq!(second_result["operations"][0]["state"], "unchanged");
    assert_eq!(second_result["operations"][1]["state"], "unchanged");
    assert_eq!(second_result["operations"][2]["state"], "unchanged");
    assert_eq!(second_result["operations"][3]["state"], "unchanged");
    assert_eq!(second_result["aliases"]["decision"], created_id);

    fs::remove_file(run_path.join("reconciliation-002.json"))
        .expect("simulate a second crash before Reconciliation persistence");
    fs::remove_file(run_path.join("reconciliation-001.json"))
        .expect("remove earlier recovered Reconciliation");
    let mut applying_manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(run_path.join("manifest.json")).expect("read manifest"))
            .expect("parse manifest");
    applying_manifest["phase"] = serde_json::json!("applying");
    applying_manifest["reconciliation"] = serde_json::Value::Null;
    fs::write(
        run_path.join("manifest.json"),
        serde_json::to_vec_pretty(&applying_manifest).expect("serialize applying manifest"),
    )
    .expect("restore applying manifest before conflict");

    let externally_changed = belay()
        .args(["status", &goal, "completed"])
        .current_dir(temporary.path())
        .output()
        .expect("change a receipt target after apply");
    assert!(
        externally_changed.status.success(),
        "{externally_changed:?}"
    );
    let conflicted = belay()
        .args(["route", "apply", &run_id, "--approve", replacement_hash])
        .current_dir(temporary.path())
        .output()
        .expect("record receipt postcondition conflict");
    assert_eq!(conflicted.status.code(), Some(6));
    let conflict_result: serde_json::Value = serde_json::from_slice(
        &fs::read(run_path.join("reconciliation-001.json")).expect("read conflict reconciliation"),
    )
    .expect("parse conflict reconciliation");
    assert!(
        conflict_result["operations"]
            .as_array()
            .expect("conflict operations")
            .iter()
            .any(|operation| operation["state"] == "failed")
    );

    let conflicted_again = belay()
        .args(["route", "apply", &run_id, "--approve", replacement_hash])
        .current_dir(temporary.path())
        .output()
        .expect("retry receipt postcondition conflict");
    assert_eq!(conflicted_again.status.code(), Some(6));
    let goal_after_retry = belay()
        .args(["show", &goal])
        .current_dir(temporary.path())
        .output()
        .expect("show goal after conflict retry");
    assert!(goal_after_retry.status.success(), "{goal_after_retry:?}");
    let goal_after_retry =
        String::from_utf8(goal_after_retry.stdout).expect("parse goal after retry");
    assert!(goal_after_retry.contains("Status: completed"));
}

#[test]
fn route_commands_reject_a_stale_input() {
    let temporary = initialize_repository();
    let goal = created_id(
        &belay()
            .args(["add", "goal", "--title", "Stale Route seed"])
            .current_dir(temporary.path())
            .output()
            .expect("add Route seed"),
    );
    let started = belay()
        .args(["route", "start", "--seed", &goal])
        .current_dir(temporary.path())
        .output()
        .expect("start Route run");
    assert!(started.status.success(), "{started:?}");

    let changed = belay()
        .args(["status", &goal, "active"])
        .current_dir(temporary.path())
        .output()
        .expect("change seed after Route start");
    assert!(changed.status.success(), "{changed:?}");

    let status = belay()
        .args(["route", "status", "ROUTE-invalid/path"])
        .current_dir(temporary.path())
        .output()
        .expect("reject unsafe Route ID");
    assert_eq!(status.status.code(), Some(4));

    let stdout = String::from_utf8(started.stdout).expect("start stdout");
    let run_id = stdout
        .lines()
        .find_map(|line| line.strip_prefix("Created Route run "))
        .expect("Route run ID");
    let status = belay()
        .args(["route", "status", run_id])
        .current_dir(temporary.path())
        .output()
        .expect("show stale run manifest");
    assert!(status.status.success());
    let manifest: serde_json::Value =
        serde_json::from_slice(&status.stdout).expect("parse run status");
    assert_eq!(manifest["phase"], "started");

    let missing = belay()
        .args(["route", "template", run_id, "assessment"])
        .current_dir(temporary.path())
        .output()
        .expect("reject stale Route before template");
    assert_eq!(missing.status.code(), Some(5));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("Route Input is stale"));
}

#[cfg(unix)]
#[test]
fn route_rejects_a_symlinked_state_root_without_external_write() {
    let temporary = initialize_repository();
    let goal = created_id(
        &belay()
            .args(["add", "goal", "--title", "Safe Route root"])
            .current_dir(temporary.path())
            .output()
            .expect("add Route seed"),
    );
    let external = temporary.path().join("external-route-state");
    fs::create_dir(&external).expect("create external directory");
    std::os::unix::fs::symlink(&external, temporary.path().join(".belay/state/route"))
        .expect("symlink Route root");

    let started = belay()
        .args(["route", "start", "--seed", &goal])
        .current_dir(temporary.path())
        .output()
        .expect("reject symlinked Route root");
    assert_eq!(started.status.code(), Some(4));
    assert!(
        String::from_utf8_lossy(&started.stderr).contains("must be a directory and not a symlink")
    );
    assert_eq!(
        fs::read_dir(&external)
            .expect("read external directory")
            .count(),
        0
    );
}

#[test]
fn show_and_status_resolve_unique_prefix_and_slug() {
    let temporary = initialize_repository();
    let id = created_id(
        &belay()
            .args([
                "add",
                "decision",
                "--title",
                "Unique prefix slug",
                "--body",
                "Keep IDs short to resolve.",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add decision"),
    );
    let parts = belay_trace::entry::parse_display_id(&id).expect("parse id");
    let prefix = format!("DEC-{}", &parts.timestamp[..8]);
    let shown = belay()
        .args(["show", &prefix])
        .current_dir(temporary.path())
        .output()
        .expect("show prefix");
    assert!(shown.status.success(), "{shown:?}");
    let stdout = String::from_utf8(shown.stdout).expect("stdout");
    assert!(stdout.contains(&format!("ID: {id}")), "{stdout}");

    let by_slug = belay()
        .args(["show", &parts.slug])
        .current_dir(temporary.path())
        .output()
        .expect("show slug");
    assert!(by_slug.status.success(), "{by_slug:?}");

    let accepted = belay()
        .args(["status", &parts.slug, "accepted"])
        .current_dir(temporary.path())
        .output()
        .expect("status by slug");
    assert!(accepted.status.success(), "{accepted:?}");
}

#[test]
fn show_rejects_an_ambiguous_slug() {
    let temporary = initialize_repository();
    created_id(
        &belay()
            .args(["add", "note", "--title", "Shared stem one", "--body", "A"])
            .current_dir(temporary.path())
            .output()
            .expect("add first"),
    );
    created_id(
        &belay()
            .args(["add", "note", "--title", "Shared stem two", "--body", "B"])
            .current_dir(temporary.path())
            .output()
            .expect("add second"),
    );
    let output = belay()
        .args(["show", "shared-stem"])
        .current_dir(temporary.path())
        .output()
        .expect("ambiguous slug");
    assert_eq!(output.status.code(), Some(4), "{output:?}");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("ambiguous"), "{stderr}");
}

#[test]
fn archived_entries_are_hidden_from_default_search_and_visible_with_opt_in() {
    let temporary = initialize_repository();
    let note = created_id(
        &belay()
            .args([
                "add",
                "note",
                "--title",
                "Hide me later",
                "--body",
                "note body",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add note"),
    );
    let archived = belay()
        .args(["status", &note, "archived"])
        .current_dir(temporary.path())
        .output()
        .expect("archive note");
    assert!(archived.status.success(), "{archived:?}");

    let hidden = belay()
        .args(["search", "Hide"])
        .current_dir(temporary.path())
        .output()
        .expect("search default");
    assert!(hidden.status.success(), "{hidden:?}");
    let stdout = String::from_utf8(hidden.stdout).expect("stdout");
    assert!(!stdout.contains(&note), "{stdout}");

    let included = belay()
        .args(["search", "Hide", "--include-archived"])
        .current_dir(temporary.path())
        .output()
        .expect("search include archived");
    let included = String::from_utf8(included.stdout).expect("stdout");
    assert!(included.contains(&note), "{included}");

    let shown = belay()
        .args(["show", &note])
        .current_dir(temporary.path())
        .output()
        .expect("show archived");
    assert!(shown.status.success(), "{shown:?}");
}

#[test]
fn archive_candidates_lists_completed_unlinked_work() {
    let temporary = initialize_repository();
    let work = created_id(
        &belay()
            .args([
                "add",
                "work",
                "--title",
                "Finished isolated work",
                "--body",
                "done",
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add work"),
    );
    assert!(
        belay()
            .args(["status", &work, "completed"])
            .current_dir(temporary.path())
            .output()
            .expect("complete work")
            .status
            .success()
    );
    let output = belay()
        .args(["archive", "candidates"])
        .current_dir(temporary.path())
        .output()
        .expect("list candidates");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains(&work), "{stdout}");
    assert!(stdout.contains("terminal-status"), "{stdout}");
}

#[test]
fn context_without_a_task_emits_the_working_set_and_next_index() {
    let temporary = initialize_repository();
    let body = concat!(
        "## Intent Brief\n\n",
        "### Constraints\n- stay local\n\n",
        "### Non-goals\n- no remote publish\n\n",
        "### Assumptions\n- None identified\n\n",
        "### Unknowns / Decisions Needed\n- None identified\n\n",
        "## Delivery Map\n\n",
        "| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |\n",
        "| --- | --- | --- | --- | --- | --- |\n",
        "| T-001 | SC-001 | First live task | AI | not-started | pending |\n\n",
        "## T-001\n\n",
        "- **Objective**: retrieve the working set.\n",
        "- **Scope**: in — compile. out — remote.\n",
        "- **Steps**: run belay context compile.\n",
        "- **Acceptance**: Next lists T-001.\n",
        "- **Verification**: cargo test\n",
    );
    let plan = created_id(
        &belay()
            .args([
                "add",
                "plan",
                "--title",
                "Live working plan",
                "--body",
                body,
            ])
            .current_dir(temporary.path())
            .output()
            .expect("add plan"),
    );
    let compiled = belay()
        .args([
            "context", "compile", "--format", "agent", "--budget", "4000",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("working set compile");
    assert!(compiled.status.success(), "{compiled:?}");
    let stdout = String::from_utf8(compiled.stdout).expect("stdout");
    assert!(stdout.contains("# Working set"), "{stdout}");
    assert!(stdout.contains(&format!("{plan}#t-001")), "{stdout}");
    assert!(stdout.contains("First live task"), "{stdout}");
}

#[test]
fn context_focus_prints_a_task_packet_without_sibling_tasks() {
    let temporary = initialize_repository();
    let body = concat!(
        "## Intent Brief\n\n",
        "### Constraints\n- stay local\n\n",
        "### Non-goals\n- no sibling leak\n\n",
        "### Assumptions\n- None identified\n\n",
        "### Unknowns / Decisions Needed\n- None identified\n\n",
        "## Delivery Map\n\n",
        "| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |\n",
        "| --- | --- | --- | --- | --- | --- |\n",
        "| T-001 | SC-001 | Packet task | AI | not-started | pending |\n",
        "| T-002 | SC-002 | Hidden sibling | AI | not-started | pending |\n\n",
        "## T-001\n\n",
        "- **Objective**: emit one packet.\n",
        "- **Scope**: in — focus. out — T-002.\n",
        "- **Steps**: compile --focus.\n",
        "- **Acceptance**: sibling absent.\n",
        "- **Verification**: cargo test\n\n",
        "## T-002\n\n",
        "- **Objective**: must not appear.\n",
        "- **Scope**: out.\n",
        "- **Steps**: none.\n",
        "- **Acceptance**: hidden.\n",
        "- **Verification**: none\n",
    );
    let plan = created_id(
        &belay()
            .args(["add", "plan", "--title", "Packet plan", "--body", body])
            .current_dir(temporary.path())
            .output()
            .expect("add plan"),
    );
    let output = belay()
        .args([
            "context",
            "compile",
            "--focus",
            &format!("{plan}#t-001"),
            "--format",
            "agent",
            "--budget",
            "2500",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("focus compile");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("# Task packet:"), "{stdout}");
    assert!(stdout.contains("stay local"), "{stdout}");
    assert!(stdout.contains("emit one packet"), "{stdout}");
    assert!(!stdout.contains("must not appear"), "{stdout}");
    assert!(!stdout.contains("Hidden sibling"), "{stdout}");
}

#[test]
fn generated_skill_documents_working_set_focus_and_archive_candidates() {
    let temporary = initialize_repository();
    let skill = fs::read_to_string(temporary.path().join(".belay/agent/codex/SKILL.md"))
        .expect("read generated skill");
    for expected in [
        "belay context compile --format agent --budget 4000",
        "belay context compile --focus",
        "belay archive candidates",
        "Do not start implementation while Unknowns",
    ] {
        assert!(skill.contains(expected), "missing {expected} in skill");
    }
}
