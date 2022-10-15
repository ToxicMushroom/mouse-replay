use std::collections::LinkedList;
use std::fs::File;
use std::io::{Error, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};
use evdev::{AbsInfo, AbsoluteAxisType, AttributeSet, AttributeSetRef, Device, EventType, InputEvent, InputEventKind, Key, RelativeAxisType, Synchronization, UinputAbsSetup};
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct EpicMouseEvent {
    event_kind: InputEventKind,
    value: i32,
    duration_since_start: Duration,
}

fn main() -> Result<(), Error> {
    if std::env::args().any(|arg| arg == "replay") {
        replay();
    } else if std::env::args().any(|arg| arg == "record") {
        record();
    } else if std::env::args().any(|arg| arg == "center") {
        let args: Vec<String> = std::env::args().collect();
        let x = args.last().unwrap();
        let y = args.get(args.len()-2).unwrap();
        center_cursor(x.parse().unwrap(), y.parse().unwrap());
    }

    Ok(())
}

fn center_cursor(x: i32, y: i32) {
    let mut vdev = get_virt_dev().unwrap();

    // kinda centers mouse cursor on right monitor of multimonitor setup 1080p x2
    let mut data = Vec::new();
    data.push(InputEvent::new_now(EventType::ABSOLUTE, AbsoluteAxisType::ABS_X.0, x));
    data.push(InputEvent::new_now(EventType::ABSOLUTE, AbsoluteAxisType::ABS_Y.0, y));
    data.push(InputEvent::new_now(EventType::SYNCHRONIZATION, Synchronization::SYN_REPORT.0, 0));
    vdev.emit(&*data).unwrap();
}

fn get_virt_dev() -> std::io::Result<VirtualDevice> {
    let x_axis = UinputAbsSetup::new(
        AbsoluteAxisType::ABS_X, AbsInfo::new(16456, 0, 33020, 0, 0, 200),
    );
    let y_axis = UinputAbsSetup::new(
        AbsoluteAxisType::ABS_Y, AbsInfo::new(11243, 0, 20320, 0, 0, 200),
    );

    let mut rel_axes_set = AttributeSet::new();
    rel_axes_set.insert(RelativeAxisType::REL_X);
    rel_axes_set.insert(RelativeAxisType::REL_Y);

    let mut key_set = AttributeSet::new();
    key_set.insert(Key::BTN_LEFT);
    key_set.insert(Key::BTN_RIGHT);

    let udev_builder = VirtualDeviceBuilder::new().unwrap()
        .name("Rust Virtual Mouse")
        .with_keys(&*key_set).unwrap()
        .with_absolute_axis(&x_axis).unwrap()
        .with_absolute_axis(&y_axis).unwrap()
        .with_relative_axes(&*rel_axes_set).unwrap();
    return udev_builder.build();
}

fn replay() {
    let mut udev = get_virt_dev().unwrap();
    let buf = std::fs::read("/home/merlijn/mousedump").unwrap();
    let events: Vec<EpicMouseEvent> = bincode::deserialize(&buf).unwrap();
    let mut map = events.into_iter().filter_map(|eme| {
        let (ev_type, sub_type) = match eme.event_kind {
            InputEventKind::Synchronization(sub_type) => { (EventType::SYNCHRONIZATION, sub_type.0) }
            InputEventKind::RelAxis(sub_type) => { (EventType::RELATIVE, sub_type.0) }
            InputEventKind::AbsAxis(sub_type) => { (EventType::ABSOLUTE, sub_type.0) }
            _ => { return None; }
        };
        let mut ievent = InputEvent::new_now(ev_type, sub_type, eme.value);

        Some((eme.duration_since_start, ievent))
    }).collect::<Vec<_>>();
    map.sort_by(|dur1, dur2| dur1.0.cmp(&dur2.0));

    let start_time = Instant::now();

    for (duration, ievent) in map {
        let future_point = (start_time + duration);
        let now = Instant::now();
        if let Some(t) = future_point.checked_duration_since(now) {
            sleep(t)
        }
        let events = &[ievent];
        udev.emit(events).unwrap();
        println!("emitting: {:?}", events);
    }
}

fn record() {
    let mut device = Device::open("/dev/input/event23").unwrap();
    let mut dump = File::create("/home/merlijn/mousedump").unwrap();
    let mut data = Vec::new();
    let start_time = Instant::now();
    loop {
        let current_time = Instant::now();
        let diff = current_time - start_time;
        if diff > Duration::from_secs(5) {
            break;
        }
        for event in device.fetch_events().unwrap() {
            let current_time = Instant::now();
            let eme = EpicMouseEvent {
                duration_since_start: current_time - start_time,
                value: event.value(),
                event_kind: event.kind(),
            };
            println!("receiving: {:?}", event);
            data.push(eme);
        }
    }

    let bytes = bincode::serialize(&data).unwrap();
    let _ = dump.write(&bytes).unwrap();
}
