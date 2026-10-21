//! Desktop notifications over D-Bus (org.freedesktop.Notifications), with
//! action buttons. The shell's notification server (end4-pC's quickshell,
//! mako, swaync, dunst, …) draws them.
//!
//! ```no_run
//! use material_3_ui::notify::{Notification, Notifier, Outcome};
//! # fn main() -> Result<(), String> {
//! let notifier = Notifier::connect()?;
//! let n = Notification::new("Screenshots", "Screenshot saved").action("edit", "Edit");
//! let id = notifier.send(&n)?;
//! if notifier.wait(id, std::time::Duration::from_secs(300)) == Outcome::Action("edit".into()) {
//!     // open the editor
//! }
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use zbus::blocking::{Connection, MessageIterator};
use zbus::zvariant::Value;

const DEST: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

#[derive(Clone, Debug)]
pub struct Notification {
    /// The app's name, as the server shows it.
    pub app_name: String,
    pub summary: String,
    pub body: String,
    /// An icon name ("camera-photo") or path.
    pub icon: String,
    /// A picture shown in the notification (a capture, an album cover).
    pub image: Option<PathBuf>,
    /// (key, label) buttons. The key "default" is a click on the
    /// notification itself.
    pub actions: Vec<(String, String)>,
    /// Milliseconds; -1 for the server's default, 0 to stay until closed.
    pub timeout_ms: i32,
    pub urgency: Urgency,
    /// The app's .desktop file name without the extension, so the server
    /// can group and style by app.
    pub desktop_entry: Option<String>,
    /// Replace an earlier notification instead of adding one.
    pub replaces_id: u32,
}

impl Default for Notification {
    fn default() -> Self {
        Self {
            app_name: String::new(),
            summary: String::new(),
            body: String::new(),
            icon: String::new(),
            image: None,
            actions: Vec::new(),
            timeout_ms: -1,
            urgency: Urgency::Normal,
            desktop_entry: None,
            replaces_id: 0,
        }
    }
}

impl Notification {
    pub fn new(app_name: impl Into<String>, summary: impl Into<String>) -> Self {
        Self { app_name: app_name.into(), summary: summary.into(), ..Default::default() }
    }

    /// Add a button.
    pub fn action(mut self, key: impl Into<String>, label: impl Into<String>) -> Self {
        self.actions.push((key.into(), label.into()));
        self
    }
}

/// What happened to a notification that was waited on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The key of the pressed button ("default" for a click on the body).
    Action(String),
    Closed,
    TimedOut,
}

/// A signal from the notification server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal {
    ActionInvoked { id: u32, key: String },
    /// `reason`: 1 expired, 2 dismissed, 3 closed by a call, 4 other.
    Closed { id: u32, reason: u32 },
}

impl Signal {
    pub fn id(&self) -> u32 {
        match self {
            Signal::ActionInvoked { id, .. } | Signal::Closed { id, .. } => *id,
        }
    }
}

/// Read an `ActionInvoked` or `NotificationClosed` signal.
pub fn parse_signal(msg: &zbus::Message) -> Option<Signal> {
    let header = msg.header();
    if header.interface().map(|i| i.as_str()) != Some(DEST) {
        return None;
    }
    match header.member()?.as_str() {
        "ActionInvoked" => {
            let (id, key) = msg.body().deserialize::<(u32, String)>().ok()?;
            Some(Signal::ActionInvoked { id, key })
        }
        "NotificationClosed" => {
            let (id, reason) = msg.body().deserialize::<(u32, u32)>().ok()?;
            Some(Signal::Closed { id, reason })
        }
        _ => None,
    }
}

/// Wait on `signals` for notification `id` to be acted on or closed.
fn wait_on(signals: &mpsc::Receiver<Signal>, id: u32, timeout: Duration) -> Outcome {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match signals.recv_timeout(left) {
            Ok(Signal::ActionInvoked { id: sid, key }) if sid == id => return Outcome::Action(key),
            Ok(Signal::Closed { id: sid, .. }) if sid == id => return Outcome::Closed,
            Ok(_) => continue,
            Err(_) => return Outcome::TimedOut,
        }
    }
}

/// A session-bus connection that sends notifications and hears what
/// happens to them.
pub struct Notifier {
    conn: Connection,
    signals: mpsc::Receiver<Signal>,
}

