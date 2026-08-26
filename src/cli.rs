use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

use crate::agent;
use crate::archive;
use crate::browse::{self, BrowseOptions};
use crate::context;
use crate::coverage;
use crate::entry::{EntryStatus, EntryType, LinkRelation};
use crate::error::BelayError;
use crate::evidence;
use crate::export;
use crate::goal;
use crate::plan;
use crate::reconcile;
use crate::repository;
use crate::route;
use crate::search::{self, SearchRequest};
use crate::store::{self, MutationOutcome};

const TOP_LEVEL_ABOUT: &str = "Preserve goals, plans, decisions, work, reviews, and evidence in a local SQLite store with a tracked Markdown review surface.\n\nWorkflow groups:\n  Setup: init and doctor\n  Capture: add, link, status, and show\n  Assurance: goal, verify, and coverage\n  Reconcile: sync and rebuild\n  Retrieve: search, context, and export\n\nThe core workflow is: initialize a repository, add and link trace entries, synchronize direct Markdown edits, then retrieve focused context with search and context commands. Archived entries stay in history and drop out of default search and compile.";

const TOP_LEVEL_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Project commands discover the current repository root and read .belay/config.toml.
  `belay init` creates .belay/ state and tracked support files. It does not modify
  AGENTS.md or install agent skills by default.

Examples:
  belay init
  belay add decision --title "Use SQLite" --body "Keep operational state local."
  belay add goal --title "Reliable sync"
  belay goal lint --all
  belay verify record --kind test --verdict pass --source "cargo test" --summary "all tests passed" --verifies GOAL-...
  belay search "sqlite migration"
  belay context compile "implement repository sync" --format agent --budget 2500

Exit Status:
  0  Success, help, or version output
  2  Invalid invocation or arguments
  3  Repository not initialized or configuration unavailable
  4  Entry, schema, input validation, not-found, or unavailable feature
  5  Sync conflict or drift requiring explicit resolution
  6  Storage, filesystem, SQLite, or runtime capability failure

Related Commands:
  Start with `belay init`. Use `belay <command> --help` for command-specific
  behavior, side effects, examples, and related commands."#;

const INIT_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Creates or completes .belay/config.toml, managed entry directories, local SQLite
  state, .belay/.gitignore, and deterministic agent integration templates. Managed
  generated templates are refreshed. AGENTS.md and agent skills are modified only
  by the explicit --update-agents and --install-skill options. --reset-state
  atomically rebuilds ignored SQLite state from tracked Markdown.

Examples:
  cd /path/to/repository
  belay init
  belay init --update-agents
  belay init --install-skill codex
  belay init --install-skill claude
  belay init --install-skill codex --install-skill claude
  belay init --reset-state

Exit Status:
  0  Repository initialized or already complete
  2  Invalid invocation
  3  Existing configuration is unavailable
  4  Configuration validation failed
  6  Filesystem, SQLite, migration, or runtime capability failure

Related Commands:
  `belay doctor` checks initialized state. `belay add --help` describes entry creation."#;

const ADD_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Creates an entry in SQLite, updates chunks and FTS state, writes its deterministic
  managed Markdown mirror, and records a sync baseline. Goal entries may omit a
  body source; belay then writes the required Goal sections as a template.

Examples:
  belay add decision --title "Use SQLite" --body "Keep operational state local."
  belay add goal --title "Reliable sync"

Exit Status:
  0  Entry created
  2  Invalid invocation
  3  Repository not initialized
  4  Entry type, title, body, or input validation failure
  6  Storage failure

Related Commands:
  `belay link`, `belay status`, `belay show`, and `belay sync`."#;

const LINK_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Adds a validated directional relationship between existing entries. Identical links
  are idempotent. Targets may include Goal item fragments such as #sc-3f9a.
  The source entry revision, mirror, and sync baseline are updated.
  Allowed relations include references, implements, reviews, supersedes,
  follows-up, fulfills, supports, verifies, and refutes.

Examples:
  belay link WRK-20260606T120000-001-schema DEC-20260606T115000-001-sqlite --relation implements

Exit Status:
  0  Link created or already present
  2  Invalid invocation
  3  Repository not initialized
  4  Invalid relation, invalid display ID, or entry not found
  5  Unsynchronized Markdown or SQLite changes require `belay sync`
  6  Storage failure

Related Commands:
  `belay add`, `belay show`, and `belay sync`."#;

const STATUS_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Validates and applies a type-specific status. Setting the current status is an
  idempotent no-op; changes update the entry revision, mirror, and sync baseline.
  Status membership is validated for plan, decision, work, review, and note entries.

Examples:
  belay status DEC-20260606T115000-001-sqlite accepted

Exit Status:
  0  Status updated
  2  Invalid invocation
  3  Repository not initialized
  4  Invalid status, invalid display ID, or entry not found
  5  Unsynchronized Markdown or SQLite changes require `belay sync`
  6  Storage failure

Related Commands:
  `belay show`, `belay link`, and `belay sync`."#;

const SHOW_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Displays one complete entry, its managed source path, and inbound and outbound links
  using display IDs. It does not change repository state.

  Appending a canonical fragment displays only that item: the line defining it
  and the body section headed with its ID. Use it to read one Success Criterion
  or one Delivery Map task without paying for the whole entry. A fragment that
  is not canonical, not present, or ambiguous fails rather than falling back to
  the complete entry. Display IDs may be a unique prefix or slug; 0 or 2+ matches
  fail with the candidate list.

  Examples:
  belay show DEC-20260606T115000-001-sqlite
  belay show PLN-20260723T120100-001-deliver-safe-sync#t-001
  belay show GOAL-20260723T120000-001-safe-sync#sc-001
  belay show retrieval-hygiene-archive
  belay show PLN-20260723#t-001

Exit Status:
  0  Entry displayed
  2  Invalid invocation
  3  Repository not initialized
  4  Invalid display ID, invalid fragment, or entry not found
  6  Storage failure

Related Commands:
  `belay search` and `belay context`."#;

const SEARCH_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Performs exact display-ID lookup, structured filtering, or deduplicated
  FTS5/BM25-ranked keyword retrieval. Search does not mutate repository state.
  Plain query words are escaped as FTS terms; multiple words match with OR.
  Archived entries are omitted unless `--include-archived` or `--status archived`
  is passed.

Examples:
  belay search "sqlite migration"
  belay search --type decision --status accepted
  belay search --status draft --status active
  belay search --exclude-status completed --exclude-status abandoned
  belay search --include-archived
  belay search --id DEC-20260606T115000-001-sqlite
  belay search --id DEC-20260606T115000-001-sqlite

