use super::*;

fn screen(number: u32, bounds: [i32; 4]) -> Screen {
    Screen {
        number,
        bounds,
        work: bounds,
    }
}

fn whole(monitor: Monitor) -> MonitorPlacement {
    MonitorPlacement {
        monitor,
        left_percent: 0.0,
        top_percent: 0.0,
        width_percent: 100.0,
        height_percent: 100.0,
    }
}

/// Enumerated out of left-to-right order, DISPLAY numbers out of order too.
fn three() -> Vec<Screen> {
    vec![
        screen(3, [3000, 0, 1920, 1080]),
        screen(1, [0, 0, 1920, 1080]),
        screen(2, [1920, -836, 1080, 1920]),
    ]
}

#[test]
fn positions_follow_the_monitors_centers() {
    let screens = three();
    for (position, number) in [
        (Position::Left, 1),
        (Position::Center, 2),
        (Position::Right, 3),
    ] {
        assert_eq!(
            whole(Monitor::Position(position)).resolve(&screens),
            whole(Monitor::Number(number)).resolve(&screens)
        );
    }
    assert_eq!(
        whole(Monitor::Position(Position::Right)).resolve(&screens),
        Ok([3000, 0, 1920, 1080])
    );
}

#[test]
fn percentages_are_of_the_work_area() {
    let mut screens = three();
    screens[0].work = [3000, 0, 1920, 1040];
    let mut half = whole(Monitor::Position(Position::Right));
    half.left_percent = 50.0;
    half.width_percent = 50.0;
    half.height_percent = 50.0;
    assert_eq!(half.resolve(&screens), Ok([3960, 0, 960, 520]));
}

#[test]
fn ambiguous_or_missing_monitors_are_refused() {
    let mut screens = vec![screen(7, [-1920, -100, 1920, 1080])];
    for position in [Position::Left, Position::Center, Position::Right] {
        assert_eq!(
            whole(Monitor::Position(position)).resolve(&screens),
            Ok(screens[0].work)
        );
    }
    screens.push(screen(2, [0, 0, 1920, 1080]));
    assert!(whole(Monitor::Position(Position::Center))
        .resolve(&screens)
        .is_err());
    screens[1].bounds = [-1920, 980, 1920, 1080];
    assert!(whole(Monitor::Position(Position::Left))
        .resolve(&screens)
        .is_err());
    assert!(whole(Monitor::Number(2)).resolve(&screens).is_ok());
    assert!(whole(Monitor::Number(1)).resolve(&screens).is_err());
    assert!(whole(Monitor::Position(Position::Left))
        .resolve(&[])
        .is_err());
}

#[test]
fn the_table_reads_back_as_written() {
    let text = "monitor = \"right\"\nleft_percent = 0\ntop_percent = 12.5\nwidth_percent = 100\nheight_percent = 87.5\n";
    let placement = MonitorPlacement::read(&toml::from_str(text).unwrap()).unwrap();
    assert_eq!(placement.top_percent, 12.5);
    let written = placement.write();
    assert_eq!(written, format!("[window.monitor_placement]\n{text}"));
    for valid in ["1", "\"left\"", "\"center\""] {
        let text = text.replace("\"right\"", valid);
        assert!(
            MonitorPlacement::read(&toml::from_str(&text).unwrap()).is_ok(),
            "{text}"
        );
    }
    for invalid in [
        text.replace("\"right\"", "0"),
        text.replace("\"right\"", "\"primary\""),
        text.replace("width_percent = 100", "width_percent = 101"),
        text.replace("left_percent = 0", "left_percent = 1"),
        text.replace("width_percent = 100", "width_percent = 0"),
        text.replace("height_percent = 87.5", "height_percent = -1"),
        text.replace("height_percent = 87.5\n", ""),
        format!("{text}x = 1\n"),
    ] {
        assert!(
            MonitorPlacement::read(&toml::from_str(&invalid).unwrap()).is_err(),
            "{invalid}"
        );
    }
}