impl Notifier {
    /// Connect to the session bus.
    pub fn connect() -> Result<Self, String> {
        let conn = Connection::session().map_err(|e| format!("D-Bus: {e}"))?;
        Self::with_connection(conn)
    }

    /// Use an existing connection (a private bus in tests).
    pub fn with_connection(conn: Connection) -> Result<Self, String> {
        // Subscribe before sending, so a quick click can't be missed.
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface(DEST)
            .map_err(|e| e.to_string())?
            .path(PATH)
            .map_err(|e| e.to_string())?
            .build();
        let iter = MessageIterator::for_match_rule(rule, &conn, Some(32)).map_err(|e| format!("D-Bus: {e}"))?;
        let (tx, signals) = mpsc::channel();
        std::thread::spawn(move || {
            for msg in iter.flatten() {
                if let Some(s) = parse_signal(&msg) {
                    if tx.send(s).is_err() {
                        break;
                    }
                }
            }
        });
        Ok(Self { conn, signals })
    }

    /// Show `n`. Returns its id, for [`Notifier::wait`] and
    /// [`Notifier::close`].
    pub fn send(&self, n: &Notification) -> Result<u32, String> {
        let actions: Vec<&str> = n.actions.iter().flat_map(|(k, l)| [k.as_str(), l.as_str()]).collect();
        let mut hints: HashMap<&str, Value> = HashMap::new();
        hints.insert("urgency", Value::U8(n.urgency as u8));
        if let Some(entry) = &n.desktop_entry {
            hints.insert("desktop-entry", Value::from(entry.as_str()));
        }
        let image = n.image.as_ref().map(|p| p.to_string_lossy().into_owned());
        if let Some(image) = &image {
            hints.insert("image-path", Value::from(image.as_str()));
        }
        let body = (
            n.app_name.as_str(),
            n.replaces_id,
            n.icon.as_str(),
            n.summary.as_str(),
            n.body.as_str(),
            actions,
            hints,
            n.timeout_ms,
        );
        let reply = self
            .conn
            .call_method(Some(DEST), PATH, Some(DEST), "Notify", &body)
            .map_err(|e| format!("Notify: {e}"))?;
        reply.body().deserialize::<u32>().map_err(|e| e.to_string())
    }

    /// Wait (up to `timeout`) for a button of notification `id` to be
    /// pressed, or for it to be dismissed.
    pub fn wait(&self, id: u32, timeout: Duration) -> Outcome {
        wait_on(&self.signals, id, timeout)
    }

    /// [`Notifier::send`] then [`Notifier::wait`].
    pub fn send_and_wait(&self, n: &Notification, timeout: Duration) -> Result<Outcome, String> {
        let id = self.send(n)?;
        Ok(self.wait(id, timeout))
    }

    pub fn close(&self, id: u32) {
        let _ = self.conn.call_method(Some(DEST), PATH, Some(DEST), "CloseNotification", &(id,));
    }
}

/// Show a notification without waiting for anything.
pub fn send(n: &Notification) -> Result<u32, String> {
    Notifier::connect()?.send(n)
}

