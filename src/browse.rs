//! Read-only localhost browser for one atomic generation of Belay trace data.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{Cursor, Write};
use std::net::Ipv4Addr;
use std::path::Path;
use std::time::Duration;

use ammonia::Builder;
use chrono::{DateTime, Local, SecondsFormat, Utc};
use pulldown_cmark::{CowStr, Event, LinkType, Options, Parser, Tag, TagEnd, html};
use rusqlite::backup::Backup;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::entry::{EntryStatus, EntryType};
use crate::error::BelayError;
use crate::repository::Repository;
use crate::search::{self, SearchRequest};

const CSS: &str = include_str!("browse.css");
const JS: &str = include_str!("browse.js");
const CYTOSCAPE: &[u8] = include_bytes!("../vendor/cytoscape-3.34.0/cytoscape.min.js");
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_EVIDENCE: usize = 250;
const MAX_GRAPH_NEIGHBORS: usize = 250;
const MAX_GIT_FRESHNESS_COMMITS: usize = 64;

#[derive(Debug, Clone, Copy)]
pub struct BrowseOptions {
    pub port: u16,
    pub open: bool,
}

pub fn run(repository: &Repository, options: BrowseOptions) -> Result<(), BelayError> {
    let mut state = BrowseState::new(repository.clone())?;
    let server = Server::http((Ipv4Addr::LOCALHOST, options.port)).map_err(|source| {
        BelayError::StorageSummary {
            message: format!("could not bind localhost Browse server: {source}"),
        }
    })?;
    let port = match server.server_addr().to_ip() {
        Some(address) => address.port(),
        None => {
            return Err(BelayError::StorageSummary {
                message: "Browse server did not bind an IP address".to_owned(),
            });
        }
    };
    state.port = port;
    let url = format!("http://127.0.0.1:{port}/");
    println!("Belay Browse: {url}");
    std::io::stdout()
        .flush()
        .map_err(|source| BelayError::io("flush Browse URL", "stdout", source))?;
    if options.open {
        if let Err(error) = webbrowser::open(&url) {
            eprintln!(
                "warning: could not open the default browser ({error}); server continues at {url}"
            );
        }
    }
    for request in server.incoming_requests() {
        state.handle(request);
    }
    Ok(())
}

struct BrowseState {
    repository: Repository,
    snapshot: Snapshot,
    reload_nonce: String,
    last_reload_result: String,
    port: u16,
}

impl BrowseState {
    fn new(repository: Repository) -> Result<Self, BelayError> {
        let snapshot = Snapshot::build(&repository)?;
        let reload_nonce = nonce(&repository, &snapshot.generated_at);
        Ok(Self {
            repository,
            snapshot,
            reload_nonce,
            last_reload_result: "Reload has not run in this process.".to_owned(),
            port: 0,
        })
    }

    fn handle(&mut self, mut request: Request) {
        if !self.valid_host(&request) || !self.valid_origin(&request) {
            let response = self.response(
                StatusCode(403),
                "text/plain; charset=utf-8",
                b"Forbidden".to_vec(),
            );
            let _ = request.respond(response);
            return;
        }
        let method = request.method().clone();
        let raw_url = request.url().to_owned();
        let (path, query) = split_url(&raw_url);
        let response = match (method, path.as_str()) {
            (Method::Get, "/") => self.library(&query),
            (Method::Get, "/assets/app.css") => Ok(self.response(
                StatusCode(200),
                "text/css; charset=utf-8",
                CSS.as_bytes().to_vec(),
            )),
            (Method::Get, "/assets/app.js") => Ok(self.response(
                StatusCode(200),
                "application/javascript; charset=utf-8",
                JS.as_bytes().to_vec(),
            )),
            (Method::Get, "/assets/cytoscape.js") => Ok(self.response(
                StatusCode(200),
                "application/javascript; charset=utf-8",
                CYTOSCAPE.to_vec(),
            )),
            (Method::Get, "/explore") => {
                Ok(self.explore_page(query_value(&query, "focus").as_deref()))
            }
            (Method::Get, "/api/explore") => {
                self.explore_api(query_value(&query, "focus").as_deref())
            }
            (Method::Post, "/api/reload") => self.reload(&mut request),
            (Method::Get, _) if path.starts_with("/entries/") => self.entry_page(&path[9..]),
            (Method::Get, _) if path.starts_with("/evidence/") => self.evidence_page(&path[10..]),
            (Method::Get, _) if path.starts_with("/commits/") => self.commit_route(&path[9..]),
            _ => Ok(self.html_response(
                StatusCode(404),
                self.layout(
                    "Not found",
                    "<div class=\"panel error\"><h1>Not found</h1></div>",
                    None,
                ),
            )),
        };
        let response = response.unwrap_or_else(|error| {
            self.html_response(
                if error.exit_code() == 6 {
                    StatusCode(500)
                } else {
                    StatusCode(400)
                },
                self.layout(
                    "Browse error",
                    &format!(
                        "<div class=\"panel error\"><h1>Unavailable</h1><p>{}</p></div>",
                        escape(&error.to_string())
                    ),
                    None,
                ),
            )
        });
        let _ = request.respond(response);
    }

    fn library(&self, query: &str) -> Result<HttpResponse, BelayError> {
        let q = query_value(query, "q").unwrap_or_default();
        let type_filter = query_value(query, "type").filter(|value| !value.is_empty());
        let (status_include, status_exclude) = parse_library_status_filters(query)?;
        let searching = !q.is_empty()
            || type_filter.is_some()
            || !status_include.is_empty()
            || !status_exclude.is_empty();
        let results = if searching {
            let mut status_exclude = status_exclude.clone();
            search::apply_default_archived_exclude(&status_include, &mut status_exclude, false);
            search::search_connection(
                &self.snapshot.connection,
                &self.repository.database_path(),
                &SearchRequest {
                    query: q.clone(),
                    entry_type: type_filter.as_deref().map(str::parse).transpose()?,
                    status_include: status_include.clone(),
                    status_exclude,
                    tag: None,
                    display_id: None,
                    limit: 100,
                },
            )?
        } else {
            recent_entries(&self.snapshot.connection)?
        };
        let mut body = String::from(
            "<section class=\"panel\"><h1>Library</h1><form class=\"filters\" method=\"get\" action=\"/\"><label class=\"sr-only\" for=\"q\">Search</label>",
        );
        body.push_str(&format!(
            "<input id=\"q\" name=\"q\" value=\"{}\" placeholder=\"Search entries\">",
            escape(&q)
        ));
        body.push_str(&select(
            "type",
            type_filter.as_deref(),
            &["", "goal", "plan", "decision", "work", "review", "note"],
        ));
        let include_selected = status_include.first().map(ToString::to_string);
        let exclude_selected = status_exclude.first().map(ToString::to_string);
        body.push_str(&grouped_status_select(
            "status",
            "Any status",
            include_selected.as_deref(),
        ));
        body.push_str(&grouped_status_select(
            "status_exclude",
            "Exclude none",
            exclude_selected.as_deref(),
        ));
        body.push_str("<button type=\"submit\">Search</button></form></section>");
        body.push_str(if searching {
            "<h2>Search results</h2>"
        } else {
            "<h2>Recently updated</h2>"
        });
        if results.is_empty() {
            body.push_str("<p>No entries matched.</p>");
        }
        for result in results {
            body.push_str(&format!(
                "<article><h3><a href=\"/entries/{}\">{}</a></h3><p>{}</p><p>{}</p><small>{}</small></article>",
                encode_segment(&result.display_id),
                escape(&result.title),
                entry_badges(&result.entry_type.to_string(), &result.status.to_string()),
                escape(&result.excerpt),
                escape(&result.reason)
            ));
        }
        Ok(self.html_response(StatusCode(200), self.layout("Library", &body, None)))
    }

