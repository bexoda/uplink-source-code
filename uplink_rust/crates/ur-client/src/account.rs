//! Uplink accounts: optional sign-in (Google, GitHub, Discord, or the local dev provider) against
//! the uplink_rust server, the account's agent roster, and background upload of verified play.
//!
//! The protocol is the server's (README, "Client login flow"): PKCE-style `client_verifier`, the
//! system browser for the provider (never an embedded view), a one-time `login_code`, then our
//! own short-lived access token and rotating refresh token.
//! - Desktop: the browser is sent back to a one-shot loopback listener on 127.0.0.1.
//! - Web: the page itself navigates to the provider and comes back with `?login_code=`.
//!
//! Everything is asynchronous (`ehttp`) and reported through a channel that the UI drains once a
//! frame, so nothing here blocks drawing. Playing never needs the network: local saves are always
//! written, and uploads catch up whenever the server is reachable.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ur_sim::replay::{Run, Segment};

/// Seconds between background uploads while an online agent is played.
const SYNC_EVERY: f64 = 20.0;

#[derive(Debug, Clone, Deserialize)]
pub struct AgentRow {
    pub name: String,
    pub seed: String,
    pub verified_tick: i64,
    #[serde(default)]
    pub credits: Option<i64>,
    #[serde(default)]
    pub game_over: bool,
}

