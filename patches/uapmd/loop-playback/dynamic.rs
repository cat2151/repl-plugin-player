use super::{events, max_peak, ons, phrase, pump, stop, Control, Duration, Value, MAIN, SNIPPET};
use serde_json::json;

pub fn automatic(app: &mut Control, sr: f64) -> Result<(), String> {
    app.run(SNIPPET)?;
    // Expectations from the reference converter, independent of loop playback.
    app.run("for (var pair of [['c4 r4',2],[\"'c4e4g4'\",1],['c4. r8',2],['c4 r4;e2 r4',3],['{\"x\":1}c4 r4',2]]) { if (loopPhraseBeats(pair[0]) !== pair[1]) throw new Error('Phrase end: '+pair[0]); }")?;
    stop(app)?;
    let old = phrase(app, sr, MAIN)?;
    let long = phrase(app, sr, "dfacga")?;
    let short_phrase = phrase(app, sr, "err")?;
    app.loop_trace(true)?;
    app.run(SNIPPET)?;
    pump(app, Duration::from_millis(900), "automatic initial phrase")?;
    app.run(&SNIPPET.replace(MAIN, "dfacga"))?;
    let extension = pump(app, Duration::from_millis(7000), "automatic longer phrase")?;
    let extended = ons(&events(app)?);
    let transport = app.read_timeline()?["state"].clone();
    if transport["loopEndSamples"].as_u64() != Some(long.period)
        || transport["playheadSamples"]
            .as_f64()
            .is_none_or(|p| p < 0.0 || p >= long.period as f64)
    {
        return Err(format!("Extended transport disagrees: {transport}"));
    }
    let first = extended
        .iter()
        .position(|(_, n)| *n == long.pitch(0))
        .ok_or("No extension")?;
    if first == 0 || first % old.pitches.len() != 0 {
        return Err(format!("Extension interrupted old cycle: {extended:?}"));
    }
    let origin = extended[0].0;
    let switch = (first / old.pitches.len()) as u64 * old.period;
    for (i, &(frame, note)) in extended.iter().enumerate() {
        let (pitch, at) = if i < first {
            (old.pitch(i), old.at(i, old.period))
        } else {
            (
                long.pitch(i - first),
                switch + long.at(i - first, long.period),
            )
        };
        if note != pitch || (frame - origin).abs_diff(at) > 1 {
            return Err(format!(
                "Extended tail, timing or origin mismatch: {extended:?}"
            ));
        }
    }
    if extended.len() < first + 2 * long.pitches.len() || max_peak(&extension)? <= 0.001 {
        return Err("Insufficient longer-phrase delivery or master signal".into());
    }
    app.run(&SNIPPET.replace(MAIN, "err"))?;
    let contraction = pump(
        app,
        Duration::from_millis(5500),
        "automatic shorter phrase with rests",
    )?;
    let contracted = ons(&events(app)?);
    if app.read_timeline()?["state"]["loopEndSamples"].as_u64() != Some(short_phrase.period) {
        return Err("Shortened transport disagrees".into());
    }
    let short = contracted
        .iter()
        .enumerate()
        .skip(first)
        .find(|(_, (_, n))| *n == short_phrase.pitch(0))
        .map(|(i, _)| i)
        .ok_or("No contraction")?;
    let boundary = contracted[short].0;
    if (short - first) % long.pitches.len() != 0
        || !(boundary - extended[first].0).is_multiple_of(long.period)
    {
        return Err(format!(
            "Shortening broke the longer-phrase boundary: {contracted:?}"
        ));
    }
    let tail = &contracted[short..];
    let period = short_phrase.period;
    if tail.len() < 2
        || tail.iter().any(|(_, n)| *n != short_phrase.pitch(0))
        || tail.windows(2).any(|p| p[1].0 - p[0].0 != period)
        || max_peak(&contraction)? <= 0.001
    {
        return Err(format!("Trailing rests lost: {tail:?}"));
    }
    let saved = app.eval("JSON.stringify(loopParts)", "map before invalid update")?;
    for invalid in [
        SNIPPET.replace(MAIN, ""),
        SNIPPET.replace(MAIN, &format!("t90 {MAIN}")),
    ] {
        if app.run(&invalid).is_ok() {
            return Err("Invalid automatic input accepted".into());
        }
    }
    if saved != app.eval("JSON.stringify(loopParts)", "map after invalid update")? {
        return Err("Invalid input replaced script map".into());
    }
    pump(
        app,
        Duration::from_millis(2000),
        "old notes and period after invalid update",
    )?;
    let delivered = events(app)?;
    let after_failure = ons(&delivered);
    if after_failure[short..]
        .iter()
        .any(|(_, n)| *n != short_phrase.pitch(0))
        || after_failure[short..]
            .windows(2)
            .any(|p| p[1].0 - p[0].0 != period)
    {
        return Err("Invalid input damaged active plan".into());
    }
    let mut held = std::collections::BTreeSet::new();
    for event in &delivered {
        let note = event["note"].as_u64().ok_or("Missing trace note")?;
        if event["on"] == true {
            if !held.insert(note) {
                return Err(format!("Double note-on: {event}"));
            }
        } else {
            held.remove(&note);
        }
    }
    app.run(&SNIPPET.replace(MAIN, "aaaaaa"))?;
    stop(app)?;
    app.log.event(
        "loop_automatic_delivery",
        json!({"sampleRate":sr,"events":delivered,
        "extensionIndex":first,"shortIndex":short,"shortPeriod":period,"mapPreserved":true,
        "extensionPeak":max_peak(&extension)?,"shortPeak":max_peak(&contraction)?}),
    )?;
    Ok(())
}