    fn entry_page(&self, encoded_id: &str) -> Result<HttpResponse, BelayError> {
        let id = decode(encoded_id)?;
        let entry =
            load_entry(&self.snapshot.connection, &id)?.ok_or_else(|| not_found("entry", &id))?;
        let links = load_links(&self.snapshot.connection, entry.internal_id)?;
        let evidence = load_evidence_for_target(&self.snapshot.connection, &id)?;
        let (visible_body, body_truncated) = truncate_utf8(&entry.body, MAX_BODY_BYTES);
        let goal_links = links
            .iter()
            .filter(|link| link.outbound && link.entry_type == EntryType::Goal.to_string())
            .collect::<Vec<_>>();
        let delivery_goal = (entry.entry_type == EntryType::Plan && goal_links.len() == 1)
            .then(|| goal_links[0].display_id.as_str());
        let mut body = format!(
            "<article><h1>{}</h1><p><code class=\"entry-id\">{}</code> {}</p><p class=\"entry-meta\">Updated {} · revision {}</p>{}{}</article>",
            escape(&entry.title),
            escape(&entry.display_id),
            entry_badges(&entry.entry_type.to_string(), &entry.status.to_string()),
            escape(&entry.updated_at),
            entry.revision,
            render_markdown(
                visible_body,
                entry.entry_type == EntryType::Goal,
                entry.entry_type == EntryType::Plan,
                delivery_goal,
            ),
            if body_truncated {
                "<p class=\"warning\">Body truncated at the Browse display limit.</p>"
            } else {
                ""
            }
        );
        body.push_str("<section class=\"panel\"><h2>Typed links</h2><h3>Outbound</h3><ul>");
        for link in links.iter().filter(|link| link.outbound) {
            body.push_str(&link_html(link));
        }
        body.push_str("</ul><h3>Inbound</h3><ul>");
        for link in links.iter().filter(|link| !link.outbound) {
            body.push_str(&link_html(link));
        }
        body.push_str("</ul></section><section class=\"panel\"><h2>Evidence</h2>");
        if evidence.items.is_empty() {
            body.push_str("<p>No Evidence recorded. Unknown.</p>");
        }
        for item in evidence.items {
            body.push_str(&evidence_card(
                &self.repository,
                &self.snapshot.head_sha,
                &self.snapshot.commit_distances,
                &item,
            ));
        }
        if evidence.truncated {
            body.push_str("<p class=\"warning\">Evidence cards truncated at the display limit; omitted verdicts are not summarized as complete.</p>");
        }
        body.push_str(&format!("</section><details class=\"panel\"><summary>Explore neighborhood</summary><p><a href=\"/explore?focus=entry:{}\">Open staged graph</a></p><ul>", encode_query(&id)));
        for link in &links {
            body.push_str(&link_html(link));
        }
        body.push_str("</ul></details>");
        Ok(self.html_response(StatusCode(200), self.layout(&entry.title, &body, None)))
    }

    fn evidence_page(&self, encoded_id: &str) -> Result<HttpResponse, BelayError> {
        let id = decode(encoded_id)?;
        let evidence = load_evidence(&self.snapshot.connection, &id)?
            .ok_or_else(|| not_found("evidence", &id))?;
        let mut body = format!(
            "<article><h1>Evidence {}</h1>{}</article><section class=\"panel\"><h2>Targets</h2><ul>",
            escape(&id),
            evidence_card(
                &self.repository,
                &self.snapshot.head_sha,
                &self.snapshot.commit_distances,
                &evidence,
            )
        );
        for target in &evidence.targets {
            let (entry_id, fragment) = target
                .target
                .split_once('#')
                .map_or((target.target.as_str(), None), |(entry, fragment)| {
                    (entry, Some(fragment))
                });
            let fragment = fragment
                .map(|value| format!("#{}", encode_segment(&value.to_ascii_lowercase())))
                .unwrap_or_default();
            body.push_str(&format!("<li><span class=\"badge relation-{}\">{}</span> <a href=\"/entries/{}{}\">{}</a></li>", escape(&target.relation), escape(&target.relation), encode_segment(entry_id), fragment, escape(&target.target)));
        }
        if evidence.targets_truncated {
            body.push_str(
                "<li class=\"warning\">Evidence targets truncated at the display limit.</li>",
            );
        }
        body.push_str("</ul></section>");
        if evidence.commit_sha == "unknown" {
            body.push_str(
                "<p class=\"panel warning\">Commit is unknown; provenance is unavailable.</p>",
            );
        } else {
            body.push_str(&format!(
                "<p><a href=\"/commits/{}\">Inspect captured commit</a></p>",
                encode_segment(&evidence.commit_sha)
            ));
        }
        Ok(self.html_response(StatusCode(200), self.layout("Evidence", &body, None)))
    }

    fn commit_route(&self, tail: &str) -> Result<HttpResponse, BelayError> {
        let pieces = tail.split('/').collect::<Vec<_>>();
        if pieces.len() == 1 {
            let sha = decode(pieces[0])?;
            return self.commit_page(&sha);
        }
        if pieces.len() == 3 && pieces[1] == "files" {
            return self.file_page(&decode(pieces[0])?, &decode(pieces[2])?);
        }
        Ok(self.html_response(
            StatusCode(404),
            self.layout(
                "Not found",
                "<div class=\"panel error\">Invalid commit path.</div>",
                None,
            ),
        ))
    }

    fn commit_page(&self, sha: &str) -> Result<HttpResponse, BelayError> {
        if !self.snapshot.allowed_commits.contains(sha) {
            return Ok(self.html_response(StatusCode(404), self.layout("Commit unavailable", "<div class=\"panel error\">This commit is not referenced by Evidence in the active snapshot.</div>", None)));
        }
        let mut body = format!(
            "<article><h1>Commit</h1><p><code>{}</code></p><p class=\"warning\">Files below were changed in this commit. This does not assert that they are Entry source files or the scope verified by Evidence.</p></article>",
            escape(sha)
        );
        body.push_str("<section class=\"panel\"><p>Git provenance is resolved only from Evidence in this snapshot.</p><p><a href=\"/explore?focus=commit:");
        body.push_str(&encode_query(sha));
        body.push_str("\">Explore this commit</a></p></section>");
        let (evidence_ids, evidence_truncated) =
            evidence_ids_for_commit(&self.snapshot.connection, sha)?;
        body.push_str("<section class=\"panel\"><h2>Referencing Evidence</h2><ul>");
        for evidence_id in evidence_ids {
            body.push_str(&format!(
                "<li><a href=\"/evidence/{}\">{}</a> <span class=\"badge\">captured at</span></li>",
                encode_segment(&evidence_id),
                escape(&evidence_id)
            ));
        }
        if evidence_truncated {
            body.push_str(
                "<li class=\"warning\">Referencing Evidence truncated at the display limit.</li>",
            );
        }
        body.push_str("</ul></section>");
        // The hardened Git reader supplies the detailed list when the object is available.
        match crate::git_provenance::GitReader::new(
            &self.repository.root,
            self.snapshot.allowed_commits.iter().cloned(),
        )
        .commit(sha)
        {
            Ok(detail) => {
                body.push_str(&format!("<section class=\"panel\"><h2>{}</h2><p>Compared with <code>{}</code>{}</p><table><thead><tr><th>Status</th><th>Path</th><th>Meaning</th></tr></thead><tbody>", escape(&detail.subject), escape(&detail.base_sha), if detail.truncated { " · truncated" } else { "" }));
                for file in detail.files {
                    body.push_str(&format!("<tr><td>{}</td><td><a href=\"/commits/{}/files/{}\">{}</a></td><td>changed in this commit</td></tr>", escape(&file.status), encode_segment(sha), encode_segment(&file.opaque_id), escape(&file.path)));
                }
                body.push_str("</tbody></table></section>");
            }
            Err(error) => body.push_str(&format!(
                "<div class=\"panel warning\"><strong>Unavailable.</strong> {}</div>",
                escape(&error.to_string())
            )),
        }
        Ok(self.html_response(StatusCode(200), self.layout("Commit", &body, None)))
    }