impl AgentRow {
    pub fn seed(&self) -> Option<u64> {
        self.seed.parse().ok()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Tokens {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    handle: Option<String>,
    needs_handle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Stored {
    server: String,
    refresh_token: String,
    handle: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// No server configured: accounts are unavailable; local play only.
    Off,
    SignedOut,
    /// Waiting for the player to finish signing in with `provider` in their browser.
    Waiting {
        provider: String,
    },
    /// Exchanging a login code or refreshing a stored session.
    Connecting,
    SignedIn {
        handle: Option<String>,
    },
}

/// Results of background requests, applied on the UI thread.
enum Msg {
    Providers(Result<Vec<String>, String>),
    /// The desktop loopback listener's result (the web build gets its code from the page URL).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    LoginCode(Result<String, String>),
    Tokens(Result<Tokens, String>, /*from refresh*/ bool),
    Handle(Result<String, String>),
    Agents(Result<Vec<AgentRow>, String>),
    Created(Result<AgentRow, String>),
    Retired(String),
    Synced {
        agent: String,
        seg: Segment,
        result: Result<(), SyncError>,
    },
    State {
        agent: String,
        seed: u64,
        result: Result<Option<Box<ur_sim::World>>, String>,
    },
}

#[derive(Debug)]
enum SyncError {
    /// The server's verified run is not where ours starts: another device played it.
    Conflict,
    /// The server refused the log (illegal, or too fast for real time).
    Rejected(String),
    Unauthorized,
    Network,
}

/// What the account layer wants the app to do. Drained once a frame.
#[derive(Debug)]
pub enum Event {
    /// A message for the player (shown in the original message box).
    Notice(String),
    /// The agent being created online has its seed; start its game with this run.
    AgentReady { name: String, seed: u64 },
    /// Creating online failed; the agent starts as a local one instead.
    AgentLocalOnly { name: String, why: String },
    /// The server accepted an upload: drop `seg`'s commands from the live run.
    Acknowledge { agent: String, seg: Segment },
    /// The server refused this agent's play; it continues unverified.
    Unverified { agent: String },
    /// The agent to enter, from the server's verified state (None: never synced, start fresh).
    Resume {
        agent: String,
        seed: u64,
        world: Option<Box<ur_sim::World>>,
    },
    /// Fetching the verified state failed; enter from the local save if there is one.
    ResumeFailed { agent: String, why: String },
}

pub struct Account {
    server: Option<String>,
    pub status: Status,
    pub providers: Vec<String>,
    /// The signed-in account's agents (empty when signed out).
    pub agents: Vec<AgentRow>,
    access: Option<String>,
    refresh: Option<String>,
    /// Seconds (app clock) when the access token should be refreshed.
    refresh_at: f64,
    verifier: Option<String>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    ctx: eframe::egui::Context,
    events: Vec<Event>,
    /// An upload in flight (one at a time).
    syncing: bool,
    last_sync: f64,
    /// Agents whose uploads stopped this session (conflict or rejection).
    stopped: HashMap<String, ()>,
    #[cfg(not(target_arch = "wasm32"))]
    cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

fn random_verifier() -> String {
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Percent-encodes a query value.
fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn err_text(r: &ehttp::Response) -> String {
    let body = r.text().unwrap_or("").trim();
    if body.is_empty() || body.len() > 200 {
        format!("the server answered {} {}", r.status, r.status_text)
    } else {
        body.to_string()
    }
}

/// The server's display name for a provider.
pub fn provider_label(p: &str) -> String {
    match p {
        "google" => "Google".into(),
        "github" => "GitHub".into(),
        "discord" => "Discord".into(),
        "dev" => "Test account".into(),
        other => other.to_string(),
    }
}

impl Account {
    /// `server` is the uplink_rust server base URL, if one is configured.
    pub fn new(ctx: &eframe::egui::Context, server: Option<String>) -> Account {
        let (tx, rx) = channel();
        let server = server
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty());
        let mut a = Account {
            status: if server.is_some() {
                Status::SignedOut
            } else {
                Status::Off
            },
            server,
            providers: Vec::new(),
            agents: Vec::new(),
            access: None,
            refresh: None,
            refresh_at: f64::INFINITY,
            verifier: None,
            tx,
            rx,
            ctx: ctx.clone(),
            events: Vec::new(),
            syncing: false,
            last_sync: 0.0,
            stopped: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            cancel: None,
        };
        if a.server.is_some() {
            a.fetch_providers();
            // A page coming back from the provider carries a login code; otherwise resume a
            // stored session.
            if !a.take_web_return() {
                if let Some(st) = store::load().filter(|s| Some(&s.server) == a.server.as_ref()) {
                    a.refresh = Some(st.refresh_token);
                    a.status = Status::Connecting;
                    a.do_refresh();
                }
            }
        }
        a
    }

    pub fn configured(&self) -> bool {
        self.server.is_some()
    }

    pub fn signed_in(&self) -> bool {
        matches!(self.status, Status::SignedIn { .. })
    }

    pub fn needs_handle(&self) -> bool {
        matches!(self.status, Status::SignedIn { handle: None })
    }

    /// The account agent with this name.
    pub fn agent(&self, name: &str) -> Option<&AgentRow> {
        self.agents.iter().find(|a| a.name == name)
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.server.as_deref().unwrap_or(""))
    }

    fn send(
        &self,
        req: ehttp::Request,
        done: impl 'static + Send + FnOnce(ehttp::Result<ehttp::Response>) -> Msg,
    ) {
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        ehttp::fetch(req, move |r| {
            let _ = tx.send(done(r));
            ctx.request_repaint();
        });
    }

    fn authed(&self, req: ehttp::Request) -> ehttp::Request {
        match &self.access {
            Some(t) => req.with_header("Authorization", format!("Bearer {t}")),
            None => req,
        }
    }

    fn fetch_providers(&self) {
        #[derive(Deserialize)]
        struct P {
            providers: Vec<String>,
        }
        self.send(ehttp::Request::get(self.url("/v1/auth/providers")), |r| {
            Msg::Providers(match r {
                Ok(r) if r.ok => r
                    .json::<P>()
                    .map(|p| p.providers)
                    .map_err(|e| e.to_string()),
                Ok(r) => Err(err_text(&r)),
                Err(e) => Err(e),
            })
        });
    }

    // ------------------------------------------------------------------ sign in / out

    /// Opens the provider's sign-in in the system browser.
    pub fn sign_in(&mut self, provider: &str) {
        if self.server.is_none() || matches!(self.status, Status::Waiting { .. }) {
            return;
        }
        let verifier = random_verifier();
        let challenge = challenge(&verifier);
        self.verifier = Some(verifier.clone());
        let started = platform::start_sign_in(self, provider, &challenge, &verifier);
        match started {
            Ok(()) => {
                self.status = Status::Waiting {
                    provider: provider.to_string(),
                }
            }
            Err(e) => self.events.push(Event::Notice(e)),
        }
    }

    pub fn cancel_sign_in(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(c) = self.cancel.take() {
            c.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.verifier = None;
        self.status = Status::SignedOut;
    }

    pub fn sign_out(&mut self) {
        if let Some(t) = self.access.take() {
            let req = ehttp::Request::post(self.url("/v1/auth/logout"), Vec::new())
                .with_header("Authorization", format!("Bearer {t}"));
            ehttp::fetch(req, |_| {});
        }
        self.refresh = None;
        self.agents.clear();
        self.status = Status::SignedOut;
        store::clear();
    }

    /// Web: if this page load is the return from the provider, exchange the code. Returns true
    /// when it was.
    fn take_web_return(&mut self) -> bool {
        match platform::web_return() {
            Some(Ok((code, verifier))) => {
                self.verifier = Some(verifier);
                self.status = Status::Connecting;
                self.exchange(code);
                true
            }
            Some(Err(e)) => {
                self.events.push(Event::Notice(e));
                false
            }
            None => false,
        }
    }

    fn exchange(&mut self, code: String) {
        let Some(verifier) = self.verifier.take() else {
            self.status = Status::SignedOut;
            return;
        };
        #[derive(Serialize)]
        struct Ex<'a> {
            login_code: &'a str,
            client_verifier: &'a str,
        }
        let Ok(req) = ehttp::Request::post_json(
            self.url("/v1/auth/exchange"),
            &Ex {
                login_code: &code,
                client_verifier: &verifier,
            },
        ) else {
            return;
        };
        self.send(req, |r| Msg::Tokens(tokens(r), false));
    }

    fn do_refresh(&mut self) {
        let Some(rt) = self.refresh.clone() else {
            return;
        };
        self.refresh_at = f64::INFINITY;
        #[derive(Serialize)]
        struct R<'a> {
            refresh_token: &'a str,
        }
        let Ok(req) =
            ehttp::Request::post_json(self.url("/v1/auth/refresh"), &R { refresh_token: &rt })
        else {
            return;
        };
        self.send(req, |r| Msg::Tokens(tokens(r), true));
    }

    /// Chooses the hacker handle shown to other players. Set once per account.
    pub fn set_handle(&mut self, handle: &str) {
        #[derive(Serialize)]
        struct H<'a> {
            handle: &'a str,
        }
        let Ok(req) = ehttp::Request::put_json(self.url("/v1/me/handle"), &H { handle }) else {
            return;
        };
        let h = handle.to_string();
        self.send(self.authed(req), move |r| {
            Msg::Handle(match r {
                Ok(r) if r.ok => Ok(h),
                Ok(r) if r.status == 409 => Err("That handle is taken. Choose another.".into()),
                Ok(r) if r.status == 400 => {
                    Err("A handle is 3-20 letters, digits or underscores.".into())
                }
                Ok(r) => Err(err_text(&r)),
                Err(e) => Err(e),
            })
        });
    }

    // ------------------------------------------------------------------ agents

    pub fn refresh_agents(&self) {
        self.send(
            self.authed(ehttp::Request::get(self.url("/v1/agents"))),
            |r| {
                Msg::Agents(match r {
                    Ok(r) if r.ok => r.json::<Vec<AgentRow>>().map_err(|e| e.to_string()),
                    Ok(r) => Err(err_text(&r)),
                    Err(e) => Err(e),
                })
            },
        );
    }

    /// Creates the agent on the account; the answer arrives as `AgentReady` or `AgentLocalOnly`.
    pub fn create_agent(&self, name: &str) {
        #[derive(Serialize)]
        struct N<'a> {
            name: &'a str,
        }
        let Ok(req) = ehttp::Request::post_json(self.url("/v1/agents"), &N { name }) else {
            return;
        };
        self.send(self.authed(req), |r| {
            Msg::Created(match r {
                Ok(r) if r.ok => r.json::<AgentRow>().map_err(|e| e.to_string()),
                Ok(r) => Err(err_text(&r)),
                Err(e) => Err(e),
            })
        });
    }