Exit Status:
  0  Search completed, including no matches
  2  Invalid invocation
  3  Repository not initialized
  4  Query, filter, or limit validation error
  6  Storage or runtime capability failure

Related Commands:
  `belay show` and `belay context`."#;

const CONTEXT_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Produces source-attributed context from ranked matches and directly linked entries.
  Selection preserves relevance order, guarantees evidence for every included entry,
  admits a budget-scaled set of candidates, and gives higher-ranked entries a
  larger share of remaining excerpt space. `belay context compile` adds Goal,
  Evidence, constraints, non-goals, and past-failure sections before ranked context.
  Selection uses 90 percent of the requested budget, reserving 10 percent headroom.
  Token estimates use ceil(ASCII UTF-8 bytes / 4) plus non-ASCII scalar count.
  The default budget is 2500; budgets below 64 are rejected.
  This command does not mutate repository state.
  Archived entries are omitted unless `--include-archived` is passed.
  With no task, compile emits the live working set and a Next index.
  `--focus PLN-...#t-001` emits a task packet instead of ranked history.

Examples:
  belay context "implement repository sync" --format agent --budget 2500
  belay context compile "implement repository sync" --budget 4000
  belay context compile --format agent --budget 4000
  belay context compile --focus PLN-20260723T120100-001-deliver-safe-sync#t-001

Exit Status:
  0  Context generated
  2  Invalid invocation
  3  Repository not initialized
  4  Invalid focus, budget, or repository state
  6  Storage failure

Related Commands:
  `belay search` and `belay show`."#;

const ARCHIVE_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Lists deterministic archive candidates without changing status. Candidates are
  entries already in a terminal status, the old side of a supersedes link, or
  both. Semantic judgment and `belay status <id> archived` stay with the caller.
  This command does not mutate repository state.

Examples:
  belay archive candidates

Exit Status:
  0  Candidates listed, including none
  2  Invalid invocation
  3  Repository not initialized
  6  Storage failure

Related Commands:
  `belay status`, `belay search --include-archived`, and `belay context`."#;

const GOAL_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Reviews Goal entries without calling an LLM. Lint performs deterministic checks;
  improve emits a structured bundle for an agent-mediated semantic review.

Examples:
  belay goal lint GOAL-20260701T090000-001-reliable-sync
  belay goal lint --all --format json
  belay goal improve GOAL-20260701T090000-001-reliable-sync --budget 3000

Exit Status:
  0  Goal command completed, including non-strict lint findings
  2  Invalid invocation
  3  Repository not initialized
  4  Invalid goal, strict lint failure, or output validation failure
  6  Storage failure

Related Commands:
  `belay add goal`, `belay context compile`, and `belay coverage`."#;

const PLAN_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Reviews Plan entries without calling an LLM. Lint performs deterministic
  structural checks on the Delivery Map: task rows exist, task IDs are canonical
  and unique, states come from the fixed set, every task ID has a `## T-NNN`
  section, and that section carries the baseline fields. Fields belay does not
  require are ignored, so a consumer may extend a task section. A Plan with no
  Delivery Map is skipped rather than failed.

Examples:
  belay plan lint PLN-20260701T090500-001-deliver-sync
  belay plan lint --all --format json

Exit Status:
  0  Plan command completed, including non-strict lint findings
  2  Invalid invocation
  3  Repository not initialized
  4  Invalid plan, strict lint failure, or output validation failure
  6  Storage failure

Related Commands:
  `belay goal lint`, `belay show <id>#t-NNN`, and `belay coverage`."#;