    fn file_page(&self, sha: &str, opaque: &str) -> Result<HttpResponse, BelayError> {
        if !self.snapshot.allowed_commits.contains(sha) {
            return Ok(self.html_response(
                StatusCode(404),
                self.layout(
                    "File unavailable",
                    "<div class=\"panel error\">Commit is outside the Evidence allowlist.</div>",
                    None,
                ),
            ));
        }
        let reader = crate::git_provenance::GitReader::new(
            &self.repository.root,
            self.snapshot.allowed_commits.iter().cloned(),
        );
        match reader.file(sha, opaque) {
            Ok(file) => {
                let mut body = format!(
                    "<article><h1>{}</h1><p class=\"warning\">This file was changed in commit <code>{}</code>. No direct Entry relationship or Evidence verification scope is asserted.</p><p>Type: {} · {} bytes{}</p></article>",
                    escape(&file.path),
                    escape(sha),
                    escape(&file.kind),
                    file.size,
                    if file.truncated { " · truncated" } else { "" }
                );
                if let Some(ref content) = file.content {
                    body.push_str(&format!("<section class=\"panel\"><h2>Content at the relevant commit side</h2><pre>{}</pre></section>", escape(content)));
                }
                if let Some(ref diff) = file.diff {
                    body.push_str(&format!(
                        "<section class=\"panel\"><h2>Diff</h2><pre>{}</pre></section>",
                        escape(diff)
                    ));
                }
                if file.content.is_none() && file.diff.is_none() {
                    body.push_str("<p class=\"panel warning\">Binary or non-UTF-8 content is shown as metadata only.</p>");
                }
                Ok(self.html_response(StatusCode(200), self.layout("Changed file", &body, None)))
            }
            Err(error) => Ok(self.html_response(
                StatusCode(404),
                self.layout(
                    "File unavailable",
                    &format!(
                        "<div class=\"panel error\">{}</div>",
                        escape(&error.to_string())
                    ),
                    None,
                ),
            )),
        }
    }

    fn explore_page(&self, focus: Option<&str>) -> HttpResponse {
        let focus = focus.unwrap_or("all");
        let mut body = "<section class=\"panel\"><h1>Explore</h1><p>The overview starts with Goal nodes. Activate a node to expand one provenance neighborhood; double-click to open its detail page. Reader deep links can still start from any Entry.</p><div class=\"graph-legend\" aria-label=\"Node color legend\"><span class=\"badge type-goal\">GOAL</span><span class=\"badge type-plan\">PLN</span><span class=\"badge type-decision\">DEC</span><span class=\"badge type-work\">WORK</span><span class=\"badge type-evidence\">EVD</span></div><div id=\"graph\" role=\"application\" aria-label=\"Trace provenance graph\"></div></section><section class=\"panel\"><h2>Accessible Goal list</h2><p>These normal links are the keyboard and canvas-independent starting points for the same staged relationships.</p><ul>".to_owned();
        if let Ok((entries, truncated)) = entry_summaries(&self.snapshot.connection, true) {
            if entries.is_empty() {
                body.push_str("<li>No Goals are available in this snapshot.</li>");
            }
            for entry in entries {
                body.push_str(&format!(
                    "<li><a href=\"/entries/{}\">{} · {}</a></li>",
                    encode_segment(&entry.display_id),
                    escape(&entry.display_id),
                    escape(&entry.title)
                ));
            }
            if truncated {
                body.push_str("<li class=\"warning\">Goal list truncated at the graph limit.</li>");
            }
        }
        body.push_str("</ul><noscript>Cytoscape requires JavaScript; all relationships remain available through normal HTML links on Reader, Evidence, Commit, and File pages.</noscript></section>");
        self.html_response(StatusCode(200), self.layout("Explore", &body, Some(focus)))
    }

    fn explore_api(&self, focus: Option<&str>) -> Result<HttpResponse, BelayError> {
        let focus = focus.ok_or_else(|| BelayError::Validation {
            message: "focus is required".to_owned(),
        })?;
        let graph = graph_neighbors(
            &self.snapshot.connection,
            &self.repository.root,
            focus,
            &self.snapshot.allowed_commits,
        )?;
        self.json_response(StatusCode(200), &graph)
    }

    fn reload(&mut self, request: &mut Request) -> Result<HttpResponse, BelayError> {
        let supplied = request
            .headers()
            .iter()
            .find(|header| header.field.equiv("X-Belay-Nonce"))
            .map(|header| header.value.as_str())
            .unwrap_or("");
        if supplied != self.reload_nonce {
            return self.json_response(
                StatusCode(403),
                &ApiMessage {
                    message: "Reload rejected: invalid process nonce.",
                },
            );
        }
        match Snapshot::build(&self.repository) {
            Ok(snapshot) => {
                self.snapshot = snapshot;
                self.last_reload_result = "Snapshot reloaded atomically.".to_owned();
                self.json_response(
                    StatusCode(200),
                    &ApiMessage {
                        message: "Snapshot reloaded atomically.",
                    },
                )
            }
            Err(error) => {
                self.last_reload_result =
                    format!("Reload failed; previous snapshot retained: {error}");
                self.json_response(
                    StatusCode(503),
                    &ApiOwnedMessage {
                        message: self.last_reload_result.clone(),
                    },
                )
            }
        }
    }

    fn valid_host(&self, request: &Request) -> bool {
        let Some(value) = request
            .headers()
            .iter()
            .find(|header| header.field.equiv("Host"))
            .map(|header| header.value.as_str())
        else {
            return false;
        };
        value == format!("127.0.0.1:{}", self.port) || value == format!("localhost:{}", self.port)
    }

    fn valid_origin(&self, request: &Request) -> bool {
        let host = request
            .headers()
            .iter()
            .find(|header| header.field.equiv("Host"))
            .map(|header| header.value.as_str());
        let origin = request
            .headers()
            .iter()
            .find(|header| header.field.equiv("Origin"));
        let reload =
            request.method() == &Method::Post && split_url(request.url()).0 == "/api/reload";
        if reload && origin.is_none() {
            return false;
        }
        origin.is_none_or(|header| {
            host.is_some_and(|host| header.value.as_str() == format!("http://{host}"))
        })
    }

    fn layout(&self, title: &str, content: &str, focus: Option<&str>) -> String {
        let drift = if self.snapshot.has_drift {
            "detected"
        } else {
            "none"
        };
        let mut diagnostic_items = String::new();
        for item in &self.snapshot.diagnostics {
            diagnostic_items.push_str(&format!("<li>{}</li>", escape(item)));
        }
        let diagnostic_details = if diagnostic_items.is_empty() {
            String::new()
        } else {
            format!("<h3>Diagnostics</h3><ul>{diagnostic_items}</ul>")
        };
        let reload_result = if self.last_reload_result.starts_with("Reload has not run") {
            ""
        } else {
            &self.last_reload_result
        };
        let snapshot_summary = if self.snapshot.has_drift {
            "Drift detected".to_owned()
        } else if self.snapshot.diagnostics.is_empty() {
            "Snapshot details".to_owned()
        } else {
            format!("Snapshot details ({})", self.snapshot.diagnostics.len())
        };
        format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{} · Belay</title><link rel=\"stylesheet\" href=\"/assets/app.css\"><script defer src=\"/assets/cytoscape.js\"></script><script defer src=\"/assets/app.js\"></script></head><body data-reload-nonce=\"{}\" data-focus=\"{}\"><header><nav><a class=\"brand\" href=\"/\">Belay</a><a href=\"/\">Library</a><a href=\"/explore\">Explore</a><span class=\"nav-actions\"><button type=\"button\" data-reload>Reload</button><span class=\"reload-result\" data-reload-result aria-live=\"polite\">{}</span><details class=\"snapshot-status\"><summary>{}</summary><div class=\"snapshot-popover\"><dl><dt>Snapshot</dt><dd>{}</dd><dt>HEAD</dt><dd><code>{}</code></dd><dt>Drift</dt><dd>{}</dd></dl>{}</div></details></span></nav></header><main>{}</main></body></html>",
            escape(title),
            escape(&self.reload_nonce),
            escape(focus.unwrap_or("")),
            escape(reload_result),
            escape(&snapshot_summary),
            escape(&self.snapshot.generated_at),
            escape(&self.snapshot.head_sha),
            drift,
            diagnostic_details,
            content
        )
    }

    fn html_response(&self, status: StatusCode, html: String) -> HttpResponse {
        self.response(status, "text/html; charset=utf-8", html.into_bytes())
    }
    fn json_response<T: Serialize>(
        &self,
        status: StatusCode,
        value: &T,
    ) -> Result<HttpResponse, BelayError> {
        let bytes = serde_json::to_vec(value).map_err(|source| BelayError::Validation {
            message: format!("could not serialize Browse response: {source}"),
        })?;
        Ok(self.response(status, "application/json; charset=utf-8", bytes))
    }
    fn response(&self, status: StatusCode, content_type: &str, bytes: Vec<u8>) -> HttpResponse {
        let mut response = Response::new(status, Vec::new(), Cursor::new(bytes), None, None);
        for (name, value) in [
            ("Content-Type", content_type),
            ("Cache-Control", "no-store"),
            ("X-Content-Type-Options", "nosniff"),
            ("X-Frame-Options", "DENY"),
            ("Referrer-Policy", "no-referrer"),
            (
                "Content-Security-Policy",
                "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'none'; font-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'",
            ),
        ] {
            response.add_header(Header::from_bytes(name, value).expect("static header is valid"));
        }
        response
    }
}