/// Show an error notification (critical, 8 seconds), ignoring failures.
pub fn error(app_name: &str, summary: &str, body: &str) {
    let n = Notification {
        body: body.into(),
        icon: "dialog-error".into(),
        timeout_ms: 8000,
        urgency: Urgency::Critical,
        ..Notification::new(app_name, summary)
    };
    if let Err(e) = send(&n) {
        eprintln!("{app_name}: couldn't notify: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};
    use std::sync::{Arc, Mutex};
    use zbus::zvariant::OwnedValue;

    fn signal(member: &str, body: &(impl serde::Serialize + zbus::zvariant::DynamicType)) -> zbus::Message {
        zbus::Message::signal(PATH, DEST, member).unwrap().build(body).unwrap()
    }

    #[test]
    fn parses_server_signals() {
        assert_eq!(
            parse_signal(&signal("ActionInvoked", &(7u32, "edit"))),
            Some(Signal::ActionInvoked { id: 7, key: "edit".into() })
        );
        assert_eq!(parse_signal(&signal("NotificationClosed", &(7u32, 2u32))), Some(Signal::Closed { id: 7, reason: 2 }));
        assert_eq!(parse_signal(&signal("ActivationToken", &(7u32, "tok"))), None);
        assert_eq!(parse_signal(&signal("ActionInvoked", &("wrong", 1u8))), None);
        let other = zbus::Message::signal(PATH, "org.example.Other", "ActionInvoked").unwrap().build(&(7u32, "x")).unwrap();
        assert_eq!(parse_signal(&other), None);
    }

    #[test]
    fn waits_for_its_own_notification() {
        let (tx, rx) = mpsc::channel();
        tx.send(Signal::ActionInvoked { id: 3, key: "other".into() }).unwrap();
        tx.send(Signal::Closed { id: 4, reason: 2 }).unwrap();
        tx.send(Signal::ActionInvoked { id: 4, key: "late".into() }).unwrap();
        tx.send(Signal::Closed { id: 5, reason: 2 }).unwrap();
        assert_eq!(wait_on(&rx, 5, Duration::from_secs(1)), Outcome::Closed);
        tx.send(Signal::ActionInvoked { id: 6, key: "default".into() }).unwrap();
        assert_eq!(wait_on(&rx, 6, Duration::from_secs(1)), Outcome::Action("default".into()));
        assert_eq!(wait_on(&rx, 6, Duration::from_millis(30)), Outcome::TimedOut);
    }

    /// A private bus for the test, killed on drop.
    struct Bus(Child, String);

    impl Bus {
        fn start() -> Option<Bus> {
            let mut child = Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .ok()?;
            let mut line = String::new();
            BufReader::new(child.stdout.take()?).read_line(&mut line).ok()?;
            Some(Bus(child, line.trim().to_string()))
        }

        fn connect(&self) -> zbus::blocking::connection::Builder<'static> {
            zbus::blocking::connection::Builder::address(self.1.as_str()).unwrap()
        }
    }

    impl Drop for Bus {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[derive(Clone, Default)]
    struct FakeServer {
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[zbus::interface(name = "org.freedesktop.Notifications")]
    impl FakeServer {
        #[allow(clippy::too_many_arguments)]
        fn notify(
            &self,
            app_name: &str,
            _replaces_id: u32,
            _icon: &str,
            summary: &str,
            _body: &str,
            actions: Vec<String>,
            hints: HashMap<String, OwnedValue>,
            timeout: i32,
        ) -> u32 {
            let mut keys: Vec<&String> = hints.keys().collect();
            keys.sort();
            self.calls
                .lock()
                .unwrap()
                .push(format!("notify {app_name}|{summary}|{}|{keys:?}|{timeout}", actions.join(",")));
            7
        }

        fn close_notification(&self, id: u32) {
            self.calls.lock().unwrap().push(format!("close {id}"));
        }
    }

    #[test]
    fn talks_to_a_notification_server() {
        let Some(bus) = Bus::start() else {
            eprintln!("dbus-daemon isn't available; skipping");
            return;
        };
        let fake = FakeServer::default();
        let server = bus.connect().name(DEST).unwrap().serve_at(PATH, fake.clone()).unwrap().build().unwrap();
        let notifier = Notifier::with_connection(bus.connect().build().unwrap()).unwrap();

        let n = Notification {
            image: Some("/tmp/shot.png".into()),
            desktop_entry: Some("material_3_screenshots".into()),
            ..Notification::new("Screenshots", "Screenshot saved").action("default", "Open").action("edit", "Edit")
        };
        let id = notifier.send(&n).unwrap();
        assert_eq!(id, 7);
        assert_eq!(
            fake.calls.lock().unwrap()[0],
            r#"notify Screenshots|Screenshot saved|default,Open,edit,Edit|["desktop-entry", "image-path", "urgency"]|-1"#
        );

        let emit = move |member: &str, body: &(u32, String)| {
            server.emit_signal(None::<&str>, PATH, DEST, member, body).unwrap();
        };
        // Someone else's click first, then ours.
        emit("ActionInvoked", &(3, "other".into()));
        emit("ActionInvoked", &(7, "edit".into()));
        assert_eq!(notifier.wait(id, Duration::from_secs(5)), Outcome::Action("edit".into()));

        let closed = notifier.conn.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            closed.emit_signal(None::<&str>, PATH, DEST, "NotificationClosed", &(8u32, 2u32)).unwrap();
        });
        assert_eq!(notifier.wait(8, Duration::from_secs(5)), Outcome::Closed);
        assert_eq!(notifier.wait(9, Duration::from_millis(50)), Outcome::TimedOut);

        notifier.close(7);
        assert_eq!(fake.calls.lock().unwrap().last().map(String::as_str), Some("close 7"));
    }
}