fn part_notes(delivered: &[Value], instance: i64) -> Vec<(u64, u64)> {
    ons(&delivered
        .iter()
        .filter(|e| e["instance"] == instance)
        .cloned()
        .collect::<Vec<_>>())
}

pub fn multiple(
    app: &mut Control,
    sr: f64,
    main: i64,
    second: i64,
    a_lead: u64,
    b_lead: u64,
) -> Result<(), String> {
    // Short part plays once then rests. The longest part controls the shared period.
    let (pa, pb, pc) = (
        phrase(app, sr, "f+f+rrrr")?,
        phrase(app, sr, "brr")?,
        phrase(app, sr, "c")?,
    );
    let cycle = pa.period.max(pb.period);
    app.run("var next = Object.assign({},loopParts); next.main=prepareLoopPart(loopTrack.trackIndex,'f+f+rrrr'); next.second=prepareLoopPart(secondTrack.trackIndex,'brr'); publishLoopParts(next, 120);")?;
    let extension = pump(
        app,
        Duration::from_millis(7200),
        "longest part extends both tracks",
    )?;
    let delivered = events(app)?;
    let a = part_notes(&delivered, main);
    let b = part_notes(&delivered, second);
    let ia = a
        .iter()
        .position(|(_, n)| *n == pa.pitch(0))
        .ok_or("Main extension missing")?;
    let ib = b
        .iter()
        .position(|(_, n)| *n == pb.pitch(0))
        .ok_or("Second extension missing")?;
    let common = a[ia].0 + a_lead;
    if a_lead == b_lead || common != b[ib].0 + b_lead || b.len() < ib + 2 || a.len() < ia + 4 {
        return Err(format!("Dynamic lead alignment missing: {a:?} {b:?}"));
    }
    for (i, &(frame, note)) in a[ia..].iter().enumerate() {
        if note != pa.pitch(i) || frame + a_lead != common + pa.at(i, cycle) {
            return Err(format!("Main long period mismatch: {a:?}"));
        }
    }
    for (i, &(frame, note)) in b[ib..].iter().enumerate() {
        if note != pb.pitch(i) || frame + b_lead != common + pb.at(i, cycle) {
            return Err(format!("Short part repeated independently: {b:?}"));
        }
    }
    // Shortening the previously longest part leaves the second part longest.
    app.run("var next=Object.assign({},loopParts); next.main=prepareLoopPart(loopTrack.trackIndex,'c'); publishLoopParts(next, 120);")?;
    let contraction = pump(
        app,
        Duration::from_millis(6000),
        "longest part shortens common period",
    )?;
    let delivered = events(app)?;
    let a = part_notes(&delivered, main);
    let b = part_notes(&delivered, second);
    let short = a
        .iter()
        .enumerate()
        .skip(ia)
        .find(|(_, (_, n))| *n == pc.pitch(0))
        .map(|(i, _)| i)
        .ok_or("Main shortening missing")?;
    let boundary = a[short].0 + a_lead;
    let b_short = b
        .iter()
        .position(|(f, _)| *f + b_lead == boundary)
        .ok_or("Second phase not shared on shortening")?;
    let period = pc.period.max(pb.period);
    if !(boundary - common).is_multiple_of(cycle)
        || a.len() < short + 2
        || b.len() < b_short + 2
        || a[short..].iter().any(|(_, n)| *n != pc.pitch(0))
        || a[short..].windows(2).any(|p| p[1].0 - p[0].0 != period)
        || b[b_short..].windows(2).any(|p| p[1].0 - p[0].0 != period)
        || max_peak(&extension)? <= 0.001
        || max_peak(&contraction)? <= 0.001
    {
        return Err(format!("Common period shortening mismatch: {a:?} {b:?}"));
    }
    app.log.event(
        "loop_dynamic_parts",
        json!({"events":delivered,"mainLead":a_lead,"secondLead":b_lead,
        "extensionOrigin":common,"shortOrigin":boundary,"shortPeriod":period}),
    )?;
    Ok(())
}
