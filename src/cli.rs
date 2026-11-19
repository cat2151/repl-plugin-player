use crate::{
    control::{Control, INIT},
    execution,
    log::Log,
    snippets::Library,
};
use serde_json::{json, Value};
use std::{
    fs::File,
    io::BufRead,
    sync::mpsc,
    time::{Duration, Instant},
};

#[path = "cli_output.rs"]
mod output;

pub fn run(args: &[String]) -> Result<(), String> {
    let mut output = output::stdout()?;
    if args == ["--help"] || args == ["-h"] {
        use std::io::Write;
        return output
            .write_all(crate::i18n::t("cli_help").as_bytes())
            .map_err(|e| e.to_string());
    }
    let options = match parse(args) {
        Ok(options) => options,
        Err(error) => {
            output::emit(
                &mut output,
                json!({"event":"error", "ok":false, "error":error}),
            )?;
            return Err(error);
        }
    };
    let deadline = Instant::now() + Duration::from_secs(options.seconds);
    let result = session(&options, deadline, &mut output);
    if let Err(error) = &result {
        output::emit(
            &mut output,
            json!({"event":"error", "ok":false, "error":error}),
        )?;
    }
    result
}

struct Options {
    script: Option<String>,
    seconds: u64,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let (script, mut i) = match args.first().map(String::as_str) {
        Some("--automation") => (None, 1),
        Some("--script") => (
            Some(
                args.get(1)
                    .filter(|s| !s.starts_with("--"))
                    .ok_or(crate::i18n::t("m075"))?
                    .clone(),
            ),
            2,
        ),
        _ => return Err(crate::i18n::t("m076").into()),
    };
    let mut seconds = 180;
    if args.get(i).map(String::as_str) == Some("--timeout") {
        seconds = args
            .get(i + 1)
            .ok_or(crate::i18n::t("m077"))?
            .parse()
            .map_err(|_| crate::i18n::t("m078"))?;
        i += 2;
    }
    if i != args.len() || !(1..=3600).contains(&seconds) {
        return Err(crate::i18n::t("m079").into());
    }
    Ok(Options { script, seconds })
}

fn session(options: &Options, deadline: Instant, output: &mut File) -> Result<(), String> {
    // Read input on another thread; the Control and all plugin calls stay here.
    let input = if options.script.is_none() {
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            for line in std::io::stdin().lock().lines() {
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        Some(receive)
    } else {
        None
    };
    let log = Log::new()?;
    let log_path = log.path.clone();
    let _frames = crate::frame_monitor::start(&log_path);
    let mut app = Control::open(log, INIT)?;
    let execution = (|| {
        while !app.ready {
            pump(&mut app, deadline)?;
        }
        output::emit(output, json!({"event":"ready", "ok":true, "log":log_path}))?;
        if let Some(path) = &options.script {
            let result = command(
                &mut app,
                &json!({"command":"script", "path":path}),
                deadline,
            )?;
            output::emit(
                output,
                json!({"event":"result", "ok":true, "result":result}),
            )?;
            return Ok(());
        }
        let mut failed = false;
        loop {
            pump(&mut app, deadline)?;
            match input
                .as_ref()
                .unwrap()
                .recv_timeout(Duration::from_millis(10))
            {
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Ok(line) => {
                    let request = line.map_err(|error| error.to_string()).and_then(|line| {
                        serde_json::from_str::<Value>(&line).map_err(|error| error.to_string())
                    });
                    let id = request
                        .as_ref()
                        .ok()
                        .and_then(|v| v.get("id"))
                        .cloned()
                        .unwrap_or(Value::Null);
                    let closing = request.as_ref().is_ok_and(|v| v["command"] == "close");
                    let result = request.and_then(|request| {
                        if closing {
                            Ok(Value::Null)
                        } else {
                            command(&mut app, &request, deadline)
                        }
                    });
                    let reply = match result {
                        Ok(result) => json!({"id":id,"ok":true,"result":result}),
                        Err(error) => {
                            failed = true;
                            json!({"id":id,"ok":false,"error":error})
                        }
                    };
                    output::emit(output, reply)?;
                    if closing {
                        break;
                    }
                }
            }
        }
        if failed {
            Err(crate::i18n::t("m080").into())
        } else {
            Ok(())
        }
    })();
    let cleanup = app.close();
    output::emit(
        output,
        json!({"event":"closed", "ok":cleanup.is_ok(), "error":cleanup.as_ref().err()}),
    )?;
    execution.and(cleanup)
}

fn text<'a>(request: &'a Value, name: &str) -> Result<&'a str, String> {
    request[name]
        .as_str()
        .ok_or_else(|| crate::i18n::render("f020", &[("name", name.to_string())]))
}

fn command(app: &mut Control, request: &Value, deadline: Instant) -> Result<Value, String> {
    match text(request, "command")? {
        "wait" => {
            let code = text(request, "code")?;
            loop {
                pump(app, deadline)?;
                if app.eval(code, "CLI wait")? == "true" {
                    return Ok(json!(true));
                }
            }
        }
        "state" => {
            let instance = request["instanceId"]
                .as_i64()
                .filter(|id| (0..=i64::from(i32::MAX)).contains(id))
                .ok_or_else(|| crate::i18n::t("m081").to_owned())?;
            Ok(json!(app.read_plugin_state(instance, deadline)?))
        }
        "eval" | "script" | "snippet" => {
            let request = match request["command"].as_str().unwrap() {
                "snippet" => {
                    let group = text(request, "group")?;
                    let name = text(request, "name")?;
                    let entries = Library::default().load()?;
                    let snippet = entries
                        .iter()
                        .find(|s| s.group == group && s.name == name)
                        .ok_or_else(|| {
                            crate::i18n::render(
                                "f021",
                                &[("group", group.to_string()), ("name", name.to_string())],
                            )
                        })?;
                    crate::snippet_insert::insert(&mut String::new(), 0, snippet)
                }
                kind => {
                    let code = if kind == "script" {
                        let path = text(request, "path")?;
                        std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?
                    } else {
                        text(request, "code")?.to_owned()
                    };
                    execution::request(
                        code.trim_start_matches('\u{feff}'),
                        0,
                        0,
                        execution::Scope::All,
                    )
                }
            };
            let value = app.run_request_result(&request)?;
            Ok(
                json!({"undefined":value == "undefined", "value":if value == "undefined" { Value::Null }
                else { serde_json::from_str(&value).map_err(|e| crate::i18n::render("f022", &[("e", format!("{}", e))]))? }}),
            )
        }
        other => Err(crate::i18n::render("f023", &[("other", other.to_string())])),
    }
}

fn pump(app: &mut Control, deadline: Instant) -> Result<(), String> {
    if Instant::now() >= deadline {
        return Err(crate::i18n::t("m082").into());
    }
    crate::frame_monitor::beat();
    app.pump_cli_messages()?;
    app.poll()?;
    std::thread::sleep(Duration::from_millis(10));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_ambiguous_or_unbounded_invocations() {
        for args in [
            vec!["--script"],
            vec!["--automation", "--timeout", "0"],
            vec!["--automation", "--timeout", "3601"],
            vec!["--automation", "extra"],
        ] {
            assert!(parse(&args.into_iter().map(String::from).collect::<Vec<_>>()).is_err());
        }
        let options = parse(&["--script".into(), "X:/checks/日本語.js".into()]).unwrap();
        assert_eq!(options.seconds, 180);
        assert_eq!(options.script.as_deref(), Some("X:/checks/日本語.js"));
    }
}