    pub fn retire_agent(&self, name: &str) {
        let n = name.to_string();
        let req = self.authed(ehttp::Request::delete(
            &self.url(&format!("/v1/agents/{}", enc(name))),
        ));
        self.send(req, move |_| Msg::Retired(n));
    }

    /// Asks for the verified world of an account agent, to enter it on this device.
    pub fn fetch_state(&self, agent: &str, seed: u64) {
        let a = agent.to_string();
        let req = self.authed(ehttp::Request::get(
            self.url(&format!("/v1/runs/{seed}/state")),
        ));
        self.send(req, move |r| Msg::State {
            agent: a,
            seed,
            result: match r {
                Ok(r) if r.ok => ur_save::decode_state(&r.bytes)
                    .map(|w| Some(Box::new(w)))
                    .map_err(|e| e.to_string()),
                Ok(r) if r.status == 404 => Ok(None),
                Ok(r) => Err(err_text(&r)),
                Err(e) => Err(e),
            },
        });
    }

    // ------------------------------------------------------------------ sync

    /// Called every frame with the agent being played. Uploads its play every `SYNC_EVERY`
    /// seconds when it is an account agent; `force` uploads now (logging off).
    pub fn sync(&mut self, now: f64, agent: &str, run: &Run, force: bool) {
        if !self.signed_in() || self.syncing || self.stopped.contains_key(agent) {
            return;
        }
        let Some(row) = self.agent(agent) else { return };
        if row.seed() != Some(run.seed) {
            return;
        }
        if !force && now - self.last_sync < SYNC_EVERY {
            return;
        }
        let seg = run.segment();
        if seg.end_tick == seg.start_tick && seg.entries.is_empty() {
            return;
        }
        let Ok(body) = ur_save::encode_segment(&seg) else {
            return;
        };
        self.syncing = true;
        self.last_sync = now;
        let a = agent.to_string();
        let req = self.authed(ehttp::Request::put(
            self.url(&format!("/v1/runs/{}/segment", run.seed)),
            body,
        ));
        self.send(req, move |r| Msg::Synced {
            agent: a,
            seg,
            result: match r {
                Ok(r) if r.ok => Ok(()),
                Ok(r) if r.status == 409 => Err(SyncError::Conflict),
                Ok(r) if r.status == 401 => Err(SyncError::Unauthorized),
                Ok(r) if r.status == 422 || r.status == 426 => {
                    Err(SyncError::Rejected(err_text(&r)))
                }
                Ok(_) => Err(SyncError::Network),
                Err(_) => Err(SyncError::Network),
            },
        });
    }

