//! `umayu.exe --mcp [folder]`: the agent's tools as an MCP server over stdio (newline-delimited
//! JSON-RPC 2.0: `initialize`, `tools/list`, `tools/call`, `ping`). It runs a headless `App` on the
//! UmaYu folder, so OpenCode (and Claude Code or Claude Desktop) get the same tools as the Claude
//! API backend.
//!
//! Long tools run to the end before the call returns (under 50 s, OpenCode's request timeout is
//! 60 s). Write tools ask the window when one is attached (`UMALORD_AGENT_DIR`, `bridge.rs`);
//! without a window the MCP client asks the user itself, as Claude Code does for every tool.

use std::io::{BufRead, Write};
use std::time::{Duration, Instant};

use eframe::egui;
use serde_json::{Value, json};

use super::{Exec, ToolOut};
use crate::App;

const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const LONG_LIMIT: Duration = Duration::from_secs(50);

const INSTRUCTIONS: &str = "UmaYu's tools for one Uma Musume: Pretty Derby account (Global server): the roster, uma.guide's game data, \
the community PvP meta sheet, the live Champions Meeting schedule, a career planner and Moomoolator's race simulator. \
Draft tools change the server's working plan and Race sim setup; save_plan, log_result and update_owned write files and ask the user first. \
Label claims as data, simulation, community meta or opinion.";

/// Serve until stdin closes. `make` builds the app on the first tool call (loading the game data
/// takes a moment, and `initialize` and `tools/list` don't need it).
pub(crate) fn serve(
    input: impl BufRead,
    mut output: impl Write,
    make: impl FnOnce() -> App,
) -> std::io::Result<()> {
    let mut make = Some(make);
    let mut app: Option<App> = None;
    let ctx = egui::Context::default();
    let mut last_call = Instant::now();
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                writeln!(
                    output,
                    "{}",
                    json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("Parse error: {e}")}})
                )?;
                output.flush()?;
                continue;
            }
        };
        let id = msg.get("id").cloned();
        let method = msg["method"].as_str().unwrap_or_default();
        let result: Result<Value, (i64, String)> = match method {
            "initialize" => {
                let asked = msg["params"]["protocolVersion"]
                    .as_str()
                    .unwrap_or_default();
                let version = VERSIONS
                    .iter()
                    .find(|v| **v == asked)
                    .copied()
                    .unwrap_or(VERSIONS[1]);
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "umalord", "title": "umaLord", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": INSTRUCTIONS,
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": super::mcp_tools()})),
            "tools/call" => {
                let name = msg["params"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let args = msg["params"]["arguments"].clone();
                if super::find(&name).is_none() {
                    Err((-32602, format!("Unknown tool: {name}")))
                } else {
                    let a = app.get_or_insert_with(|| {
                        let mut a = make.take().expect("made once")();
                        a.agent.mcp = true;
                        a
                    });
                    // A long-lived client (Claude Desktop): the per-question simulation limit
                    // starts again after a quiet spell.
                    if last_call.elapsed() > Duration::from_secs(180) {
                        a.agent.sims = 0;
                    }
                    last_call = Instant::now();
                    let out = call(a, &ctx, &name, &args);
                    // The card's line first: OpenCode's chat card shows a result's first line.
                    let first = out.text.lines().next().unwrap_or_default();
                    let text = if out.card.summary.is_empty()
                        || first.starts_with(out.card.summary.trim_end_matches('…'))
                    {
                        out.text
                    } else {
                        format!("{}\n\n{}", out.card.summary, out.text)
                    };
                    Ok(
                        json!({"content": [{"type": "text", "text": text}], "isError": out.is_error}),
                    )
                }
            }
            _ if id.is_none() => continue, // notifications/initialized, cancellations: nothing to say
            _ => Err((-32601, format!("Method not found: {method}"))),
        };
        let Some(id) = id else { continue };
        let reply = match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((code, message)) => {
                json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
            }
        };
        writeln!(output, "{reply}")?;
        output.flush()?;
    }
    Ok(())
}

/// One tool call, run to the end.
fn call(app: &mut App, ctx: &egui::Context, name: &str, args: &Value) -> ToolOut {
    match app.agent_call_blocking(ctx, name, args, LONG_LIMIT) {
        Exec::Done(out) => out,
        Exec::Wait(_) => ToolOut::err("The simulation didn't finish."),
        Exec::Ask(a) => {
            let name = super::find(name).map_or(name, |s| s.name);
            match app.agent.bridge.clone() {
                Some(dir) => match super::bridge::ask_window(&dir, name, &a.question) {
                    Ok(true) => app.agent_write(name, args),
                    Ok(false) => ToolOut::err("The user declined."),
                    Err(e) => ToolOut::err(e),
                },
                // No window: the MCP client asked the user before calling.
                None => app.agent_write(name, args),
            }
        }
    }
}

/// The app for `--mcp`: the folder's game data and account, the bridge folder if the window
/// passed one, and the last question's draft.
pub(crate) fn headless_app(root: Option<std::path::PathBuf>) -> App {
    let mut app = App::new(root);
    app.onboarding.open = false;
    app.agent.bridge = std::env::var_os(super::bridge::ENV)
        .map(std::path::PathBuf::from)
        .filter(|d| d.is_dir() || std::fs::create_dir_all(d).is_ok());
    app.load_bridge_draft();
    app
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The handshake, the tool list and a tools/call round trip over in-memory stdio.
    #[test]
    fn mcp_handshake_and_calls() {
        let lines = [
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "get_account_summary", "arguments": {}}}),
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "find_umas", "arguments": {"distance": "Long", "style": "Pace"}}}),
            json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "get_uma", "arguments": {"nope": 1}}}),
            json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "no_such_tool", "arguments": {}}}),
            json!({"jsonrpc": "2.0", "id": 7, "method": "no/such/method"}),
        ];
        let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
        let mut out = Vec::new();
        serve(std::io::Cursor::new(input), &mut out, App::for_tests).unwrap();
        let replies: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(
            replies.len(),
            7,
            "one reply per request, none for the notification"
        );
        assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(replies[0]["result"]["serverInfo"]["name"], "umalord");
        let tools = replies[1]["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), super::super::registry().len());
        assert!(
            tools
                .iter()
                .all(|t| t["inputSchema"]["type"] == "object" && t["description"].is_string())
        );
        let summary = replies[2]["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            summary.contains("support card") && replies[2]["result"]["isError"] == false,
            "{summary}"
        );
        assert!(
            replies[3]["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Narita Brian")
        );
        assert_eq!(
            replies[4]["result"]["isError"], true,
            "bad input is a tool error the model can fix"
        );
        assert!(
            replies[4]["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Invalid input")
        );
        assert_eq!(replies[5]["error"]["code"], -32602);
        assert_eq!(replies[6]["error"]["code"], -32601);
    }
}