const VERIFY_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Records append-only Evidence in .belay/evidence/*.ndjson and indexes it in SQLite.
  Evidence is linked to Goal, Decision, Work, or Goal item references and is never
  edited in place; updates are represented by new records.

Examples:
  belay verify record --kind test --verdict pass --source "cargo test" --summary "all tests passed" --verifies GOAL-...
  belay verify import --junit target/junit.xml --verifies WRK-...
  belay verify status GOAL-...

Exit Status:
  0  Evidence command completed
  2  Invalid invocation
  3  Repository not initialized
  4  Invalid evidence, target, or file
  6  Storage or filesystem failure

Related Commands:
  `belay coverage`, `belay doctor`, and `belay rebuild`."#;

const COVERAGE_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Computes Goal Coverage for active Goals, separating link-based traceability from
  fresh passing Evidence-backed verification. `--fail-under verified=N` can be used
  as a CI gate and returns exit status 4 on failure.

Examples:
  belay coverage
  belay coverage GOAL-20260701T090000-001-reliable-sync
  belay coverage --format json --fail-under verified=60

Exit Status:
  0  Coverage computed and thresholds satisfied
  2  Invalid invocation
  3  Repository not initialized
  4  Invalid target or fail-under threshold not met
  6  Storage failure

Related Commands:
  `belay goal lint`, `belay verify status`, and `belay context compile`."#;

const SYNC_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Reconciles SQLite with managed Markdown using the last successful sync hashes.
  Markdown-only changes are imported, SQLite-only changes are rendered, missing
  counterparts are restored, and conflicts preserve both sides. Batch sync is
  atomic per entry. Explicit --prefer resolution is entry-scoped.

Examples:
  belay sync
  belay sync DEC-20260606T115000-001-sqlite
  belay sync --prefer markdown DEC-20260606T115000-001-sqlite
  belay sync --prefer sqlite DEC-20260606T115000-001-sqlite

Exit Status:
  0  Sync completed
  2  Invalid invocation
  3  Repository not initialized
  4  Invalid Markdown, path, duplicate ID, or other validation error
  5  Conflict requiring explicit resolution
  6  Storage failure

Related Commands:
  `belay doctor`, `belay rebuild`, and `belay show`."#;

const REBUILD_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Validates every managed Markdown entry before creating a temporary replacement
  database. Restores entries, links, chunks, FTS, and fresh sync baselines. The
  active database is replaced only after the temporary database is complete.

Examples:
  belay rebuild

Exit Status:
  0  Database rebuilt
  2  Invalid invocation
  3  Repository not initialized
  4  Mirror, relationship, or schema validation error
  6  Filesystem or SQLite failure

Related Commands:
  `belay sync` and `belay doctor`."#;

const EXPORT_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Writes a filtered point-in-time artifact outside the managed Markdown mirror.
  Markdown output is a combined human-readable snapshot; JSON is one document
  with an entries array; NDJSON writes one entry per line. Export reads SQLite
  and never changes entries, mirrors, revisions, indexes, or sync baselines.

Examples:
  belay export markdown --output ./artifacts/belay-export.md
  belay export json --type decision --status accepted --output ./artifacts/decisions.json
  belay export ndjson --tag release --output ./artifacts/release.ndjson

Exit Status:
  0  Export written
  2  Invalid invocation
  3  Repository not initialized
  4  Filter, display ID, or destination validation error
  6  Filesystem or SQLite failure

Related Commands:
  `belay search`, `belay show`, and `belay rebuild`."#;

const DOCTOR_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Performs read-only checks for configuration, current SQLite schema, FTS5/BM25,
  managed Markdown validity, duplicate IDs, path rules, sync drift, orphaned
  temporary files, and generated or active agent integration.

Agent Integration States:
  generated present  Canonical artifact exists; activation is not implied
  inactive           Optional repository target is absent
  active             Repository target exactly matches canonical content
  stale              File differs; refresh it with the reported init command
  malformed          AGENTS.md marker structure is unsafe and must be repaired

  These are repository observations. Doctor does not claim that an agent runtime
  loaded the Skill or can execute its commands.

Examples:
  belay doctor
  belay init
  belay init --update-agents
  belay init --install-skill codex
  belay init --install-skill claude

Exit Status:
  0  Checks passed
  2  Invalid invocation
  3  Repository not initialized
  4  Validation failure
  5  Drift requiring explicit resolution
  6  Filesystem, SQLite, or runtime capability failure

Related Commands:
  `belay init`, `belay sync`, and `belay rebuild`."#;

const BROWSE_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Copies the current SQLite generation into an in-memory snapshot and serves a
  read-only UI on 127.0.0.1. It never edits entries, Evidence, sync baselines, or
  repository files. Reload atomically replaces the snapshot only after success.

Examples:
  belay browse
  belay browse --port 8765 --open

Exit Status:
  0  Server stopped normally
  2  Invalid invocation
  3  Repository not initialized
  6  Bind, snapshot, browser runtime, or server failure

Related Commands:
  `belay search`, `belay show`, and `belay doctor`."#;

const ROUTE_AFTER_HELP: &str = r#"Behavior and Side Effects:
  Manages one CLI-first decision-preparation run under ignored local state at
  .belay/state/route/<run-id>/. Route Input and validation are deterministic.
  Assessment and Proposal are produced by an external AI and remain advisory.
  `preview` has no trace side effects. `apply` requires the exact preview hash
  and materializes only operations selected by an accepted Human Response.
  `pending` reports the one current, fresh Preview an agent may present for
  conversational approval; it does not authenticate or interpret chat.

Examples:
  belay route start --seed GOAL-20260701T090000-001-reliable-sync
  belay route template ROUTE-... assessment
  belay route submit ROUTE-... assessment --file ./assessment.json
  belay route submit ROUTE-... proposal --file ./proposal.json
  belay route submit ROUTE-... response --file ./response.json
  belay route preview ROUTE-...
  belay route pending ROUTE-...
  belay route apply ROUTE-... --approve <preview-sha256>
  belay route status ROUTE-...

Exit Status:
  0  Route operation completed
  2  Invalid invocation
  3  Repository not initialized
  4  Schema, reference, authority, revision, or approval validation failed
  5  Route Input is stale or managed trace state has drift
  6  Filesystem, SQLite, or partial materialization failure

Related Commands:
  `belay context compile`, `belay show`, `belay sync`, and `belay doctor`."#;

#[derive(Debug, Parser)]
#[command(
    name = "belay",
    version,
    about = "Local-first trace and context harness",
    long_about = TOP_LEVEL_ABOUT,
    after_help = TOP_LEVEL_AFTER_HELP
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(
        about = "Initialize belay-trace in the current repository",
        after_help = INIT_AFTER_HELP
    )]
    Init(InitArgs),

    #[command(
        about = "Create a trace entry",
        after_help = ADD_AFTER_HELP
    )]
    Add(AddArgs),

    #[command(
        about = "Link two trace entries",
        after_help = LINK_AFTER_HELP
    )]
    Link(LinkArgs),

    #[command(
        about = "Transition an entry status",
        after_help = STATUS_AFTER_HELP
    )]
    Status(StatusArgs),

    #[command(
        about = "Display a trace entry",
        after_help = SHOW_AFTER_HELP
    )]
    Show(ShowArgs),

    #[command(
        about = "Search trace entries",
        after_help = SEARCH_AFTER_HELP
    )]
    Search(SearchArgs),

    #[command(
        about = "Browse trace provenance on localhost",
        after_help = BROWSE_AFTER_HELP
    )]
    Browse(BrowseArgs),

    #[command(
        about = "Generate bounded task context",
        after_help = CONTEXT_AFTER_HELP
    )]
    Context(ContextArgs),

    #[command(
        about = "List deterministic archive candidates",
        after_help = ARCHIVE_AFTER_HELP
    )]
    Archive(ArchiveArgs),

    #[command(
        about = "Prepare and safely materialize a Route decision run",
        after_help = ROUTE_AFTER_HELP
    )]
    Route(RouteArgs),

    #[command(
        about = "Review and improve Goal entries",
        after_help = GOAL_AFTER_HELP
    )]
    Goal(GoalArgs),

    #[command(
        about = "Review Plan entries",
        after_help = PLAN_AFTER_HELP
    )]
    Plan(PlanArgs),

    #[command(
        about = "Record and inspect verification evidence",
        after_help = VERIFY_AFTER_HELP
    )]
    Verify(VerifyArgs),

    #[command(
        about = "Compute Goal Coverage",
        after_help = COVERAGE_AFTER_HELP
    )]
    Coverage(CoverageArgs),

    #[command(
        about = "Reconcile SQLite and managed Markdown",
        after_help = SYNC_AFTER_HELP
    )]
    Sync(SyncArgs),

    #[command(
        about = "Rebuild SQLite from managed Markdown",
        after_help = REBUILD_AFTER_HELP
    )]
    Rebuild,

    #[command(
        about = "Write a filtered external snapshot",
        after_help = EXPORT_AFTER_HELP
    )]
    Export(ExportArgs),

    #[command(
        about = "Check repository and agent integration health",
        after_help = DOCTOR_AFTER_HELP
    )]
    Doctor,
}

#[derive(Debug, Args)]
struct InitArgs {
    /// Add or refresh the marker-managed belay-trace section in repository AGENTS.md.
    #[arg(long)]
    update_agents: bool,

    /// Install generated agent skills into explicit repository-scoped targets. Repeat for multiple targets.
    #[arg(long, value_enum, value_name = "TARGET", action = clap::ArgAction::Append)]
    install_skill: Vec<SkillTarget>,

    /// Atomically rebuild local SQLite state from tracked Markdown after initialization.
    #[arg(long)]
    reset_state: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SkillTarget {
    Codex,
    Claude,
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("body_source")
        .required(false)
        .multiple(false)
        .args(["body", "body_file", "stdin"])
))]
struct AddArgs {
    /// Entry type: goal, plan, decision, work, review, or note.
    entry_type: String,

    /// Human-readable entry title.
    #[arg(long)]
    title: String,

    /// Inline Markdown body.
    #[arg(long, conflicts_with_all = ["body_file", "stdin"])]
    body: Option<String>,

    /// Read the Markdown body from this path.
    #[arg(long, value_name = "PATH", conflicts_with_all = ["body", "stdin"])]
    body_file: Option<PathBuf>,

    /// Read the Markdown body from standard input.
    #[arg(long, conflicts_with_all = ["body", "body_file"])]
    stdin: bool,
}

#[derive(Debug, Args)]
struct LinkArgs {
    /// Source display ID.
    from: String,

    /// Target display ID.
    to: String,

    /// Directional relation, such as implements or references.
    #[arg(long)]
    relation: String,
}

#[derive(Debug, Args)]
struct StatusArgs {
    /// Entry display ID.
    id: String,

    /// New type-specific status.
    status: String,
}

#[derive(Debug, Args)]
struct EntryIdArgs {
    /// Entry display ID.
    id: String,
}

#[derive(Debug, Args)]
struct ShowArgs {
    /// Entry display ID, unique prefix, or slug, optionally with `#sc-NNN` or `#t-NNN`.
    id: String,
}

#[derive(Debug, Args)]
struct SearchArgs {
    /// Plain-text keyword query or exact display ID.
    query: Option<String>,

    /// Filter by entry type.
    #[arg(long = "type", value_name = "TYPE")]
    entry_type: Option<String>,

    /// Include entries with this status. Repeatable.
    #[arg(long, value_name = "STATUS")]
    status: Vec<String>,

    /// Exclude entries with this status. Repeatable.
    #[arg(long = "exclude-status", value_name = "STATUS")]
    exclude_status: Vec<String>,

    /// Include archived entries in results. `--status archived` also includes them.
    #[arg(long)]
    include_archived: bool,

    /// Filter by an exact tag.
    #[arg(long)]
    tag: Option<String>,

    /// Resolve or filter by an exact display ID.
    #[arg(long, conflicts_with = "query")]
    id: Option<String>,

    /// Maximum number of deduplicated entries to return.
    #[arg(long, default_value_t = 20)]
    limit: usize,
}

#[derive(Debug, Args)]
struct BrowseArgs {
    /// Fixed localhost port. Zero lets the OS choose an available port.
    #[arg(long, default_value_t = 0)]
    port: u16,

    /// Open the assigned localhost URL in the default browser.
    #[arg(long)]
    open: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliContextFormat {
    Human,
    Agent,
}

#[derive(Debug, Args)]
struct ContextArgs {
    /// Task to retrieve context for. Omit for the live working set. Use
    /// `compile <task>` for the Phase 5 compiler.
    #[arg(num_args = 0..=2)]
    task: Vec<String>,

    /// Output format for a human or an AI agent.
    #[arg(long, value_enum, default_value_t = CliContextFormat::Human)]
    format: CliContextFormat,

    /// Approximate output token budget.
    #[arg(long, default_value_t = 2500)]
    budget: usize,

    /// Explicit seed entry for context compile. Unique prefix or slug is allowed.
    #[arg(long = "seed")]
    seeds: Vec<String>,

    /// Compile a task packet for one `#t-nnn` or `#sc-nnn` fragment.
    #[arg(long)]
    focus: Option<String>,

    /// Include archived entries in ranked context and compile sections.
    #[arg(long)]
    include_archived: bool,
}

#[derive(Debug, Args)]
struct ArchiveArgs {
    #[command(subcommand)]
    command: ArchiveCommand,
}

#[derive(Debug, Subcommand)]
enum ArchiveCommand {
    #[command(about = "List deterministic archive candidates without changing status")]
    Candidates,
}

#[derive(Debug, Args)]
struct RouteArgs {
    #[command(subcommand)]
    command: RouteCommand,
}

#[derive(Debug, Subcommand)]
enum RouteCommand {
    #[command(about = "Create a deterministic Route Input from a primary Belay seed")]
    Start(RouteStartArgs),
    #[command(about = "Validate and store an external Route artifact")]
    Submit(RouteSubmitArgs),
    #[command(about = "Print a hash-bound JSON template for the next external artifact")]
    Template(RouteTemplateArgs),
    #[command(about = "Create a side-effect-free materialization preview")]
    Preview(RouteRunArgs),
    #[command(about = "Show the one current fresh Preview for conversational approval")]
    Pending(RouteRunArgs),
    #[command(about = "Materialize the exact explicitly approved preview")]
    Apply(RouteApplyArgs),
    #[command(about = "Show the current Route run manifest")]
    Status(RouteRunArgs),
}

#[derive(Debug, Args)]
struct RouteStartArgs {
    #[arg(long)]
    seed: String,

    #[arg(long = "include")]
    includes: Vec<String>,
}

#[derive(Debug, Args)]
struct RouteSubmitArgs {
    run_id: String,
    #[arg(value_enum)]
    kind: RouteSubmitKind,
    #[arg(long, value_name = "PATH")]
    file: PathBuf,
}

#[derive(Debug, Args)]
struct RouteTemplateArgs {
    run_id: String,
    #[arg(value_enum)]
    kind: RouteSubmitKind,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum RouteSubmitKind {
    Assessment,
    Proposal,
    Response,
}

#[derive(Debug, Args)]
struct RouteRunArgs {
    run_id: String,
}

#[derive(Debug, Args)]
struct RouteApplyArgs {
    run_id: String,
    #[arg(long, value_name = "PREVIEW_SHA256")]
    approve: String,
}

#[derive(Debug, Args)]
struct GoalArgs {
    #[command(subcommand)]
    command: GoalCommand,
}

#[derive(Debug, Subcommand)]
enum GoalCommand {
    Lint(GoalLintArgs),
    Improve(GoalImproveArgs),
}

#[derive(Debug, Args)]
struct PlanArgs {
    #[command(subcommand)]
    command: PlanCommand,
}

#[derive(Debug, Subcommand)]
enum PlanCommand {
    Lint(PlanLintArgs),
}

#[derive(Debug, Args)]
struct PlanLintArgs {
    id: Option<String>,

    #[arg(long)]
    all: bool,

    #[arg(long, value_enum, default_value_t = CliReportFormat::Human)]
    format: CliReportFormat,

    #[arg(long)]
    strict: bool,
}

#[derive(Debug, Args)]
struct GoalLintArgs {
    id: Option<String>,

    #[arg(long)]
    all: bool,

    #[arg(long, value_enum, default_value_t = CliReportFormat::Human)]
    format: CliReportFormat,

    #[arg(long)]
    strict: bool,

    #[arg(long)]
    score: bool,
}

#[derive(Debug, Args)]
struct GoalImproveArgs {
    id: String,

    #[arg(long, default_value_t = 3000)]
    budget: usize,
}

#[derive(Debug, Args)]
struct VerifyArgs {
    #[command(subcommand)]
    command: VerifyCommand,
}

#[derive(Debug, Subcommand)]
enum VerifyCommand {
    Record(VerifyRecordArgs),
    Import(VerifyImportArgs),
    Status(EntryIdArgs),
}

#[derive(Debug, Args)]
struct VerifyRecordArgs {
    #[arg(long)]
    kind: String,

    #[arg(long)]
    verdict: String,

    #[arg(long)]
    source: String,

    #[arg(long, default_value = "local:user")]
    issuer: String,

    #[arg(long)]
    summary: String,

    #[arg(long)]
    commit: Option<String>,

    #[arg(long = "captured-at")]
    captured_at: Option<String>,

    #[arg(long = "verifies")]
    verifies: Vec<String>,
}

#[derive(Debug, Args)]
struct VerifyImportArgs {
    #[arg(long)]
    junit: PathBuf,

    #[arg(long = "verifies")]
    verifies: Vec<String>,
}

#[derive(Debug, Args)]
struct CoverageArgs {
    id: Option<String>,

    #[arg(long, value_enum, default_value_t = CliReportFormat::Human)]
    format: CliReportFormat,

    #[arg(long = "include-completed")]
    include_completed: bool,

    #[arg(long = "fail-under")]
    fail_under: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliReportFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SyncPreference {
    Markdown,
    Sqlite,
}

#[derive(Debug, Args)]
struct SyncArgs {
    /// Explicitly keep one side of an entry-scoped conflict.
    #[arg(long, value_enum, requires = "id")]
    prefer: Option<SyncPreference>,

    /// Optional display ID to synchronize or resolve.
    id: Option<String>,
}

#[derive(Debug, Args)]
struct ExportArgs {
    /// Output format: markdown, json, or ndjson.
    #[arg(value_enum)]
    format: CliExportFormat,

    /// Destination path for the external snapshot.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,

    /// Filter by entry type.
    #[arg(long = "type", value_name = "TYPE")]
    entry_type: Option<String>,

    /// Filter by type-specific status.
    #[arg(long)]
    status: Option<String>,

    /// Filter by an exact tag.
    #[arg(long)]
    tag: Option<String>,

    /// Filter by an exact display ID.
    #[arg(long)]
    id: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliExportFormat {
    Markdown,
    Json,
    Ndjson,
}

pub fn run() -> Result<(), BelayError> {
    let cli = Cli::parse();
    let current_dir = env::current_dir()
        .map_err(|source| BelayError::io("read current directory", ".", source))?;
    execute(cli, &current_dir)
}

pub fn try_parse_from<I, T>(arguments: I) -> Result<Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    Cli::try_parse_from(arguments)
}

fn execute(cli: Cli, current_dir: &Path) -> Result<(), BelayError> {
    match cli.command {
        Command::Init(arguments) => {
            let outcome = repository::initialize(current_dir)?;
            if outcome.already_initialized {
                println!(
                    "belay-trace is initialized at {}",
                    outcome.repository.root.display()
                );
            } else {
                println!(
                    "Initialized belay-trace at {}",
                    outcome.repository.root.display()
                );
            }
            if arguments.update_agents {
                let activation = agent::update_agents(&outcome.repository)?;
                println!(
                    "AGENTS.md integration {}: {}",
                    activation.status.as_str(),
                    activation.path.display()
                );
            }
            if arguments.reset_state {
                let count = reconcile::rebuild(&outcome.repository)?;
                println!("Rebuilt local state from {count} Markdown entries");
            }
            for target in arguments.install_skill {
                let (name, activation) = match target {
                    SkillTarget::Codex => {
                        ("Codex", agent::install_codex_skill(&outcome.repository)?)
                    }
                    SkillTarget::Claude => {
                        ("Claude", agent::install_claude_skill(&outcome.repository)?)
                    }
                };
                println!(
                    "{name} skill {}: {}",
                    activation.status.as_str(),
                    activation.path.display()
                );
            }
            Ok(())
        }
        Command::Add(arguments) => {
            let repository = repository::discover(current_dir)?;
            let entry_type = EntryType::from_str(&arguments.entry_type)?;
            let body = read_body(&arguments, entry_type)?;
            let entry = store::create(&repository, entry_type, arguments.title, body)?;
            println!("Created {}", entry.display_id);
            Ok(())
        }
        Command::Link(arguments) => {
            let repository = repository::discover(current_dir)?;
            let relation = LinkRelation::from_str(&arguments.relation)?;
            let outcome = store::link(&repository, &arguments.from, &arguments.to, relation)?;
            match outcome {
                MutationOutcome::Changed => {
                    println!("Linked {} {} {}", arguments.from, relation, arguments.to)
                }
                MutationOutcome::Unchanged => println!(
                    "Link already exists: {} {} {}",
                    arguments.from, relation, arguments.to
                ),
            }
            Ok(())
        }
        Command::Status(arguments) => {
            let repository = repository::discover(current_dir)?;
            let status = EntryStatus::from_str(&arguments.status)?;
            let outcome = store::set_status(&repository, &arguments.id, status)?;
            match outcome {
                MutationOutcome::Changed => {
                    println!("Updated {} status to {}", arguments.id, status)
                }
                MutationOutcome::Unchanged => {
                    println!("{} already has status {}", arguments.id, status)
                }
            }
            Ok(())
        }
        Command::Show(arguments) => {
            let repository = repository::discover(current_dir)?;
            let shown = store::show(&repository, &arguments.id)?;
            print_shown_entry(&shown);
            Ok(())
        }
        Command::Search(arguments) => {
            let repository = repository::discover(current_dir)?;
            let entry_type = arguments
                .entry_type
                .as_deref()
                .map(EntryType::from_str)
                .transpose()?;
            let status_include = arguments
                .status
                .into_iter()
                .map(|value| EntryStatus::from_str(&value))
                .collect::<Result<Vec<_>, _>>()?;
            let mut status_exclude = arguments
                .exclude_status
                .into_iter()
                .map(|value| EntryStatus::from_str(&value))
                .collect::<Result<Vec<_>, _>>()?;
            let display_id = match arguments.id {
                Some(id) => Some(store::resolve_reference(&repository, &id)?.display_id),
                None => None,
            };
            search::apply_default_archived_exclude(
                &status_include,
                &mut status_exclude,
                arguments.include_archived,
            );
            let request = SearchRequest {
                query: arguments.query.unwrap_or_default(),
                entry_type,
                status_include,
                status_exclude,
                tag: arguments.tag,
                display_id,
                limit: arguments.limit,
            };
            let results = search::search(&repository, &request)?;
            print_search_results(&request, &results);
            Ok(())
        }
        Command::Browse(arguments) => {
            let repository = repository::discover(current_dir)?;
            browse::run(
                &repository,
                BrowseOptions {
                    port: arguments.port,
                    open: arguments.open,
                },
            )
        }
        Command::Context(arguments) => {
            let repository = repository::discover(current_dir)?;
            let format = match arguments.format {
                CliContextFormat::Human => context::ContextFormat::Human,
                CliContextFormat::Agent => context::ContextFormat::Agent,
            };
            let (compile, task) = parse_context_task(&arguments.task)?;
            let bundle = if let Some(focus) = arguments.focus.as_deref() {
                context::compile_focus(&repository, focus, format, arguments.budget)?
            } else if compile {
                context::compile(
                    &repository,
                    task,
                    format,
                    arguments.budget,
                    &arguments.seeds,
                    arguments.include_archived,
                )?
            } else if task.is_empty() {
                context::compile_working_set(
                    &repository,
                    format,
                    arguments.budget,
                    arguments.include_archived,
                )?
            } else {
                context::generate(
                    &repository,
                    task,
                    format,
                    arguments.budget,
                    arguments.include_archived,
                )?
            };
            print!("{}", bundle.text);
            Ok(())
        }
        Command::Archive(arguments) => {
            let repository = repository::discover(current_dir)?;
            match arguments.command {
                ArchiveCommand::Candidates => {
                    let candidates = archive::candidates(&repository)?;
                    print!("{}", archive::render_candidates(&candidates));
                    Ok(())
                }
            }
        }
        Command::Route(arguments) => {
            let repository = repository::discover(current_dir)?;
            match arguments.command {
                RouteCommand::Start(arguments) => {
                    let outcome = route::start(&repository, &arguments.seed, &arguments.includes)?;
                    println!("Created Route run {}", outcome.run_id);
                    println!("Run path: {}", outcome.run_path.display());
                    println!("Input: {}", outcome.input_path.display());
                    println!("Input fingerprint: {}", outcome.fingerprint);
                    Ok(())
                }
                RouteCommand::Submit(arguments) => {
                    let kind = match arguments.kind {
                        RouteSubmitKind::Assessment => route::SubmitKind::Assessment,
                        RouteSubmitKind::Proposal => route::SubmitKind::Proposal,
                        RouteSubmitKind::Response => route::SubmitKind::Response,
                    };
                    let outcome =
                        route::submit(&repository, &arguments.run_id, kind, &arguments.file)?;
                    println!(
                        "Stored Route artifact revision {} at {}",
                        outcome.revision,
                        outcome.artifact_path.display()
                    );
                    println!("Artifact hash: {}", outcome.artifact_hash);
                    Ok(())
                }
                RouteCommand::Template(arguments) => {
                    let kind = match arguments.kind {
                        RouteSubmitKind::Assessment => route::TemplateKind::Assessment,
                        RouteSubmitKind::Proposal => route::TemplateKind::Proposal,
                        RouteSubmitKind::Response => route::TemplateKind::Response,
                    };
                    let template = route::template(&repository, &arguments.run_id, kind)?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&template).map_err(|error| {
                            BelayError::Validation {
                                message: format!(
                                    "Route template JSON serialization failed: {error}"
                                ),
                            }
                        })?
                    );
                    Ok(())
                }
                RouteCommand::Preview(arguments) => {
                    let outcome = route::preview(&repository, &arguments.run_id)?;
                    println!(
                        "Created Materialization Preview at {}",
                        outcome.preview_path.display()
                    );
                    println!("Pending preview revision: {}", outcome.revision);
                    println!("Approve exact preview hash: {}", outcome.preview_hash);
                    Ok(())
                }
                RouteCommand::Pending(arguments) => {
                    let pending = route::pending(&repository, &arguments.run_id)?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&pending).map_err(|error| {
                            BelayError::Validation {
                                message: format!(
                                    "Route pending Preview JSON serialization failed: {error}"
                                ),
                            }
                        })?
                    );
                    Ok(())
                }
                RouteCommand::Apply(arguments) => {
                    let outcome = route::apply(&repository, &arguments.run_id, &arguments.approve)?;
                    println!(
                        "Recorded Reconciliation Result at {}",
                        outcome.result_path.display()
                    );
                    for operation in &outcome.result.operations {
                        println!(
                            "{}: {:?}{}",
                            operation.operation_id,
                            operation.state,
                            operation
                                .target
                                .as_ref()
                                .map(|target| format!(" ({target})"))
                                .unwrap_or_default()
                        );
                    }
                    if outcome.has_failures() {
                        Err(BelayError::StorageSummary {
                            message:
                                "Route materialization completed with failed operations; inspect the Reconciliation Result and retry safely"
                                    .to_owned(),
                        })
                    } else {
                        Ok(())
                    }
                }
                RouteCommand::Status(arguments) => {
                    let manifest = route::status(&repository, &arguments.run_id)?;
                    print!(
                        "{}",
                        serde_json::to_string_pretty(&manifest).map_err(|error| {
                            BelayError::Validation {
                                message: format!(
                                    "Route manifest JSON serialization failed: {error}"
                                ),
                            }
                        })?
                    );
                    println!();
                    Ok(())
                }
            }
        }
        Command::Goal(arguments) => {
            let repository = repository::discover(current_dir)?;
            match arguments.command {
                GoalCommand::Lint(arguments) => {
                    if arguments.all && arguments.id.is_some() {
                        return Err(BelayError::Validation {
                            message:
                                "`belay goal lint` accepts either a goal ID or --all, not both"
                                    .to_owned(),
                        });
                    }
                    let reports = goal::lint(&repository, arguments.id.as_deref(), arguments.all)?;
                    let format = match arguments.format {
                        CliReportFormat::Human => goal::GoalLintFormat::Human,
                        CliReportFormat::Json => goal::GoalLintFormat::Json,
                    };
                    print!("{}", goal::render_lint(&reports, format, arguments.score)?);
                    if arguments.strict
                        && reports
                            .iter()
                            .any(goal::GoalLintReport::has_strict_findings)
                    {
                        Err(BelayError::Validation {
                            message: "goal lint strict mode found deterministic findings"
                                .to_owned(),
                        })
                    } else {
                        Ok(())
                    }
                }
                GoalCommand::Improve(arguments) => {
                    print!(
                        "{}",
                        goal::improve(&repository, &arguments.id, arguments.budget)?
                    );
                    Ok(())
                }
            }
        }
        Command::Plan(arguments) => {
            let repository = repository::discover(current_dir)?;
            match arguments.command {
                PlanCommand::Lint(arguments) => {
                    if arguments.all && arguments.id.is_some() {
                        return Err(BelayError::Validation {
                            message:
                                "`belay plan lint` accepts either a plan ID or --all, not both"
                                    .to_owned(),
                        });
                    }
                    let reports = plan::lint(&repository, arguments.id.as_deref(), arguments.all)?;
                    let format = match arguments.format {
                        CliReportFormat::Human => plan::PlanLintFormat::Human,
                        CliReportFormat::Json => plan::PlanLintFormat::Json,
                    };
                    print!("{}", plan::render_lint(&reports, format)?);
                    if arguments.strict
                        && reports
                            .iter()
                            .any(plan::PlanLintReport::has_strict_findings)
                    {
                        Err(BelayError::Validation {
                            message: "plan lint strict mode found deterministic findings"
                                .to_owned(),
                        })
                    } else {
                        Ok(())
                    }
                }
            }
        }
        Command::Verify(arguments) => {
            let repository = repository::discover(current_dir)?;
            match arguments.command {
                VerifyCommand::Record(arguments) => {
                    let record = evidence::record(
                        &repository,
                        evidence::RecordInput {
                            kind: arguments.kind,
                            verdict: arguments.verdict,
                            commit_sha: arguments.commit,
                            captured_at: arguments.captured_at,
                            source: arguments.source,
                            issuer: arguments.issuer,
                            summary: arguments.summary,
                            detail: serde_json::json!({}),
                            verifies: arguments.verifies,
                        },
                    )?;
                    println!("Recorded {}", record.display_id);
                    Ok(())
                }
                VerifyCommand::Import(arguments) => {
                    let record =
                        evidence::import_junit(&repository, &arguments.junit, arguments.verifies)?;
                    println!("Imported {}", record.display_id);
                    Ok(())
                }
                VerifyCommand::Status(arguments) => {
                    let status = evidence::status(&repository, &arguments.id)?;
                    print!("{}", evidence::render_status(&status));
                    Ok(())
                }
            }
        }
        Command::Coverage(arguments) => {
            let repository = repository::discover(current_dir)?;
            let report = coverage::report(
                &repository,
                arguments.id.as_deref(),
                arguments.include_completed,
            )?;
            let format = match arguments.format {
                CliReportFormat::Human => coverage::CoverageFormat::Human,
                CliReportFormat::Json => coverage::CoverageFormat::Json,
            };
            print!("{}", coverage::render(&report, format)?);
            let threshold = arguments
                .fail_under
                .as_deref()
                .map(parse_fail_under)
                .transpose()?;
            if coverage::fails_under(&report, threshold) {
                Err(BelayError::Validation {
                    message: "goal coverage is below requested threshold".to_owned(),
                })
            } else {
                Ok(())
            }
        }
        Command::Sync(arguments) => {
            let repository = repository::discover(current_dir)?;
            let preference = arguments.prefer.map(|preference| match preference {
                SyncPreference::Markdown => reconcile::SyncPreference::Markdown,
                SyncPreference::Sqlite => reconcile::SyncPreference::Sqlite,
            });
            let report = reconcile::synchronize(&repository, arguments.id.as_deref(), preference)?;
            let unchanged = report
                .outcomes
                .iter()
                .filter(|outcome| outcome.action == "unchanged")
                .count();
            for outcome in report
                .outcomes
                .iter()
                .filter(|outcome| outcome.action != "unchanged")
            {
                println!("{}: {}", outcome.display_id, outcome.action);
            }
            for failure in &report.failures {
                eprintln!("{}: {}", failure.subject, failure.message);
            }
            if report.failures.is_empty() {
                println!(
                    "Sync completed: {} entries ({} unchanged)",
                    report.outcomes.len(),
                    unchanged
                );
                Ok(())
            } else if report.failures.iter().any(|failure| failure.exit_code == 6) {
                Err(BelayError::StorageSummary {
                    message: format!(
                        "sync completed with {} failure(s), including a storage failure",
                        report.failures.len()
                    ),
                })
            } else if report.failures.iter().any(|failure| failure.exit_code == 4) {
                Err(BelayError::Validation {
                    message: format!(
                        "sync completed with {} validation failure(s)",
                        report.failures.len()
                    ),
                })
            } else {
                Err(BelayError::Conflict {
                    message: format!(
                        "sync completed with {} conflict or drift failure(s)",
                        report.failures.len()
                    ),
                })
            }
        }
        Command::Rebuild => {
            let repository = repository::discover(current_dir)?;
            let count = reconcile::rebuild(&repository)?;
            println!("Rebuilt SQLite from {count} managed Markdown entries");
            Ok(())
        }
        Command::Export(arguments) => {
            let repository = repository::discover(current_dir)?;
            let format = match arguments.format {
                CliExportFormat::Markdown => export::ExportFormat::Markdown,
                CliExportFormat::Json => export::ExportFormat::Json,
                CliExportFormat::Ndjson => export::ExportFormat::Ndjson,
            };
            let filter = export::ExportFilter {
                entry_type: arguments
                    .entry_type
                    .as_deref()
                    .map(EntryType::from_str)
                    .transpose()?,
                status: arguments
                    .status
                    .as_deref()
                    .map(EntryStatus::from_str)
                    .transpose()?,
                tag: arguments.tag,
                display_id: arguments.id,
            };
            let count = export::write(&repository, format, &arguments.output, &filter)?;
            println!(
                "Exported {count} entries to {}",
                if arguments.output.is_absolute() {
                    arguments.output
                } else {
                    repository.root.join(arguments.output)
                }
                .display()
            );
            Ok(())
        }
        Command::Doctor => {
            let repository = repository::discover(current_dir)?;
            let report = reconcile::doctor(&repository);
            println!("Repository health");
            for check in &report.checks {
                println!("{}: {} ({})", check.name, check.status, check.detail);
            }
            if report.has_invalid {
                Err(BelayError::Validation {
                    message: "repository health checks found invalid state; repair the marker pair or resolve the other reported validation errors before continuing"
                        .to_owned(),
                })
            } else if report.has_drift {
                Err(BelayError::Conflict {
                    message: "repository health checks found drift; run `belay sync`, `belay rebuild`, `belay init`, or the reported explicit integration command"
                        .to_owned(),
                })
            } else {
                Ok(())
            }
        }
    }
}

fn print_search_results(request: &SearchRequest, results: &[search::SearchResult]) {
    println!("Search results");
    if !request.query.trim().is_empty() {
        println!("Query: {}", request.query.trim());
    }
    if let Some(display_id) = &request.display_id {
        println!("Display ID: {display_id}");
    }
    if let Some(entry_type) = request.entry_type {
        println!("Type: {entry_type}");
    }
    if !request.status_include.is_empty() {
        println!(
            "Status include: {}",
            request
                .status_include
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !request.status_exclude.is_empty() {
        println!(
            "Status exclude: {}",
            request
                .status_exclude
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if let Some(tag) = &request.tag {
        println!("Tag: {tag}");
    }
    println!("Matches: {}", results.len());
    for (index, result) in results.iter().enumerate() {
        println!();
        println!(
            "{}. {} [{} / {}]",
            index + 1,
            result.display_id,
            result.entry_type,
            result.status
        );
        println!("   Title: {}", result.title);
        if let Some(score) = result.score {
            println!("   BM25 score: {score:.6}");
        }
        println!("   Why: {}", result.reason);
        println!("   Section: {}", result.section);
        println!("   Excerpt: {}", result.excerpt);
        println!("   Matching chunks: {}", result.match_count);
        println!("   Source: {}", result.source_path);
        println!("   Next: belay show {}", result.display_id);
    }
}

fn read_body(arguments: &AddArgs, entry_type: EntryType) -> Result<String, BelayError> {
    if let Some(body) = &arguments.body {
        return Ok(body.clone());
    }
    if let Some(path) = &arguments.body_file {
        return fs::read_to_string(path)
            .map_err(|source| BelayError::io("read body file", path, source));
    }
    if arguments.stdin {
        let mut body = String::new();
        io::stdin()
            .read_to_string(&mut body)
            .map_err(|source| BelayError::io("read standard input", "stdin", source))?;
        return Ok(body);
    }
    if entry_type == EntryType::Goal {
        return Ok(goal::template());
    }
    Err(BelayError::Validation {
        message: "entry body is required; pass --body, --body-file, or --stdin".to_owned(),
    })
}

fn parse_context_task(values: &[String]) -> Result<(bool, &str), BelayError> {
    match values {
        [] => Ok((true, "")),
        [command] if command == "compile" => Ok((true, "")),
        [task] => Ok((false, task.as_str())),
        [command, task] if command == "compile" => Ok((true, task.as_str())),
        [command, _] => Err(BelayError::Validation {
            message: format!("unsupported context subcommand {command:?}; expected compile"),
        }),
        _ => Err(BelayError::Validation {
            message: "context requires a task".to_owned(),
        }),
    }
}

fn parse_fail_under(value: &str) -> Result<(String, usize), BelayError> {
    let Some((kind, threshold)) = value.split_once('=') else {
        return Err(BelayError::Validation {
            message: "--fail-under must be formatted as verified=N or traceability=N".to_owned(),
        });
    };
    if kind != "verified" && kind != "traceability" {
        return Err(BelayError::Validation {
            message: "--fail-under kind must be verified or traceability".to_owned(),
        });
    }
    let threshold = threshold
        .parse::<usize>()
        .ok()
        .filter(|value| *value <= 100)
        .ok_or_else(|| BelayError::Validation {
            message: "--fail-under threshold must be an integer from 0 to 100".to_owned(),
        })?;
    Ok((kind.to_owned(), threshold))
}

fn print_shown_entry(shown: &store::ShownEntry) {
    let entry = &shown.entry;
    if let Some(fragment) = &shown.fragment {
        print_shown_fragment(shown, fragment);
        return;
    }
    println!("ID: {}", entry.display_id);
    println!("Type: {}", entry.entry_type);
    println!("Title: {}", entry.title);
    println!("Status: {}", entry.status);
    println!("Created: {}", entry.created_at);
    println!("Updated: {}", entry.updated_at);
    println!("Revision: {}", entry.revision);
    println!("Source: {}", shown.source_path);
    if entry.tags.is_empty() {
        println!("Tags: none");
    } else {
        println!("Tags: {}", entry.tags.join(", "));
    }
    let metadata = serde_json::to_string(&entry.metadata).unwrap_or_else(|_| "{}".to_owned());
    println!("Metadata: {metadata}");
    println!("Outbound Links:");
    if entry.links.is_empty() {
        println!("  none");
    } else {
        for link in &entry.links {
            println!("  - {} {}", link.relation, link.id);
        }
    }
    println!("Inbound Links:");
    if shown.inbound_links.is_empty() {
        println!("  none");
    } else {
        for link in &shown.inbound_links {
            println!("  - {} {}", link.id, link.relation);
        }
    }
    println!("Body:");
    println!("{}", entry.body);
}

/// One item's view. Entry-scoped fields that would reintroduce the cost the
/// fragment exists to avoid — tags, metadata, links, and the full body — are
/// left out; `Source` stays so a reader can still fall back to the file.
fn print_shown_fragment(shown: &store::ShownEntry, fragment: &store::ShownFragment) {
    let entry = &shown.entry;
    println!("ID: {}#{}", entry.display_id, fragment.fragment);
    println!("Type: {}", entry.entry_type);
    println!("Title: {}", entry.title);
    println!("Status: {}", entry.status);
    println!("Source: {}", shown.source_path);
    println!("Definition:");
    println!("{}", fragment.definition);
    match &fragment.section {
        Some(section) => {
            println!("Section:");
            println!("{section}");
        }
        None => println!(
            "Section: none (no body section is headed {})",
            fragment.fragment.to_ascii_uppercase()
        ),
    }
}

#[cfg(test)]
mod tests {
    use clap::error::ErrorKind;

    use super::*;

    #[test]
    fn invalid_invocation_uses_clap_usage_exit_category() {
        let error = try_parse_from(["belay", "add", "decision"]).expect_err("title is required");
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn add_requires_exactly_one_body_source() {
        try_parse_from(["belay", "add", "goal", "--title", "Reliable sync"])
            .expect("goal body source is optional because a template is generated");

        let duplicate = try_parse_from([
            "belay", "add", "decision", "--title", "SQLite", "--body", "inline", "--stdin",
        ])
        .expect_err("body sources conflict");
        assert_eq!(duplicate.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn sync_id_and_preference_are_coupled() {
        try_parse_from(["belay", "sync", "DEC-20260606T115000-001-sqlite"])
            .expect("ID without preference is a targeted sync");

        let preference_without_id = try_parse_from(["belay", "sync", "--prefer", "sqlite"])
            .expect_err("--prefer requires an ID");
        assert_eq!(
            preference_without_id.kind(),
            ErrorKind::MissingRequiredArgument
        );
    }
}