type HttpResponse = Response<Cursor<Vec<u8>>>;

struct Snapshot {
    connection: Connection,
    generated_at: String,
    head_sha: String,
    has_drift: bool,
    diagnostics: Vec<String>,
    allowed_commits: BTreeSet<String>,
    commit_distances: HashMap<String, Result<u32, String>>,
}

impl Snapshot {
    fn build(repository: &Repository) -> Result<Self, BelayError> {
        let database_path = repository.database_path();
        let source = crate::database::open_read_only(&database_path)?;
        let mut connection = Connection::open_in_memory()
            .map_err(|source| BelayError::sqlite(":memory:", source))?;
        let backup = Backup::new(&source, &mut connection)
            .map_err(|source| BelayError::sqlite(&database_path, source))?;
        backup
            .run_to_completion(128, Duration::from_millis(2), None)
            .map_err(|source| BelayError::sqlite(&database_path, source))?;
        drop(backup);
        crate::database::verify_schema_health(&connection, &database_path)?;
        connection
            .pragma_update(None, "query_only", true)
            .map_err(|source| BelayError::sqlite(":memory:", source))?;
        let report = crate::reconcile::doctor(repository);
        let has_drift = report.has_drift;
        let diagnostics = report
            .checks
            .into_iter()
            .filter(|check| check.status != "ok")
            .map(|check| format!("{}: {} ({})", check.name, check.status, check.detail))
            .collect();
        let head_sha = crate::git_provenance::read_head(&repository.root)
            .unwrap_or_else(|_| "unknown".to_owned());
        let mut statement = connection
            .prepare("SELECT DISTINCT commit_sha FROM evidence ORDER BY commit_sha")
            .map_err(|source| BelayError::sqlite(&database_path, source))?;
        let allowed_commits = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|source| BelayError::sqlite(&database_path, source))?
            .collect::<rusqlite::Result<BTreeSet<_>>>()
            .map_err(|source| BelayError::sqlite(&database_path, source))?;
        drop(statement);
        let reader = crate::git_provenance::GitReader::new(
            &repository.root,
            allowed_commits.iter().cloned(),
        );
        let commit_distances = if head_sha == "unknown" {
            HashMap::new()
        } else {
            allowed_commits
                .iter()
                .take(MAX_GIT_FRESHNESS_COMMITS)
                .map(|commit| {
                    (
                        commit.clone(),
                        reader
                            .commits_behind(commit, &head_sha)
                            .map_err(|error| error.to_string()),
                    )
                })
                .collect()
        };
        Ok(Self {
            connection,
            generated_at: Local::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            head_sha,
            has_drift,
            diagnostics,
            allowed_commits,
            commit_distances,
        })
    }
}

#[derive(Debug)]
struct EntryView {
    internal_id: i64,
    display_id: String,
    entry_type: EntryType,
    title: String,
    status: EntryStatus,
    updated_at: String,
    revision: i64,
    body: String,
}

struct EntrySummary {
    display_id: String,
    title: String,
    entry_type: String,
}

#[derive(Debug)]
struct LinkView {
    outbound: bool,
    relation: String,
    display_id: String,
    title: String,
    entry_type: String,
    fragment: String,
}

#[derive(Debug)]
struct EvidenceView {
    display_id: String,
    kind: String,
    verdict: String,
    commit_sha: String,
    captured_at: String,
    source: String,
    issuer: String,
    summary: String,
    targets: Vec<EvidenceTarget>,
    targets_truncated: bool,
}

#[derive(Debug)]
struct EvidenceTarget {
    target: String,
    relation: String,
}

struct EvidenceBatch {
    items: Vec<EvidenceView>,
    truncated: bool,
}

fn load_entry(connection: &Connection, display_id: &str) -> Result<Option<EntryView>, BelayError> {
    connection.query_row("SELECT id, display_id, type, title, status, updated_at, revision, body FROM entries WHERE display_id=?1", [display_id], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?, row.get::<_, i64>(6)?, row.get::<_, String>(7)?))).optional().map_err(|source| BelayError::sqlite(":memory:", source))?.map(|row| Ok(EntryView { internal_id: row.0, display_id: row.1, entry_type: row.2.parse()?, title: row.3, status: row.4.parse()?, updated_at: row.5, revision: row.6, body: row.7 })).transpose()
}