    // ------------------------------------------------------------------ pump

    /// Applies finished requests. Call once per frame with the app clock (seconds).
    pub fn update(&mut self, now: f64) {
        if now >= self.refresh_at {
            self.do_refresh();
        }
        while let Ok(m) = self.rx.try_recv() {
            self.apply(m, now);
        }
    }

    fn apply(&mut self, m: Msg, now: f64) {
        match m {
            Msg::Providers(Ok(p)) => self.providers = p,
            Msg::Providers(Err(_)) => self.providers.clear(),
            Msg::LoginCode(Ok(code)) => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.cancel = None;
                }
                if matches!(self.status, Status::Waiting { .. }) {
                    self.status = Status::Connecting;
                    self.exchange(code);
                }
            }
            Msg::LoginCode(Err(e)) => {
                if matches!(self.status, Status::Waiting { .. }) {
                    self.status = Status::SignedOut;
                    if !e.is_empty() {
                        self.events.push(Event::Notice(e));
                    }
                }
            }
            Msg::Tokens(Ok(t), _) => {
                self.access = Some(t.access_token);
                self.refresh = Some(t.refresh_token.clone());
                // Refresh a minute before the access token runs out.
                self.refresh_at = now + (t.expires_in as f64 - 60.0).max(30.0);
                let handle = if t.needs_handle { None } else { t.handle };
                if let Some(server) = &self.server {
                    store::save(&Stored {
                        server: server.clone(),
                        refresh_token: t.refresh_token,
                        handle: handle.clone(),
                    });
                }
                self.status = Status::SignedIn { handle };
                self.refresh_agents();
            }
            Msg::Tokens(Err(e), from_refresh) => {
                self.access = None;
                self.refresh = None;
                self.agents.clear();
                self.status = Status::SignedOut;
                store::clear();
                self.events.push(Event::Notice(if from_refresh {
                    "Your Uplink account session ended. Sign in again to sync your agents.".into()
                } else {
                    format!("Sign-in failed: {e}")
                }));
            }
            Msg::Handle(Ok(h)) => {
                self.status = Status::SignedIn {
                    handle: Some(h.clone()),
                };
                if let (Some(server), Some(rt)) = (&self.server, &self.refresh) {
                    store::save(&Stored {
                        server: server.clone(),
                        refresh_token: rt.clone(),
                        handle: Some(h),
                    });
                }
            }
            Msg::Handle(Err(e)) => self.events.push(Event::Notice(e)),
            Msg::Agents(Ok(a)) => self.agents = a,
            Msg::Agents(Err(_)) => {}
            Msg::Created(Ok(row)) => {
                let seed = row.seed();
                let name = row.name.clone();
                self.agents.retain(|a| a.name != name);
                self.agents.push(row);
                self.agents.sort_by(|a, b| a.name.cmp(&b.name));
                match seed {
                    Some(seed) => self.events.push(Event::AgentReady { name, seed }),
                    None => self.events.push(Event::AgentLocalOnly {
                        name,
                        why: "the server sent a bad seed".into(),
                    }),
                }
            }
            Msg::Created(Err(why)) => self.events.push(Event::AgentLocalOnly {
                name: String::new(),
                why,
            }),
            Msg::Retired(name) => self.agents.retain(|a| a.name != name),
            Msg::Synced { agent, seg, result } => {
                self.syncing = false;
                match result {
                    Ok(()) => {
                        if let Some(a) = self.agents.iter_mut().find(|a| a.name == agent) {
                            a.verified_tick = seg.end_tick as i64;
                        }
                        self.events.push(Event::Acknowledge { agent, seg })
                    }
                    Err(SyncError::Conflict) => {
                        self.stopped.insert(agent.clone(), ());
                        self.events.push(Event::Notice(format!(
                            "'{agent}' was played further on another device, so this session isn't uploaded. Log off and back in to continue from the latest play."
                        )));
                    }
                    Err(SyncError::Rejected(why)) => {
                        self.stopped.insert(agent.clone(), ());
                        self.events.push(Event::Unverified {
                            agent: agent.clone(),
                        });
                        self.events.push(Event::Notice(format!(
                            "The Uplink server refused '{agent}''s play ({why}). It continues on this device only."
                        )));
                    }
                    Err(SyncError::Unauthorized) => self.do_refresh(),
                    // Offline: try again next round.
                    Err(SyncError::Network) => {}
                }
            }
            Msg::State {
                agent,
                seed,
                result,
            } => match result {
                Ok(world) => self.events.push(Event::Resume { agent, seed, world }),
                Err(why) => self.events.push(Event::ResumeFailed { agent, why }),
            },
        }
    }

    /// After a successful upload, acknowledge it on the live run.
    pub fn acknowledge(run: &mut Run, seg: &Segment) {
        run.acknowledge(seg);
    }

    /// Forget that uploads stopped for an agent (a fresh session after logging back in).
    pub fn reset_stopped(&mut self, agent: &str) {
        self.stopped.remove(agent);
    }
}

