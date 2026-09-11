use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
};
use zbus::{
    Connection,
    message::{Flags, Header},
    zvariant::OwnedObjectPath,
};

pub const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const UNIT_PATH: &str = "/org/freedesktop/systemd1/unit/example_2eservice";

pub struct TestBus {
    child: Child,
    pub address: String,
}

impl TestBus {
    pub fn new() -> Self {
        let mut child = Command::new("dbus-daemon")
            .args([
                "--config-file=tests/session-bus.conf",
                "--nofork",
                "--print-address=1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("Install dbus-daemon to run isolated D-Bus integration tests");
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        assert!(
            address.starts_with("unix:"),
            "Test bus did not provide an address"
        );
        Self {
            child,
            address: address.trim().into(),
        }
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for TestBus {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug, Clone)]
pub struct Call {
    pub method: String,
    pub args: Vec<String>,
    pub interactive: bool,
}

pub struct FakeState {
    pub description: String,
    pub active: String,
    pub file_state: String,
    pub result: String,
    pub reload_fails: bool,
    pub list_hangs: bool,
    pub fragment: String,
    pub drop_ins: Vec<String>,
    pub calls: Vec<Call>,
}

impl Default for FakeState {
    fn default() -> Self {
        Self {
            description: "Test service".into(),
            active: "inactive".into(),
            file_state: "disabled".into(),
            result: "done".into(),
            reload_fails: false,
            list_hangs: false,
            fragment: String::new(),
            drop_ins: vec![],
            calls: vec![],
        }
    }
}

type Shared = Arc<Mutex<FakeState>>;
type ListedUnit = (
    String,
    String,
    String,
    String,
    String,
    String,
    OwnedObjectPath,
    u32,
    String,
    OwnedObjectPath,
);
type FileChange = (String, String, String);

pub async fn serve(address: &str, state: Shared) -> Connection {
    zbus::connection::Builder::address(address)
        .unwrap()
        .name("org.freedesktop.systemd1")
        .unwrap()
        .serve_at(MANAGER_PATH, FakeManager(state.clone()))
        .unwrap()
        .serve_at(UNIT_PATH, FakeUnit(state))
        .unwrap()
        .build()
        .await
        .unwrap()
}

fn path(value: &str) -> OwnedObjectPath {
    OwnedObjectPath::try_from(value).unwrap()
}

struct FakeManager(Shared);

impl FakeManager {
    fn record(&self, method: &str, args: Vec<String>, header: &Header<'_>) {
        self.0.lock().unwrap().calls.push(Call {
            method: method.into(),
            args,
            interactive: header
                .primary()
                .flags()
                .contains(Flags::AllowInteractiveAuth),
        });
    }

    async fn runtime_action(
        &self,
        method: &str,
        name: &str,
        mode: &str,
        header: Header<'_>,
        connection: &Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        self.record(method, vec![name.into(), mode.into()], &header);
        let result = self.0.lock().unwrap().result.clone();
        if result == "deny" {
            return Err(zbus::fdo::Error::AccessDenied(
                "Authorization denied".into(),
            ));
        }
        let job = path("/org/freedesktop/systemd1/job/42");
        if result == "hang" {
            return Ok(job);
        }
        if result == "done" {
            self.0.lock().unwrap().active = if method == "stop" {
                "inactive"
            } else {
                "active"
            }
            .into();
        }
        // Both signals are sent before the method returns its job path.
        connection
            .emit_signal(
                None::<&str>,
                MANAGER_PATH,
                "org.freedesktop.systemd1.Manager",
                "JobRemoved",
                &(
                    41u32,
                    path("/org/freedesktop/systemd1/job/41"),
                    "unrelated.service",
                    "failed",
                ),
            )
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        connection
            .emit_signal(
                None::<&str>,
                MANAGER_PATH,
                "org.freedesktop.systemd1.Manager",
                "JobRemoved",
                &(42u32, job.clone(), name, result),
            )
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(job)
    }
}

#[zbus::interface(name = "org.freedesktop.systemd1.Manager")]
impl FakeManager {
    async fn list_units(&self) -> Vec<ListedUnit> {
        let hangs = self.0.lock().unwrap().list_hangs;
        if hangs {
            std::future::pending::<()>().await;
        }
        let state = self.0.lock().unwrap();
        vec![(
            "example.service".into(),
            state.description.clone(),
            "loaded".into(),
            state.active.clone(),
            "dead".into(),
            "".into(),
            path(UNIT_PATH),
            0,
            "".into(),
            path("/"),
        )]
    }
    fn get_unit(&self, name: &str) -> zbus::fdo::Result<OwnedObjectPath> {
        if name == "example.service" {
            Ok(path(UNIT_PATH))
        } else {
            Err(zbus::fdo::Error::UnknownObject(name.into()))
        }
    }
    fn subscribe(&self) {}
    async fn start_unit(
        &self,
        name: &str,
        mode: &str,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        self.runtime_action("start", name, mode, header, connection)
            .await
    }
    async fn stop_unit(
        &self,
        name: &str,
        mode: &str,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        self.runtime_action("stop", name, mode, header, connection)
            .await
    }
    async fn restart_unit(
        &self,
        name: &str,
        mode: &str,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        self.runtime_action("restart", name, mode, header, connection)
            .await
    }
    fn enable_unit_files(
        &self,
        names: Vec<String>,
        runtime: bool,
        force: bool,
        #[zbus(header)] header: Header<'_>,
    ) -> (bool, Vec<FileChange>) {
        self.record(
            "enable",
            [names, vec![runtime.to_string(), force.to_string()]].concat(),
            &header,
        );
        self.0.lock().unwrap().file_state = "enabled".into();
        (true, vec![])
    }
    fn disable_unit_files(
        &self,
        names: Vec<String>,
        runtime: bool,
        #[zbus(header)] header: Header<'_>,
    ) -> Vec<FileChange> {
        self.record(
            "disable",
            [names, vec![runtime.to_string()]].concat(),
            &header,
        );
        self.0.lock().unwrap().file_state = "disabled".into();
        vec![]
    }
    fn reload(&self, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<()> {
        self.record("reload", vec![], &header);
        if self.0.lock().unwrap().reload_fails {
            Err(zbus::fdo::Error::AccessDenied("Reload denied".into()))
        } else {
            Ok(())
        }
    }
}

struct FakeUnit(Shared);

#[zbus::interface(name = "org.freedesktop.systemd1.Unit")]
impl FakeUnit {
    #[zbus(property)]
    fn description(&self) -> String {
        self.0.lock().unwrap().description.clone()
    }
    #[zbus(property)]
    fn load_state(&self) -> &str {
        "loaded"
    }
    #[zbus(property)]
    fn active_state(&self) -> String {
        self.0.lock().unwrap().active.clone()
    }
    #[zbus(property)]
    fn sub_state(&self) -> &str {
        "dead"
    }
    #[zbus(property)]
    fn can_start(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_stop(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn refuse_manual_start(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn refuse_manual_stop(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn job(&self) -> (u32, OwnedObjectPath) {
        (0, path("/"))
    }
    #[zbus(property)]
    fn unit_file_state(&self) -> String {
        self.0.lock().unwrap().file_state.clone()
    }
    #[zbus(property)]
    fn fragment_path(&self) -> String {
        self.0.lock().unwrap().fragment.clone()
    }
    #[zbus(property)]
    fn drop_in_paths(&self) -> Vec<String> {
        self.0.lock().unwrap().drop_ins.clone()
    }
}