fn load_links(connection: &Connection, id: i64) -> Result<Vec<LinkView>, BelayError> {
    let mut statement = connection.prepare("SELECT l.from_entry_id=e.id, l.relation, CASE WHEN l.from_entry_id=e.id THEN t.display_id ELSE s.display_id END, CASE WHEN l.from_entry_id=e.id THEN t.title ELSE s.title END, CASE WHEN l.from_entry_id=e.id THEN t.type ELSE s.type END, CASE WHEN l.from_entry_id=e.id THEN l.to_fragment ELSE '' END FROM entry_links l JOIN entries e ON e.id=?1 JOIN entries s ON s.id=l.from_entry_id JOIN entries t ON t.id=l.to_entry_id WHERE l.from_entry_id=e.id OR l.to_entry_id=e.id ORDER BY l.relation, 3").map_err(|source| BelayError::sqlite(":memory:", source))?;
    statement
        .query_map([id], |row| {
            Ok(LinkView {
                outbound: row.get(0)?,
                relation: row.get(1)?,
                display_id: row.get(2)?,
                title: row.get(3)?,
                entry_type: row.get(4)?,
                fragment: row.get(5)?,
            })
        })
        .map_err(|source| BelayError::sqlite(":memory:", source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(":memory:", source))
}

fn load_evidence_for_target(
    connection: &Connection,
    target: &str,
) -> Result<EvidenceBatch, BelayError> {
    let mut statement = connection.prepare("SELECT DISTINCT e.display_id FROM evidence_links l JOIN evidence e ON e.id=l.evidence_id WHERE l.target=?1 OR l.target LIKE ?2 ORDER BY julianday(e.captured_at) DESC, e.display_id DESC LIMIT ?3").map_err(|source| BelayError::sqlite(":memory:", source))?;
    let prefix = format!("{target}#%");
    let mut ids = statement
        .query_map(params![target, prefix, (MAX_EVIDENCE + 1) as i64], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|source| BelayError::sqlite(":memory:", source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(":memory:", source))?;
    let truncated = ids.len() > MAX_EVIDENCE;
    ids.truncate(MAX_EVIDENCE);
    let items = ids
        .into_iter()
        .map(|id| {
            load_evidence(connection, &id)
                .and_then(|item| item.ok_or_else(|| not_found("evidence", &id)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EvidenceBatch { items, truncated })
}

fn load_evidence(connection: &Connection, id: &str) -> Result<Option<EvidenceView>, BelayError> {
    let row = connection.query_row("SELECT id, display_id, kind, verdict, commit_sha, captured_at, source, issuer, summary FROM evidence WHERE display_id=?1", [id], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?, row.get::<_, String>(6)?, row.get::<_, String>(7)?, row.get::<_, String>(8)?))).optional().map_err(|source| BelayError::sqlite(":memory:", source))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let mut statement = connection.prepare("SELECT target, relation FROM evidence_links WHERE evidence_id=?1 ORDER BY relation, target LIMIT ?2").map_err(|source| BelayError::sqlite(":memory:", source))?;
    let mut targets = statement
        .query_map(params![row.0, (MAX_EVIDENCE + 1) as i64], |row| {
            Ok(EvidenceTarget {
                target: row.get(0)?,
                relation: row.get(1)?,
            })
        })
        .map_err(|source| BelayError::sqlite(":memory:", source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(":memory:", source))?;
    let targets_truncated = targets.len() > MAX_EVIDENCE;
    targets.truncate(MAX_EVIDENCE);
    Ok(Some(EvidenceView {
        display_id: row.1,
        kind: row.2,
        verdict: row.3,
        commit_sha: row.4,
        captured_at: row.5,
        source: row.6,
        issuer: row.7,
        summary: row.8,
        targets,
        targets_truncated,
    }))
}

fn recent_entries(connection: &Connection) -> Result<Vec<search::SearchResult>, BelayError> {
    let mut statement = connection.prepare("SELECT display_id FROM entries ORDER BY julianday(updated_at) DESC, display_id LIMIT 20").map_err(|source| BelayError::sqlite(":memory:", source))?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|source| BelayError::sqlite(":memory:", source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(":memory:", source))?;
    ids.into_iter()
        .map(|id| {
            search::search_connection(
                connection,
                Path::new(":memory:"),
                &SearchRequest {
                    query: String::new(),
                    entry_type: None,
                    status_include: Vec::new(),
                    status_exclude: Vec::new(),
                    tag: None,
                    display_id: Some(id),
                    limit: 1,
                },
            )
            .and_then(|mut found| found.pop().ok_or_else(|| not_found("entry", "recent")))
        })
        .collect()
}

fn render_markdown(markdown: &str, goal: bool, plan: bool, delivery_goal: Option<&str>) -> String {
    let mut rendered = String::new();
    let mut events = Vec::new();
    let mut suppress_links = 0_u32;
    for event in Parser::new_ext(markdown, Options::all()) {
        match event {
            Event::Html(_) | Event::InlineHtml(_) => {}
            Event::Start(tag) => {
                if matches!(
                    tag,
                    Tag::Link { .. } | Tag::Image { .. } | Tag::CodeBlock(_)
                ) {
                    suppress_links += 1;
                }
                events.push(Event::Start(tag.into_static()));
            }
            Event::End(end) => {
                events.push(Event::End(end));
                if matches!(end, TagEnd::Link | TagEnd::Image | TagEnd::CodeBlock) {
                    suppress_links = suppress_links.saturating_sub(1);
                }
            }
            Event::Text(text) if suppress_links == 0 => {
                events.extend(link_reference_text(text.as_ref()));
            }
            event => events.push(event.into_static()),
        }
    }
    html::push_html(&mut rendered, events.into_iter());
    let mut builder = Builder::default();
    builder.rm_tags(HashSet::from(["img"]));
    builder.url_schemes(HashSet::from(["http", "https", "mailto"]));
    builder.link_rel(Some("noopener noreferrer"));
    let mut clean = builder.clean(&rendered).to_string();
    if goal {
        add_success_criteria_anchors(markdown, &mut clean);
    }
    if plan {
        decorate_delivery_map(markdown, &mut clean, delivery_goal);
    }
    format!("<div class=\"markdown-body\">{clean}</div>")
}

fn link_reference_text(text: &str) -> Vec<Event<'static>> {
    let spans = crate::trace_ids::reference_spans(text);
    if spans.is_empty() {
        return vec![Event::Text(CowStr::from(text.to_owned()))];
    }
    let mut events = Vec::new();
    let mut cursor = 0;
    for span in spans {
        if span.start > cursor {
            events.push(Event::Text(CowStr::from(
                text[cursor..span.start].to_owned(),
            )));
        }
        let destination = if span.evidence {
            format!("/evidence/{}", encode_segment(&span.value))
        } else {
            let (entry, fragment) = span
                .value
                .split_once('#')
                .map_or((span.value.as_str(), None), |(entry, fragment)| {
                    (entry, Some(fragment))
                });
            fragment.map_or_else(
                || format!("/entries/{}", encode_segment(entry)),
                |fragment| {
                    format!(
                        "/entries/{}#{}",
                        encode_segment(entry),
                        encode_segment(&fragment.to_ascii_lowercase())
                    )
                },
            )
        };
        events.push(Event::Start(Tag::Link {
            link_type: LinkType::Inline,
            dest_url: CowStr::from(destination),
            title: CowStr::from(String::new()),
            id: CowStr::from(String::new()),
        }));
        events.push(Event::Text(CowStr::from(span.value)));
        events.push(Event::End(TagEnd::Link));
        cursor = span.end;
    }
    if cursor < text.len() {
        events.push(Event::Text(CowStr::from(text[cursor..].to_owned())));
    }
    events
}

fn add_success_criteria_anchors(markdown: &str, html: &mut String) {
    let mut in_section = false;
    let mut html_cursor = html
        .find(">Success Criteria</h2>")
        .map_or(0, |position| position + ">Success Criteria</h2>".len());
    for line in markdown.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            in_section = heading.trim().eq_ignore_ascii_case("Success Criteria");
            continue;
        }
        if !in_section {
            continue;
        }
        if line.starts_with("## ") {
            break;
        }
        let Some(item) = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .or_else(|| line.strip_prefix("+ "))
        else {
            continue;
        };
        let Some(relative) = html[html_cursor..].find("<li>") else {
            continue;
        };
        let position = html_cursor + relative;
        html_cursor = position + 4;
        let Some(id) = crate::trace_ids::explicit_goal_id(item).map(str::to_ascii_lowercase) else {
            continue;
        };
        let replacement = format!("<li id=\"{id}\">");
        html.replace_range(position..position + 4, &replacement);
        html_cursor = position + replacement.len();
    }
}

fn decorate_delivery_map(markdown: &str, html: &mut String, goal_id: Option<&str>) {
    let Some(section_start) = html.find(">Delivery Map</h2>") else {
        return;
    };
    let mut lines = markdown.lines().skip_while(|line| {
        !line
            .trim_start_matches('#')
            .trim()
            .eq_ignore_ascii_case("Delivery Map")
    });
    let _ = lines.next();
    let Some(header_line) = lines.find(|line| line.trim_start().starts_with('|')) else {
        return;
    };
    let headers = markdown_table_cells(header_line);
    let Some(id_index) = headers
        .iter()
        .position(|header| header.eq_ignore_ascii_case("ID"))
    else {
        return;
    };
    let goal_index = headers
        .iter()
        .position(|header| header.eq_ignore_ascii_case("Goal item"));
    let status_index = headers
        .iter()
        .position(|header| header.eq_ignore_ascii_case("State"));
    let actor_index = headers
        .iter()
        .position(|header| header.eq_ignore_ascii_case("Actor"));
    let mut cursor = section_start;
    for line in lines {
        if !line.trim_start().starts_with('|') {
            break;
        }
        let cells = markdown_table_cells(line);
        if cells.len() != headers.len()
            || cells.iter().all(|cell| {
                cell.chars()
                    .all(|character| character == '-' || character == ':')
            })
        {
            continue;
        }
        let task_id = cells[id_index].trim();
        if !task_id.starts_with("T-") {
            continue;
        }
        let task_fragment = task_id.to_ascii_lowercase();
        cursor = decorate_table_cell(
            html,
            cursor,
            task_id,
            &format!("class=\"delivery-id\" id=\"{}\"", task_fragment),
            &format!("<a href=\"#{}\">{}</a>", task_fragment, escape(task_id)),
        );
        if let Some(index) = goal_index {
            let goal_item = cells[index].trim();
            let linked = goal_id
                .map(|goal_id| link_goal_items(goal_item, goal_id))
                .unwrap_or_else(|| escape(goal_item));
            cursor = decorate_table_cell(html, cursor, goal_item, "class=\"goal-item\"", &linked);
        }
        if let Some(index) = actor_index {
            let actor = cells[index].trim();
            cursor = decorate_table_cell(
                html,
                cursor,
                actor,
                "class=\"delivery-actor\"",
                &escape(actor),
            );
        }
        if let Some(index) = status_index {
            let state = cells[index].trim();
            cursor = decorate_table_cell(
                html,
                cursor,
                state,
                "class=\"delivery-status\"",
                &format!(
                    "<span class=\"badge status-{}\">{}</span>",
                    escape(state),
                    escape(state)
                ),
            );
        }
    }
}

fn markdown_table_cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_owned())
        .collect()
}

fn decorate_table_cell(
    html: &mut String,
    cursor: usize,
    value: &str,
    attributes: &str,
    content: &str,
) -> usize {
    let needle = format!("<td>{}</td>", escape(value));
    let Some(relative) = html[cursor..].find(&needle) else {
        return cursor;
    };
    let position = cursor + relative;
    let replacement = format!("<td {attributes}>{content}</td>");
    html.replace_range(position..position + needle.len(), &replacement);
    position + replacement.len()
}

