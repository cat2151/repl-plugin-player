use super::*;
use serde_json::json;

fn edge(source: &str, target: &str) -> Value {
    json!({"busType":"audio", "source":{"type":"plugin","nodeId":source},
        "target":{"type":"plugin","nodeId":target}})
}

fn fixture() -> Value {
    json!({"nodes":[
        {"nodeId":"effect:0","isPlugin":true,"displayName":"Effect","instanceId":2},
        {"nodeId":"gain","isPlugin":false},
        {"nodeId":"plugin:0","isPlugin":true,"displayName":"Instrument","instanceId":1}],
        "graph":{"success":true,"connections":[
            edge("plugin:0", "gain"), edge("gain", "effect:0"),
            {"busType":"audio","source":{"type":"plugin","nodeId":"effect:0"},
                "target":{"type":"graphOutput"}},
            {"busType":"event","source":{"type":"graphInput"},
                "target":{"type":"plugin","nodeId":"plugin:0"}}]}})
}

#[test]
fn serial_audio_path_crosses_builtins_and_ignores_enumeration_and_event_order() {
    assert_eq!(
        project(&fixture()),
        (vec!["Instrument".into(), "Effect".into()], true)
    );
    let mut data = fixture();
    data["graph"]["connections"][0]["source"] = json!({"type":"plugin","instanceId":1});
    assert_eq!(project(&data).0, ["Instrument", "Effect"]);
}

#[test]
fn branches_merges_cycles_and_disconnected_plugins_keep_occurrence_order_without_arrows() {
    for extra in [
        edge("plugin:0", "effect:0"),
        edge("effect:0", "plugin:0"),
        edge("unconnected-a", "unconnected-b"),
        edge("cycle-a", "cycle-b"),
    ] {
        let mut data = fixture();
        data["graph"]["connections"]
            .as_array_mut()
            .unwrap()
            .push(extra);
        assert_eq!(
            project(&data),
            (vec!["Effect".into(), "Instrument".into()], false)
        );
    }
    let mut data = fixture();
    data["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"nodeId":"disconnected","isPlugin":true,"displayName":"Unused"}));
    assert_eq!(
        project(&data),
        (
            vec!["Effect".into(), "Instrument".into(), "Unused".into()],
            false
        )
    );
}

#[test]
fn missing_or_failed_graph_does_not_claim_a_signal_order() {
    for graph in [
        Value::Null,
        json!({"success":false}),
        json!({"success":true,"connections":[]}),
    ] {
        let mut data = fixture();
        data["graph"] = graph;
        assert_eq!(
            project(&data),
            (vec!["Effect".into(), "Instrument".into()], false)
        );
    }
}