fn tokens(r: ehttp::Result<ehttp::Response>) -> Result<Tokens, String> {
    match r {
        Ok(r) if r.ok => r.json::<Tokens>().map_err(|e| e.to_string()),
        Ok(r) if r.status == 401 => Err("the sign-in expired or was already used".into()),
        Ok(r) => Err(err_text(&r)),
        Err(e) => Err(format!("cannot reach the Uplink server ({e})")),
    }
}

// ---------------------------------------------------------------------- platform: desktop

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::{enc, Account, Msg};

    /// The page the browser shows after coming back to the game.
    const DONE_PAGE: &str = r#"<!doctype html><html><head><meta charset="utf-8"><title>Uplink</title>
<style>body{margin:0;height:100vh;display:grid;place-items:center;background:#000;color:#b0c0f0;font:14px ui-monospace,Menlo,Consolas,monospace}
div{background:#00003a;border:1px solid #3050c0;padding:18px 22px;max-width:360px}b{color:#fff}</style></head>
<body><div><b>MESSAGE</b><p>You can close this tab and return to Uplink.</p></div></body></html>"#;

    /// Desktop: listen on a loopback port, open the system browser, catch the redirect.
    pub fn start_sign_in(
        a: &mut Account,
        provider: &str,
        challenge: &str,
        _verifier: &str,
    ) -> Result<(), String> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let return_to = format!("http://127.0.0.1:{port}/cb");
        let url = a.url(&format!(
            "/v1/auth/{}/start?return_to={}&client_challenge={}",
            enc(provider),
            enc(&return_to),
            challenge
        ));
        let cancel = Arc::new(AtomicBool::new(false));
        a.cancel = Some(cancel.clone());
        let tx = a.tx.clone();
        let ctx = a.ctx.clone();
        std::thread::spawn(move || {
            let r = wait_for_code(listener, &cancel);
            let _ = tx.send(Msg::LoginCode(r));
            ctx.request_repaint();
        });
        // Without a browser the listener still waits: the player can open the address by hand.
        if webbrowser::open(&url).is_err() {
            a.events.push(super::Event::Notice(format!(
                "Couldn't open a web browser. Open this address to sign in: {url}"
            )));
        }
        Ok(())
    }

    fn wait_for_code(listener: TcpListener, cancel: &AtomicBool) -> Result<String, String> {
        let deadline = Instant::now() + Duration::from_secs(600);
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(String::new());
            }
            if Instant::now() > deadline {
                return Err("Sign-in timed out.".into());
            }
            match listener.accept() {
                Ok((mut s, _)) => {
                    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
                    let mut buf = [0u8; 4096];
                    let n = s.read(&mut buf).unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let path = req.split_whitespace().nth(1).unwrap_or("");
                    let Some(query) = path.strip_prefix("/cb?") else {
                        let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
                        continue;
                    };
                    let mut code = None;
                    let mut error = None;
                    for kv in query.split('&') {
                        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
                        match k {
                            "login_code" => code = Some(v.to_string()),
                            "error" => error = Some(v.to_string()),
                            _ => {}
                        }
                    }
                    let msg = if code.is_some() {
                        "Signed in."
                    } else {
                        "Sign-in was cancelled."
                    };
                    let body = super::platform::DONE_PAGE.replace("MESSAGE", msg);
                    let _ = s.write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    );
                    return match (code, error) {
                        (Some(c), _) => Ok(c),
                        (None, Some(e)) if e == "access_denied" => Err(String::new()),
                        (None, Some(e)) => Err(format!("Sign-in failed ({e}).")),
                        (None, None) => Err("Sign-in failed.".into()),
                    };
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => return Err(e.to_string()),
            }
        }
    }

    pub fn web_return() -> Option<Result<(String, String), String>> {
        None
    }
}