fn link_goal_items(value: &str, goal_id: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(relative) = value[cursor..].find("SC-") {
        let start = cursor + relative;
        output.push_str(&escape(&value[cursor..start]));
        let digits_start = start + 3;
        let digits_end = value[digits_start..]
            .find(|character: char| !character.is_ascii_digit())
            .map_or(value.len(), |relative| digits_start + relative);
        if digits_end == digits_start {
            output.push_str("SC-");
            cursor = digits_start;
            continue;
        }
        let label = &value[start..digits_end];
        output.push_str(&format!(
            "<a href=\"/entries/{}#{}\">{}</a>",
            encode_segment(goal_id),
            label.to_ascii_lowercase(),
            escape(label)
        ));
        cursor = digits_end;
    }
    output.push_str(&escape(&value[cursor..]));
    output
}

fn evidence_card(
    repository: &Repository,
    head: &str,
    commit_distances: &HashMap<String, Result<u32, String>>,
    item: &EvidenceView,
) -> String {
    let freshness = browse_freshness(
        repository,
        head,
        commit_distances,
        &item.commit_sha,
        &item.captured_at,
    );
    let relations = item
        .targets
        .iter()
        .map(|target| {
            format!(
                "<span class=\"badge relation-{}\">relation: {}</span>",
                escape(&target.relation),
                escape(&target.relation)
            )
        })
        .collect::<String>();
    let freshness_class = if freshness.starts_with("fresh") {
        "fresh"
    } else if freshness.starts_with("stale") {
        "stale"
    } else {
        "unknown"
    };
    format!(
        "<div class=\"card verdict-{}\"><h3><a href=\"/evidence/{}\">{}</a></h3><p><span class=\"badge type-evidence\">EVD</span><span class=\"badge verdict-{}\">verdict: {}</span> <span class=\"badge freshness-{}\">freshness: {}</span>{}</p><p>{}</p><dl><dt>Kind</dt><dd>{}</dd><dt>Captured</dt><dd>{}</dd><dt>Commit</dt><dd><code>{}</code></dd><dt>Source</dt><dd>{}</dd><dt>Issuer</dt><dd>{}</dd></dl></div>",
        escape(&item.verdict),
        encode_segment(&item.display_id),
        escape(&item.display_id),
        escape(&item.verdict),
        escape(&item.verdict),
        freshness_class,
        escape(&freshness),
        relations,
        escape(&item.summary),
        escape(&item.kind),
        escape(&item.captured_at),
        escape(&item.commit_sha),
        escape(&item.source),
        escape(&item.issuer)
    )
}

fn browse_freshness(
    repository: &Repository,
    head: &str,
    commit_distances: &HashMap<String, Result<u32, String>>,
    commit_sha: &str,
    captured_at: &str,
) -> String {
    let Ok(captured) = DateTime::parse_from_rfc3339(captured_at) else {
        return "unknown (captured-at invalid)".to_owned();
    };
    let captured = captured.with_timezone(&Utc);
    let now = Utc::now();
    if captured > now {
        return "unknown (captured-at is in the future)".to_owned();
    }
    let stale_after =
        chrono::Duration::try_days(i64::from(repository.config.verify.stale_after_days));
    if let Some(Ok(behind)) = commit_distances.get(commit_sha) {
        return if *behind <= repository.config.verify.stale_after_commits {
            "fresh".to_owned()
        } else {
            format!("stale ({behind} commits behind)")
        };
    }
    let fallback_reason = if commit_sha == "unknown" {
        "commit unknown"
    } else if head == "unknown" {
        "HEAD unavailable"
    } else if commit_distances.contains_key(commit_sha) {
        "Git object unavailable"
    } else {
        return "unknown (freshness not evaluated due to snapshot Git limit)".to_owned();
    };
    if stale_after.is_some_and(|threshold| now.signed_duration_since(captured) > threshold) {
        format!(
            "stale (older than {} days; {fallback_reason})",
            repository.config.verify.stale_after_days
        )
    } else {
        "fresh".to_owned()
    }
}

fn link_html(link: &LinkView) -> String {
    let suffix = if link.fragment.is_empty() {
        String::new()
    } else {
        format!("#{}", encode_segment(&link.fragment.to_ascii_lowercase()))
    };
    format!(
        "<li><span class=\"badge relation-{}\">{}</span> <span class=\"badge type-{}\">{}</span> <a href=\"/entries/{}{}\">{} · {}</a></li>",
        escape(&link.relation),
        escape(&link.relation),
        escape(&link.entry_type),
        escape(&link.entry_type),
        encode_segment(&link.display_id),
        suffix,
        escape(&link.display_id),
        escape(&link.title)
    )
}

fn entry_badges(entry_type: &str, status: &str) -> String {
    format!(
        "<span class=\"badge type-{}\">{}</span><span class=\"badge status-{}\">{}</span>",
        escape(entry_type),
        escape(entry_type),
        escape(status),
        escape(status)
    )
}

#[derive(Serialize)]
struct ApiMessage<'a> {
    message: &'a str,
}
#[derive(Serialize)]
struct ApiOwnedMessage {
    message: String,
}
#[derive(Serialize)]
struct Graph {
    nodes: Vec<GraphElement>,
    edges: Vec<GraphElement>,
    truncated: bool,
}
#[derive(Serialize)]
struct GraphElement {
    data: serde_json::Value,
}

fn graph_neighbors(
    connection: &Connection,
    repository_root: &Path,
    focus: &str,
    allowed: &BTreeSet<String>,
) -> Result<Graph, BelayError> {
    let mut graph = Graph {
        nodes: Vec::new(),
        edges: Vec::new(),
        truncated: false,
    };
    if focus == "all" {
        let (entries, truncated) = entry_summaries(connection, true)?;
        graph.truncated = truncated;
        for entry in entries {
            graph.nodes.push(node(
                &format!("entry:{}", entry.display_id),
                &entry.title,
                "entry",
                &entry.entry_type,
                &format!("/entries/{}", encode_segment(&entry.display_id)),
            ));
        }
    } else if let Some(id) = focus.strip_prefix("entry:") {
        let entry = load_entry(connection, id)?.ok_or_else(|| not_found("entry", id))?;
        graph.nodes.push(node(
            focus,
            &entry.title,
            "entry",
            &entry.entry_type.to_string(),
            &format!("/entries/{}", encode_segment(id)),
        ));
        let links = load_links(connection, entry.internal_id)?;
        graph.truncated |= links.len() > MAX_GRAPH_NEIGHBORS;
        for link in links.into_iter().take(MAX_GRAPH_NEIGHBORS) {
            let target = format!("entry:{}", link.display_id);
            graph.nodes.push(node(
                &target,
                &link.title,
                "entry",
                &link.entry_type,
                &format!("/entries/{}", encode_segment(&link.display_id)),
            ));
            let (source, destination) = if link.outbound {
                (focus.to_owned(), target)
            } else {
                (target, focus.to_owned())
            };
            graph
                .edges
                .push(edge(&source, &destination, &link.relation));
        }
        let evidence_items = load_evidence_for_target(connection, id)?;
        graph.truncated |= evidence_items.truncated;
        for evidence in evidence_items.items {
            let relation = evidence
                .targets
                .iter()
                .find(|target| target.target == id || target.target.starts_with(&format!("{id}#")))
                .map_or("verifies", |target| target.relation.as_str());
            let target = format!("evidence:{}", evidence.display_id);
            graph.nodes.push(node(
                &target,
                &evidence.display_id,
                "evidence",
                "evidence",
                &format!("/evidence/{}", encode_segment(&evidence.display_id)),
            ));
            graph.edges.push(edge(focus, &target, relation));
        }
    } else if let Some(id) = focus.strip_prefix("evidence:") {
        let evidence = load_evidence(connection, id)?.ok_or_else(|| not_found("evidence", id))?;
        graph.nodes.push(node(
            focus,
            id,
            "evidence",
            "evidence",
            &format!("/evidence/{}", encode_segment(id)),
        ));
        graph.truncated |= evidence.targets_truncated;
        for target in evidence.targets.iter().take(MAX_GRAPH_NEIGHBORS) {
            let entry_id = target.target.split('#').next().unwrap_or(&target.target);
            let node_id = format!("entry:{entry_id}");
            graph.nodes.push(node(
                &node_id,
                entry_id,
                "entry",
                &load_entry(connection, entry_id)?
                    .map_or_else(|| "entry".to_owned(), |entry| entry.entry_type.to_string()),
                &format!("/entries/{}", encode_segment(entry_id)),
            ));
            graph.edges.push(edge(focus, &node_id, &target.relation));
        }
        if allowed.contains(&evidence.commit_sha) && evidence.commit_sha != "unknown" {
            let commit = format!("commit:{}", evidence.commit_sha);
            graph.nodes.push(node(
                &commit,
                &short_sha(&evidence.commit_sha),
                "commit",
                "commit",
                &format!("/commits/{}", encode_segment(&evidence.commit_sha)),
            ));
            graph.edges.push(edge(focus, &commit, "captured at"));
        }
    } else if let Some(sha) = focus.strip_prefix("commit:") {
        if !allowed.contains(sha) {
            return Err(not_found("allowed commit", sha));
        }
        graph.nodes.push(node(
            focus,
            &short_sha(sha),
            "commit",
            "commit",
            &format!("/commits/{}", encode_segment(sha)),
        ));
        let (evidence_ids, evidence_truncated) = evidence_ids_for_commit(connection, sha)?;
        graph.truncated |= evidence_truncated;
        for evidence_id in evidence_ids {
            let evidence_node = format!("evidence:{evidence_id}");
            graph.nodes.push(node(
                &evidence_node,
                &evidence_id,
                "evidence",
                "evidence",
                &format!("/evidence/{}", encode_segment(&evidence_id)),
            ));
            graph.edges.push(edge(&evidence_node, focus, "captured at"));
        }
        let reader =
            crate::git_provenance::GitReader::new(repository_root, allowed.iter().cloned());
        if let Ok(detail) = reader.commit(sha) {
            graph.truncated |= detail.truncated || detail.files.len() > MAX_GRAPH_NEIGHBORS;
            for file in detail.files.into_iter().take(MAX_GRAPH_NEIGHBORS) {
                let id = format!("file:{sha}:{}", file.opaque_id);
                graph.nodes.push(node(
                    &id,
                    &file.path,
                    "file",
                    "file",
                    &format!(
                        "/commits/{}/files/{}",
                        encode_segment(sha),
                        encode_segment(&file.opaque_id)
                    ),
                ));
                graph.edges.push(edge(focus, &id, "changed"));
            }
        }
    }
    Ok(graph)
}

