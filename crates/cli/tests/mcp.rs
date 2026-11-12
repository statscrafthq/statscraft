//! `statscraft mcp` end to end: the real binary, spoken to in JSON-RPC over its standard input
//! and output, as an agent's client does.

use serde_json::{Value, json};
use statscraft_engine::live;
use statscraft_engine::session::Session;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Workdir(PathBuf);

impl Workdir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("statscraft-mcp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    /// Where the server looks for a window.
    fn endpoint(&self) -> PathBuf {
        self.0.join("runtime/statscraft/gui.json")
    }
}

impl Drop for Workdir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Client {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    /// Starts the server in `cwd`, with the home, data and runtime folders inside `dir`, and
    /// initializes the connection.
    fn start(dir: &Workdir, cwd: &Path) -> (Self, Value) {
        std::fs::create_dir_all(dir.0.join("home")).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_statscraft"))
            .arg("mcp")
            .current_dir(cwd)
            .env("HOME", dir.0.join("home"))
            .env("USERPROFILE", dir.0.join("home"))
            .env("XDG_DATA_HOME", dir.0.join("data"))
            .env("LOCALAPPDATA", dir.0.join("data"))
            .env("XDG_RUNTIME_DIR", dir.0.join("runtime"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut client = Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
            id: 0,
        };
        let init = client.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1" }
            }),
        );
        client.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        (client, init)
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.input, "{message}").unwrap();
        self.input.flush().unwrap();
    }

    /// Sends a request and returns its result, skipping notifications. Every line on standard
    /// output must be a protocol message.
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        let id = self.id;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            assert!(
                self.output.read_line(&mut line).unwrap() > 0,
                "server closed"
            );
            let message: Value = serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("not JSON-RPC on standard output ({e}): {line}"));
            assert_eq!(message["jsonrpc"], "2.0");
            if message["id"] == id {
                assert!(message["error"].is_null(), "{method}: {message}");
                return message["result"].clone();
            }
        }
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        self.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        )
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn fixture(name: &str) -> String {
    format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn text(result: &Value) -> &str {
    result["content"][0]["text"].as_str().unwrap()
}

#[test]
fn initialize_lists_the_tools_with_their_hints_and_the_prompt() {
    let dir = Workdir::new("list");
    let (mut client, init) = Client::start(&dir, &dir.0);
    assert_eq!(init["serverInfo"]["name"], "statscraft");
    assert!(init["instructions"].as_str().unwrap().contains("describe"));
    let tools = client.request("tools/list", json!({}))["tools"].clone();
    let tool = |name: &str| {
        tools
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("no tool {name}"))
            .clone()
    };
    for name in [
        "describe",
        "list_commands",
        "command_help",
        "check_syntax",
        "window_status",
        "list_syntax_windows",
        "read_syntax",
    ] {
        let t = tool(name);
        assert_eq!(t["annotations"]["readOnlyHint"], true, "{name}");
        assert!(
            t["title"].is_string() && t["description"].is_string(),
            "{name}"
        );
    }
    for (name, destructive) in [
        ("run_syntax", true),
        ("save_syntax", true),
        ("edit_syntax", false),
        ("new_syntax_window", false),
        ("open_syntax_file", false),
    ] {
        let t = tool(name);
        assert_eq!(t["annotations"]["readOnlyHint"], false, "{name}");
        assert_eq!(t["annotations"]["destructiveHint"], destructive, "{name}");
    }
    let run = tool("run_syntax");
    let fields = run["inputSchema"]["properties"].as_object().unwrap();
    for field in [
        "syntax", "file", "lines", "session", "folder", "xlsx", "window",
    ] {
        assert!(fields.contains_key(field), "run_syntax lacks {field}");
    }
    assert_eq!(
        tool("command_help")["inputSchema"]["required"],
        json!(["command"])
    );

    let prompts = client.request("prompts/list", json!({}));
    assert_eq!(prompts["prompts"][0]["name"], "analysis");
    let prompt = client.request("prompts/get", json!({ "name": "analysis" }));
    assert!(
        prompt["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("check_syntax")
    );
}

#[test]
fn describe_help_check_and_runs_that_build_on_each_other() {
    let dir = Workdir::new("loop");
    let elsewhere = dir.0.join("client-folder");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let (mut client, _) = Client::start(&dir, &elsewhere);

    let described = client.call("describe", json!({ "file": fixture("survey_basic.sav") }));
    assert_eq!(described["isError"], false);
    assert_eq!(described["structuredContent"]["case_count"], 200);
    assert_eq!(described["structuredContent"]["variable_count"], 19);

    let help = client.call("command_help", json!({ "command": "frequencies" }));
    assert!(text(&help).starts_with("FREQUENCIES: "), "{}", text(&help));
    let list = client.call("list_commands", json!({}));
    assert!(text(&list).contains("RECODE"));
    let unknown = client.call("command_help", json!({ "command": "NO SUCH" }));
    assert_eq!(unknown["isError"], true);

    // a check runs on a copy: nothing is written, not even the session
    let check = client.call(
        "check_syntax",
        json!({
            "syntax": format!("GET FILE='{}'.\nSAVE OUTFILE='copy.sav'.", fixture("survey_basic.sav")),
            "session": "t1",
            "folder": dir.0.to_str().unwrap(),
        }),
    );
    assert_eq!(check["structuredContent"]["ok"], true, "{check}");
    assert!(!dir.0.join("copy.sav").exists());
    // the server's per-user folder: Statscraft under LOCALAPPDATA, statscraft under XDG_DATA_HOME
    let name = if cfg!(windows) {
        "Statscraft"
    } else {
        "statscraft"
    };
    let sessions = dir.0.join("data").join(name).join("sessions");
    assert!(!sessions.join("t1").exists());

    let syntax = format!(
        "GET FILE='{}'.\nRECODE age (LO THRU 34=1) (ELSE=2) INTO young.\nFREQUENCIES young.",
        fixture("survey_basic.sav")
    );
    let first = client.call("run_syntax", json!({ "syntax": syntax, "session": "t1" }));
    assert_eq!(first["isError"], false);
    let result = &first["structuredContent"];
    assert_eq!(result["ok"], true, "{first}");
    assert_eq!(result["session"], "t1");
    assert_eq!(result["saved"], true);
    assert_eq!(result["dataset"], json!([200, 20]));
    assert!(text(&first).starts_with("ok: true\n"));
    // the session's folder keeps the syntax of every run that worked
    let trail = sessions.join("t1").join("syntax.sps");
    assert_eq!(result["trail"], trail.to_str().unwrap());
    assert!(text(&first).contains(&format!("syntax kept in: {}\n", trail.display())));
    assert!(text(&first).contains("### "), "tables as Markdown");

    let data = statscraft_data::Dataset::read_sav(fixture("survey_basic.sav")).unwrap();
    let ages = data.variable("age").unwrap().numeric().unwrap();
    let young = ages.iter().filter(|a| a.is_some_and(|a| a <= 34.0)).count();

    // the second call builds on the session the first one kept
    let second = client.call(
        "run_syntax",
        json!({ "syntax": "FREQUENCIES VARIABLES=young.", "session": "t1" }),
    );
    let result = &second["structuredContent"];
    assert_eq!(result["ok"], true, "{second}");
    assert_eq!(result["saved"], false);
    let table = result["commands"][0]["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["title"] == "young")
        .unwrap();
    let frequency = table["columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c == "Frequency")
        .unwrap();
    let counts: Vec<&Value> = table["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| &row[frequency])
        .collect();
    assert_eq!(
        counts,
        [
            &json!(young as f64),
            &json!((200 - young) as f64),
            &json!(200.0)
        ]
    );

    // sessions live in the user's data folder, not in the folder the client started us in
    assert!(sessions.join("t1/data.zsav").exists());
    assert!(!elsewhere.join(".statscraft").exists());

    // a syntax error is a normal result the agent reads and fixes
    let error = client.call(
        "run_syntax",
        json!({ "syntax": "FREQUENCIS young.", "session": "t1" }),
    );
    assert_eq!(error["isError"], false);
    let result = &error["structuredContent"];
    assert_eq!(result["ok"], false);
    assert_eq!(result["errors"][0]["kind"], "syntax");
    assert_eq!(result["errors"][0]["line"], 1);
    assert_eq!(result["errors"][0]["column"], 1);
    assert!(text(&error).contains("line 1, column 1"));
    let kept = std::fs::read_to_string(&trail).unwrap();
    assert!(
        kept.starts_with("* Agent 20") && kept.contains(&syntax),
        "{kept}"
    );
    assert!(
        kept.ends_with(".\nFREQUENCIES VARIABLES=young.\n"),
        "{kept}"
    );
    assert!(
        !kept.contains("FREQUENCIS"),
        "a failed run leaves no syntax"
    );

    // bad arguments are a failure of the tool
    let relative = client.call("run_syntax", json!({ "file": "job.sps" }));
    assert_eq!(relative["isError"], true);
    let both = client.call(
        "check_syntax",
        json!({ "syntax": "SHOW.", "file": "/job.sps" }),
    );
    assert_eq!(both["isError"], true);
}

#[test]
fn a_syntax_file_by_lines_and_a_workbook() {
    let dir = Workdir::new("file");
    let job = dir.0.join("job.sps");
    let syntax = format!(
        "GET FILE='{}'.\nCOMPUTE older = age + 10.\nDESCRIPTIVES older.\n",
        fixture("survey_basic.sav")
    );
    std::fs::write(&job, syntax).unwrap();
    let (mut client, _) = Client::start(&dir, &dir.0);
    let file = job.to_str().unwrap();
    let first = client.call(
        "run_syntax",
        json!({ "file": file, "lines": "1-2", "session": "f" }),
    );
    assert_eq!(first["structuredContent"]["dataset"], json!([200, 20]));
    let second = client.call(
        "run_syntax",
        json!({ "file": file, "lines": "3-3", "session": "f", "xlsx": "tables.xlsx", "folder": dir.0.to_str().unwrap() }),
    );
    assert_eq!(
        second["structuredContent"]["commands"][0]["name"],
        "DESCRIPTIVES"
    );
    assert!(dir.0.join("tables.xlsx").exists());
    let described = client.call("describe", json!({ "session": "f" }));
    assert_eq!(described["structuredContent"]["variable_count"], 20);
}

#[test]
fn without_a_session_calls_go_to_the_window_when_one_is_open() {
    let dir = Workdir::new("window");
    let (mut client, _) = Client::start(&dir, &dir.0);
    let status = client.call("window_status", json!({}));
    assert_eq!(status["structuredContent"]["running"], false);
    let run = client.call("run_syntax", json!({ "syntax": "SET SEED=7." }));
    assert_eq!(run["structuredContent"]["session"], "work");

    let mut session = Session::default();
    let _window = live::serve(dir.endpoint(), move |request| {
        live::answer(&mut session, &request)
    })
    .unwrap();
    let syntax = format!("GET FILE='{}'.", fixture("survey_basic.sav"));
    let run = client.call("run_syntax", json!({ "syntax": syntax }));
    assert_eq!(run["structuredContent"]["session"], "gui", "{run}");
    let status = client.call("window_status", json!({}));
    assert_eq!(status["structuredContent"]["running"], true);
    assert_eq!(status["structuredContent"]["window"]["case_count"], 200);
    let described = client.call("describe", json!({}));
    assert_eq!(described["structuredContent"]["case_count"], 200);
}

#[test]
fn syntax_windows_are_read_edited_run_and_saved_and_runs_leave_their_syntax() {
    let dir = Workdir::new("syntax");
    let (mut client, _) = Client::start(&dir, &dir.0);
    // without a window the syntax window tools say so
    let none = client.call("list_syntax_windows", json!({}));
    assert_eq!(none["isError"], true);
    assert!(
        text(&none).contains("no Statscraft window is running"),
        "{none}"
    );

    let mut desk = live::Desk::default();
    let _window = live::serve(dir.endpoint(), move |request| desk.answer(&request)).unwrap();
    let get = format!("GET FILE='{}'.", fixture("survey_basic.sav"));
    let run = client.call("run_syntax", json!({ "syntax": get }));
    assert_eq!(
        run["structuredContent"]["trail"], "syntax window Agent",
        "{run}"
    );
    assert!(text(&run).contains("syntax kept in: syntax window Agent\n"));
    let failed = client.call("run_syntax", json!({ "syntax": "FREQUENCIS age." }));
    assert_eq!(failed["structuredContent"]["ok"], false);
    let agent = client.call("read_syntax", json!({ "window": "Agent" }));
    let shown = text(&agent);
    assert!(
        shown.starts_with("Agent (id 1, 2 lines, unsaved changes)\n1  * Agent "),
        "{shown}"
    );
    assert!(shown.ends_with(&format!("\n2  {get}\n")), "{shown}");

    // the person's syntax: a window with a mistake in line 2
    let mine = "COMPUTE older = age + 10.\nFREQUENCIES olderr.\nDESCRIPTIVES age.\n";
    let new = client.call("new_syntax_window", json!({ "name": "Ages", "text": mine }));
    assert_eq!(new["structuredContent"]["window"]["id"], 2, "{new}");
    let list = client.call("list_syntax_windows", json!({}));
    let windows = &list["structuredContent"]["windows"];
    assert_eq!(windows[0]["agent"], true);
    assert_eq!(windows[1]["name"], "Ages");
    assert_eq!(windows[1]["lines"], 3);
    assert_eq!(windows[1]["unsaved"], true);
    let fixed = client.call(
        "edit_syntax",
        json!({ "window": "ages", "mode": "replace", "lines": "2-2", "text": "FREQUENCIES older." }),
    );
    assert_eq!(
        fixed["structuredContent"]["lines"],
        json!([2, 2]),
        "{fixed}"
    );
    let added = client.call(
        "edit_syntax",
        json!({ "window": "2", "mode": "append", "text": "MEANS age." }),
    );
    assert_eq!(added["structuredContent"]["lines"], json!([4, 4]));
    let bad = client.call(
        "edit_syntax",
        json!({ "window": "2", "mode": "replace", "lines": "9-9", "text": "x" }),
    );
    assert_eq!(bad["isError"], true);

    // run lines 1-2 from the window: its text is the record, so the Agent window stays as it was
    let ran = client.call("run_syntax", json!({ "window": "Ages", "lines": "1-2" }));
    let result = &ran["structuredContent"];
    assert_eq!(result["ok"], true, "{ran}");
    assert_eq!(result["trail"], "syntax window Ages");
    let names: Vec<&Value> = result["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| &c["name"])
        .collect();
    assert_eq!(names, [&json!("COMPUTE"), &json!("FREQUENCIES")]);
    let agent = client.call("read_syntax", json!({ "window": "1" }));
    assert_eq!(agent["structuredContent"]["window"]["lines"], 2);
    let both = client.call("run_syntax", json!({ "window": "Ages", "syntax": "SHOW." }));
    assert_eq!(both["isError"], true);

    // save: a new path, then never over another file unless told
    let own = dir.0.join("ages.sps");
    let other = dir.0.join("other.sps");
    std::fs::write(&other, "* not mine.\n").unwrap();
    let refused = client.call("save_syntax", json!({ "window": "Ages", "path": other }));
    assert_eq!(refused["isError"], true);
    assert!(text(&refused).contains("overwrite"), "{refused}");
    assert_eq!(std::fs::read_to_string(&other).unwrap(), "* not mine.\n");
    let saved = client.call("save_syntax", json!({ "window": "Ages", "path": own }));
    assert_eq!(
        saved["structuredContent"]["window"]["name"], "ages.sps",
        "{saved}"
    );
    assert_eq!(
        std::fs::read_to_string(&own).unwrap(),
        "COMPUTE older = age + 10.\nFREQUENCIES older.\nDESCRIPTIVES age.\nMEANS age.\n"
    );
    let relative = client.call("save_syntax", json!({ "window": "Ages", "path": "a.sps" }));
    assert_eq!(relative["isError"], true);

    // open a file: a new window, and the same window when it is open already
    let opened = client.call("open_syntax_file", json!({ "path": other }));
    assert_eq!(opened["structuredContent"]["window"]["name"], "other.sps");
    assert_eq!(opened["structuredContent"]["window"]["unsaved"], false);
    let again = client.call("open_syntax_file", json!({ "path": own }));
    assert_eq!(again["structuredContent"]["window"]["id"], 2);
    let replaced = client.call(
        "save_syntax",
        json!({ "window": "Ages", "path": other, "overwrite": true }),
    );
    assert_eq!(replaced["isError"], true, "another window holds that file");
}

/// The Claude Code plugin keeps users on its version until it changes, so it moves with every
/// release.
#[test]
fn the_plugin_version_is_the_release_version() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../plugins/statscraft/.claude-plugin/plugin.json"
    );
    let plugin: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        plugin["version"],
        env!("CARGO_PKG_VERSION"),
        "set the same version in {path}"
    );
}