// ---------------------------------------------------------------------- platform: browser

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::{enc, Account};

    const PKCE_KEY: &str = "uplink_rust.pkce";

    /// Web: remember the verifier for this tab, then send the whole page to the provider.
    pub fn start_sign_in(
        a: &mut Account,
        provider: &str,
        challenge: &str,
        verifier: &str,
    ) -> Result<(), String> {
        let w = web_sys::window().ok_or("no window")?;
        let loc = w.location();
        let here = format!(
            "{}{}",
            loc.origin().map_err(|_| "no origin")?,
            loc.pathname().map_err(|_| "no path")?
        );
        w.session_storage()
            .ok()
            .flatten()
            .ok_or("this browser blocks session storage")?
            .set_item(PKCE_KEY, verifier)
            .map_err(|_| "this browser blocks session storage")?;
        let url = a.url(&format!(
            "/v1/auth/{}/start?return_to={}&client_challenge={}",
            enc(provider),
            enc(&here),
            challenge
        ));
        loc.assign(&url)
            .map_err(|_| "could not open the sign-in page".to_string())
    }

    /// If the page URL carries `login_code` (or `error`), take it and the stored verifier and
    /// clean the address bar.
    pub fn web_return() -> Option<Result<(String, String), String>> {
        let w = web_sys::window()?;
        let search = w.location().search().ok()?;
        let q = search.strip_prefix('?')?;
        let mut code = None;
        let mut error = None;
        for kv in q.split('&') {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            match k {
                "login_code" => code = Some(v.to_string()),
                "error" => error = Some(v.to_string()),
                _ => {}
            }
        }
        if code.is_none() && error.is_none() {
            return None;
        }
        if let (Ok(path), Ok(h)) = (w.location().pathname(), w.history()) {
            let _ = h.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&path));
        }
        let ss = w.session_storage().ok().flatten();
        let verifier = ss
            .as_ref()
            .and_then(|s| s.get_item(PKCE_KEY).ok().flatten());
        if let Some(s) = &ss {
            let _ = s.remove_item(PKCE_KEY);
        }
        match (code, verifier, error) {
            (Some(c), Some(v), _) => Some(Ok((c, v))),
            (Some(_), None, _) => Some(Err(
                "Sign-in must finish in the same tab it started in. Try again.".into(),
            )),
            (None, _, Some(e)) if e == "access_denied" => None,
            (None, _, Some(e)) => Some(Err(format!("Sign-in failed ({e})."))),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------- session storage

/// Where the refresh token lives between runs.
/// Desktop: a file next to the saves, readable only by the user (or the OS keychain when built
/// with `os-keyring`). Browser: localStorage. Either way it is a bearer secret for this
/// device; signing out deletes it and revokes the session on the server.
mod store {
    use super::Stored;

    #[cfg(not(target_arch = "wasm32"))]
    fn path() -> Option<std::path::PathBuf> {
        crate::persist::data_dir().map(|d| d.join("account.json"))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load() -> Option<Stored> {
        serde_json::from_slice(&std::fs::read(path()?).ok()?).ok()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(s: &Stored) {
        let Some(p) = path() else { return };
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let Ok(json) = serde_json::to_vec(s) else {
            return;
        };
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
            }
            let _ = std::fs::rename(&tmp, &p);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn clear() {
        if let Some(p) = path() {
            let _ = std::fs::remove_file(p);
        }
    }

    #[cfg(target_arch = "wasm32")]
    const KEY: &str = "uplink_rust.account";

    #[cfg(target_arch = "wasm32")]
    fn ls() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load() -> Option<Stored> {
        serde_json::from_str(&ls()?.get_item(KEY).ok()??).ok()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn save(s: &Stored) {
        if let (Some(l), Ok(j)) = (ls(), serde_json::to_string(s)) {
            let _ = l.set_item(KEY, &j);
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn clear() {
        if let Some(l) = ls() {
            let _ = l.remove_item(KEY);
        }
    }
}

/// The server to use: `UPLINK_RUST_SERVER` on desktop; on the web, the page's
/// `<meta name="uplink-rust-server" content="...">`.
pub fn configured_server() -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("UPLINK_RUST_SERVER").ok()
    }
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let doc = web_sys::window()?.document()?;
        let meta = doc
            .query_selector("meta[name=uplink-rust-server]")
            .ok()??
            .dyn_into::<web_sys::HtmlMetaElement>()
            .ok()?;
        Some(meta.content())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_is_s256_base64url() {
        let c = challenge("abc");
        assert_eq!(c.len(), 43);
        assert_eq!(c, "ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0");
        assert_eq!(random_verifier().len(), 64);
        assert_ne!(random_verifier(), random_verifier());
    }

    #[test]
    fn query_values_are_encoded() {
        assert_eq!(
            enc("http://127.0.0.1:5/cb"),
            "http%3A%2F%2F127.0.0.1%3A5%2Fcb"
        );
        assert_eq!(enc("neo_1-x.y~"), "neo_1-x.y~");
    }
}