fn evidence_ids_for_commit(
    connection: &Connection,
    sha: &str,
) -> Result<(Vec<String>, bool), BelayError> {
    let mut statement = connection
        .prepare("SELECT display_id FROM evidence WHERE commit_sha=?1 ORDER BY display_id LIMIT ?2")
        .map_err(|source| BelayError::sqlite(":memory:", source))?;
    let mut ids = statement
        .query_map(params![sha, (MAX_GRAPH_NEIGHBORS + 1) as i64], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|source| BelayError::sqlite(":memory:", source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(":memory:", source))?;
    let truncated = ids.len() > MAX_GRAPH_NEIGHBORS;
    ids.truncate(MAX_GRAPH_NEIGHBORS);
    Ok((ids, truncated))
}

fn entry_summaries(
    connection: &Connection,
    goals_only: bool,
) -> Result<(Vec<EntrySummary>, bool), BelayError> {
    let sql = if goals_only {
        "SELECT display_id, title, type FROM entries WHERE type='goal' ORDER BY display_id LIMIT ?1"
    } else {
        "SELECT display_id, title, type FROM entries ORDER BY type, display_id LIMIT ?1"
    };
    let mut statement = connection
        .prepare(sql)
        .map_err(|source| BelayError::sqlite(":memory:", source))?;
    statement
        .query_map([(MAX_GRAPH_NEIGHBORS + 1) as i64], |row| {
            Ok(EntrySummary {
                display_id: row.get(0)?,
                title: row.get(1)?,
                entry_type: row.get(2)?,
            })
        })
        .map_err(|source| BelayError::sqlite(":memory:", source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(":memory:", source))
        .map(|mut entries| {
            let truncated = entries.len() > MAX_GRAPH_NEIGHBORS;
            entries.truncate(MAX_GRAPH_NEIGHBORS);
            (entries, truncated)
        })
}

fn node(id: &str, label: &str, kind: &str, entry_type: &str, href: &str) -> GraphElement {
    GraphElement {
        data: serde_json::json!({"id":id,"label":label,"kind":kind,"entry_type":entry_type,"href":href}),
    }
}
fn edge(source: &str, target: &str, label: &str) -> GraphElement {
    let digest = Sha256::digest(format!("{source}\0{target}\0{label}"));
    GraphElement {
        data: serde_json::json!({"id":format!("edge-{digest:x}"),"source":source,"target":target,"label":label}),
    }
}

fn nonce(repository: &Repository, generated_at: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!(
            "{}\0{}\0{}",
            repository.root.display(),
            generated_at,
            std::process::id()
        ))
    )
}
fn short_sha(sha: &str) -> String {
    sha.chars().take(12).collect()
}
fn not_found(kind: &str, id: &str) -> BelayError {
    BelayError::Validation {
        message: format!("{kind} {id:?} was not found in the active snapshot"),
    }
}
fn split_url(url: &str) -> (String, String) {
    match url.split_once('?') {
        Some((path, query)) => (path.to_owned(), query.to_owned()),
        None => (url.to_owned(), String::new()),
    }
}
fn query_value(query: &str, key: &str) -> Option<String> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(name, _)| decode(name).ok().as_deref() == Some(key))
        .and_then(|(_, value)| decode(value).ok())
}
fn decode(value: &str) -> Result<String, BelayError> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let high = hex(bytes[index + 1])?;
                let low = hex(bytes[index + 2])?;
                output.push(high * 16 + low);
                index += 3;
            }
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(output).map_err(|_| BelayError::Validation {
        message: "URL contains non-UTF-8 data".to_owned(),
    })
}
fn hex(byte: u8) -> Result<u8, BelayError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(BelayError::Validation {
            message: "URL contains invalid percent encoding".to_owned(),
        }),
    }
}
fn encode_segment(value: &str) -> String {
    percent_encode(value, false)
}
fn encode_query(value: &str) -> String {
    percent_encode(value, true)
}
fn percent_encode(value: &str, query: bool) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) || (query && byte == b':') {
            output.push(char::from(byte));
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn truncate_utf8(value: &str, max_bytes: usize) -> (&str, bool) {
    if value.len() <= max_bytes {
        return (value, false);
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (&value[..end], true)
}
const STATUS_GROUPS: &[(&str, &[EntryStatus])] = &[
    (
        "Open",
        &[
            EntryStatus::Draft,
            EntryStatus::Proposed,
            EntryStatus::Pending,
            EntryStatus::Approved,
        ],
    ),
    (
        "Active",
        &[
            EntryStatus::Active,
            EntryStatus::InProgress,
            EntryStatus::Blocked,
        ],
    ),
    ("Resolved", &[EntryStatus::Accepted, EntryStatus::Completed]),
    (
        "Closed",
        &[
            EntryStatus::Rejected,
            EntryStatus::Superseded,
            EntryStatus::Abandoned,
            EntryStatus::Archived,
        ],
    ),
];

fn parse_library_status_filters(
    query: &str,
) -> Result<(Vec<EntryStatus>, Vec<EntryStatus>), BelayError> {
    let include = parse_optional_status(query, "status")?;
    let exclude = parse_optional_status(query, "status_exclude")?;
    if query_value(query, "status_mode").as_deref() == Some("exclude") && exclude.is_empty() {
        return Ok((Vec::new(), include));
    }
    Ok((include, exclude))
}

fn parse_optional_status(query: &str, key: &str) -> Result<Vec<EntryStatus>, BelayError> {
    query_value(query, key)
        .filter(|value| !value.is_empty())
        .map(|value| value.parse().map(|status| vec![status]))
        .transpose()
        .map(|value| value.unwrap_or_default())
}

fn grouped_status_select(name: &str, empty_label: &str, selected: Option<&str>) -> String {
    let mut output = format!(
        "<select name=\"{}\" aria-label=\"{}\"><option value=\"{}\">{}</option>",
        escape(name),
        escape(empty_label),
        "",
        escape(empty_label)
    );
    for (group, statuses) in STATUS_GROUPS {
        output.push_str(&format!("<optgroup label=\"{}\">", escape(group)));
        for status in *statuses {
            let value = status.to_string();
            output.push_str(&format!(
                "<option value=\"{}\"{}>{}</option>",
                escape(&value),
                if selected == Some(value.as_str()) {
                    " selected"
                } else {
                    ""
                },
                escape(&value)
            ));
        }
        output.push_str("</optgroup>");
    }
    output.push_str("</select>");
    output
}

fn select(name: &str, selected: Option<&str>, values: &[&str]) -> String {
    let mut output = format!(
        "<select name=\"{}\"><option value=\"\">Any {}</option>",
        escape(name),
        escape(name)
    );
    for value in values.iter().filter(|value| !value.is_empty()) {
        output.push_str(&format!(
            "<option value=\"{}\"{}>{}</option>",
            escape(value),
            if selected == Some(*value) {
                " selected"
            } else {
                ""
            },
            escape(value)
        ));
    }
    output.push_str("</select>");
    output
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn sanitizer_removes_active_content_images_and_dangerous_urls() {
        let html = render_markdown(
            "<script>alert(1)</script>\n\n<img src=x onerror=alert(2)>\n\n<div id=graph>raw</div>\n\n<details>raw</details>\n\n[bad](javascript:alert(3)) [ok](https://example.com)",
            false,
            false,
            None,
        );
        assert!(!html.contains("<script"));
        assert!(!html.contains("<img"));
        assert!(!html.contains("href=\"javascript:"));
        assert!(html.contains("https://example.com"));
        assert!(!html.contains("onerror"));
        assert!(!html.contains("id=\"graph\""));
        assert!(!html.contains("<details"));
    }

    #[test]
    fn goal_success_criteria_receive_canonical_anchors() {
        let rendered = render_markdown(
            "## Success Criteria\n\n- [SC-001] Stable result\n",
            true,
            false,
            None,
        );
        assert!(rendered.contains("id=\"sc-001\""));
        assert!(!rendered.contains("id=\"sc-1\""));
    }

    #[test]
    fn delivery_map_task_and_goal_items_are_links_and_state_is_a_badge() {
        let rendered = render_markdown(
            "## Delivery Map\n\n| ID | Goal item | State |\n| --- | --- | --- |\n| T-001 | SC-001..SC-003 | in-progress |\n",
            false,
            true,
            Some("GOAL-20260712T000000-001-example"),
        );
        assert!(rendered.contains("id=\"t-001\""));
        assert!(!rendered.contains("id=\"task-t-001\""));
        assert!(rendered.contains("href=\"#t-001\""));
        assert!(rendered.contains("href=\"/entries/GOAL-20260712T000000-001-example#sc-001\""));
        assert!(rendered.contains("href=\"/entries/GOAL-20260712T000000-001-example#sc-003\""));
        assert!(rendered.contains("class=\"badge status-in-progress\""));
    }

    #[test]
    fn explicit_goal_ids_do_not_emit_legacy_aliases() {
        let rendered = render_markdown(
            "## Success Criteria\n\n- [SC-001] Stable result\n",
            true,
            false,
            None,
        );
        assert!(rendered.contains("id=\"sc-001\""));
        assert!(!rendered.contains("id=\"sc-1\""));
    }

    #[test]
    fn fully_qualified_references_are_linked_but_code_is_not() {
        let rendered = render_markdown(
            "See GOAL-20260723T120000-001-safe-sync#SC-001 and \
             EVD-20260723T120500-001.\n\n\
             `GOAL-20260723T120000-001-safe-sync#sc-001`",
            false,
            false,
            None,
        );
        assert!(rendered.contains("href=\"/entries/GOAL-20260723T120000-001-safe-sync#sc-001\""));
        assert!(rendered.contains(">GOAL-20260723T120000-001-safe-sync#SC-001</a>"));
        assert!(rendered.contains("href=\"/evidence/EVD-20260723T120500-001\""));
        assert_eq!(rendered.matches("href=\"/entries/").count(), 1);
    }

    #[test]
    fn url_codec_does_not_turn_path_data_into_routing_segments() {
        assert_eq!(decode("a%2Fb").unwrap(), "a/b");
        assert_eq!(encode_segment("a/b"), "a%2Fb");
    }

    #[test]
    fn library_status_filter_lists_every_status_and_honors_exclude_mode() {
        let temporary = tempdir().expect("create temporary repository");
        fs::create_dir(temporary.path().join(".git")).expect("create repository marker");
        let repository = crate::repository::initialize(temporary.path())
            .expect("initialize")
            .repository;
        let proposed = crate::store::create(
            &repository,
            EntryType::Decision,
            "Still proposed".to_owned(),
            "proposed body".to_owned(),
        )
        .expect("create proposed");
        let accepted = crate::store::create(
            &repository,
            EntryType::Decision,
            "Now accepted".to_owned(),
            "accepted body".to_owned(),
        )
        .expect("create accepted");
        crate::store::set_status(&repository, &accepted.display_id, EntryStatus::Accepted)
            .expect("accept");
        let state = BrowseState::new(repository).expect("browse state");

        let form = response_html(state.library("").expect("empty library"));
        assert!(!form.contains("name=\"tag\""));
        assert!(form.contains("name=\"status\""));
        assert!(form.contains("name=\"status_exclude\""));
        assert!(form.contains("optgroup label=\"Open\""));
        assert!(form.contains("optgroup label=\"Active\""));
        assert!(form.contains("optgroup label=\"Resolved\""));
        assert!(form.contains("optgroup label=\"Closed\""));
        for status in EntryStatus::ALL {
            assert!(
                form.contains(&format!("value=\"{status}\"")),
                "missing status option: {status}"
            );
        }

        let excluded = response_html(
            state
                .library("status_exclude=proposed")
                .expect("exclude proposed"),
        );
        assert!(excluded.contains("Search results"));
        assert!(excluded.contains("Now accepted"));
        assert!(!excluded.contains("Still proposed"));
        assert!(excluded.contains(&accepted.display_id));
        assert!(!excluded.contains(&proposed.display_id));
        assert!(excluded.contains("value=\"proposed\" selected"));
    }

    fn response_html(response: HttpResponse) -> String {
        String::from_utf8(response.into_reader().into_inner()).expect("utf-8 body")
    }

    #[test]
    fn snapshot_is_read_only_and_search_matches_the_cli_connection_path() {
        let temporary = tempdir().expect("create temporary repository");
        fs::create_dir(temporary.path().join(".git")).expect("create repository marker");
        let repository = crate::repository::initialize(temporary.path())
            .expect("initialize")
            .repository;
        crate::store::create(
            &repository,
            EntryType::Decision,
            "Use atomic snapshots".to_owned(),
            "The browser searches SQLite with BM25.".to_owned(),
        )
        .expect("create entry");
        let goal = crate::store::create(
            &repository,
            EntryType::Goal,
            "Start exploration from intent".to_owned(),
            "## Summary\n\nStart from Goals.\n\n## Success Criteria\n\n- Goal is visible.\n\n## Constraints\n\n- Stay read-only.\n\n## Non-goals\n\n- Editing.\n\n## Verification\n\n- Inspect graph.\n\n## Risks\n\n- Empty graph.\n".to_owned(),
        )
        .expect("create goal");
        let database_before = fs::read(repository.database_path()).expect("read database before");
        let snapshot = Snapshot::build(&repository).expect("build snapshot");
        let query_only = snapshot
            .connection
            .query_row("PRAGMA query_only", [], |row| row.get::<_, i64>(0))
            .expect("read query_only state");
        assert_eq!(query_only, 1);
        assert!(
            snapshot
                .connection
                .execute("DELETE FROM entries", [])
                .is_err(),
            "snapshot connection must reject writes"
        );
        let graph = graph_neighbors(
            &snapshot.connection,
            &repository.root,
            "all",
            &snapshot.allowed_commits,
        )
        .expect("load initial graph");
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[0].data["entry_type"], "goal");
        assert_eq!(
            graph.nodes[0].data["id"],
            format!("entry:{}", goal.display_id)
        );
        assert!(graph.edges.is_empty());
        let request = SearchRequest {
            query: "browser BM25".to_owned(),
            entry_type: None,
            status_include: Vec::new(),
            status_exclude: Vec::new(),
            tag: None,
            display_id: None,
            limit: 20,
        };
        let cli = search::search(&repository, &request).expect("search source database");
        let browse =
            search::search_connection(&snapshot.connection, &repository.database_path(), &request)
                .expect("search snapshot");
        assert_eq!(
            cli.iter().map(|item| &item.display_id).collect::<Vec<_>>(),
            browse
                .iter()
                .map(|item| &item.display_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            fs::read(repository.database_path()).expect("read database after"),
            database_before
        );
    }
}
