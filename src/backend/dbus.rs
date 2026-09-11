use super::{BackendError, BackendEvent, Result, SystemdBackend};
use crate::model::{Scope, UnitAction, UnitDetails, UnitFileContent, UnitId, UnitSummary};
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use std::{path::PathBuf, time::Duration};
use tokio::sync::Mutex;
use zbus::{Connection, MatchRule, MessageStream, message::Type, zvariant::OwnedObjectPath};

const SERVICE: &str = "org.freedesktop.systemd1";
const PATH: &str = "/org/freedesktop/systemd1";
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

#[zbus::proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1",
    gen_blocking = false
)]
trait Manager {
    fn list_units(&self) -> zbus::Result<Vec<ListedUnit>>;
    fn get_unit(&self, name: &str) -> zbus::Result<OwnedObjectPath>;
    fn subscribe(&self) -> zbus::Result<()>;
    #[zbus(allow_interactive_auth)]
    fn start_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;
    #[zbus(allow_interactive_auth)]
    fn stop_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;
    #[zbus(allow_interactive_auth)]
    fn restart_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;
    #[zbus(allow_interactive_auth)]
    fn enable_unit_files(
        &self,
        names: &[&str],
        runtime: bool,
        force: bool,
    ) -> zbus::Result<(bool, Vec<FileChange>)>;
    #[zbus(allow_interactive_auth)]
    fn disable_unit_files(&self, names: &[&str], runtime: bool) -> zbus::Result<Vec<FileChange>>;
    #[zbus(allow_interactive_auth)]
    fn reload(&self) -> zbus::Result<()>;
    #[zbus(signal)]
    fn job_removed(
        &self,
        id: u32,
        job: OwnedObjectPath,
        unit: String,
        result: String,
    ) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.systemd1.Unit",
    default_service = "org.freedesktop.systemd1",
    gen_blocking = false
)]
trait Unit {
    #[zbus(property(emits_changed_signal = "false"))]
    fn description(&self) -> zbus::Result<String>;
    #[zbus(property(emits_changed_signal = "false"))]
    fn load_state(&self) -> zbus::Result<String>;
    #[zbus(property(emits_changed_signal = "false"))]
    fn active_state(&self) -> zbus::Result<String>;
    #[zbus(property(emits_changed_signal = "false"))]
    fn sub_state(&self) -> zbus::Result<String>;
    #[zbus(property(emits_changed_signal = "false"))]
    fn can_start(&self) -> zbus::Result<bool>;
    #[zbus(property(emits_changed_signal = "false"))]
    fn can_stop(&self) -> zbus::Result<bool>;
    #[zbus(property(emits_changed_signal = "false"))]
    fn refuse_manual_start(&self) -> zbus::Result<bool>;
    #[zbus(property(emits_changed_signal = "false"))]
    fn refuse_manual_stop(&self) -> zbus::Result<bool>;
    #[zbus(property(emits_changed_signal = "false"))]
    fn job(&self) -> zbus::Result<(u32, OwnedObjectPath)>;
    #[zbus(property(emits_changed_signal = "false"))]
    fn unit_file_state(&self) -> zbus::Result<String>;
    #[zbus(property(emits_changed_signal = "false"))]
    fn fragment_path(&self) -> zbus::Result<String>;
    #[zbus(property(emits_changed_signal = "false"))]
    fn drop_in_paths(&self) -> zbus::Result<Vec<String>>;
}

#[derive(Debug, Clone, Copy)]
pub struct Timeouts {
    pub read: Duration,
    pub operation: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            read: Duration::from_secs(30),
            operation: Duration::from_secs(120),
        }
    }
}

enum Endpoints {
    Host,
    Explicit { system: String, user: String },
}

/// Connections are scope-specific. Explicit addresses are deliberately mandatory
/// in tests, so a missing test service cannot fall back to the host managers.
pub struct DbusBackend {
    endpoints: Endpoints,
    connections: [Mutex<Option<Connection>>; 2],
    timeouts: Timeouts,
}

impl Default for DbusBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl DbusBackend {
    pub fn new() -> Self {
        Self {
            endpoints: Endpoints::Host,
            connections: std::array::from_fn(|_| Mutex::new(None)),
            timeouts: Timeouts::default(),
        }
    }

    pub fn for_addresses(system: String, user: String, timeouts: Timeouts) -> Self {
        Self {
            endpoints: Endpoints::Explicit { system, user },
            connections: std::array::from_fn(|_| Mutex::new(None)),
            timeouts,
        }
    }

    async fn connect(&self, scope: Scope) -> Result<Connection> {
        let builder = match &self.endpoints {
            Endpoints::Host => match scope {
                Scope::System => zbus::connection::Builder::system()?,
                Scope::User => zbus::connection::Builder::session()?,
            },
            Endpoints::Explicit { system, user } => {
                zbus::connection::Builder::address(match scope {
                    Scope::System => system.as_str(),
                    Scope::User => user.as_str(),
                })?
            }
        };
        Ok(builder
            .method_timeout(self.timeouts.operation)
            .build()
            .await?)
    }

    async fn connection(&self, scope: Scope) -> Result<Connection> {
        // A slow system connection must not block connecting to the user manager.
        let mut slot = self.connection_slot(scope).lock().await;
        if let Some(connection) = slot.as_ref() {
            return Ok(connection.clone());
        }
        let connection = self.connect(scope).await?;
        ManagerProxy::new(&connection).await?.subscribe().await?;
        *slot = Some(connection.clone());
        Ok(connection)
    }

    fn connection_slot(&self, scope: Scope) -> &Mutex<Option<Connection>> {
        &self.connections[match scope {
            Scope::System => 0,
            Scope::User => 1,
        }]
    }

    async fn read<T>(&self, future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
        tokio::time::timeout(self.timeouts.read, future)
            .await
            .map_err(|_| BackendError::ReadTimeout)?
    }

    async fn unit_proxy<'a>(
        &self,
        connection: &'a Connection,
        id: &UnitId,
    ) -> Result<UnitProxy<'a>> {
        let path = ManagerProxy::new(connection)
            .await?
            .get_unit(&id.name)
            .await?;
        Ok(UnitProxy::builder(connection).path(path)?.build().await?)
    }

    async fn read_details(&self, id: UnitId) -> Result<UnitDetails> {
        let connection = self.connection(id.scope).await?;
        let unit = self.unit_proxy(&connection, &id).await?;
        let (
            description,
            load,
            active,
            sub,
            can_start,
            can_stop,
            refuse_start,
            refuse_stop,
            job,
            file_state,
        ) = tokio::try_join!(
            unit.description(),
            unit.load_state(),
            unit.active_state(),
            unit.sub_state(),
            unit.can_start(),
            unit.can_stop(),
            unit.refuse_manual_start(),
            unit.refuse_manual_stop(),
            unit.job(),
            unit.unit_file_state(),
        )?;
        Ok(UnitDetails {
            unit: UnitSummary {
                id,
                description,
                load,
                active,
                sub,
            },
            can_start,
            can_stop,
            refuse_start,
            refuse_stop,
            job_pending: job.0 != 0,
            file_state,
        })
    }

    async fn run_action(&self, id: UnitId, action: UnitAction) -> Result<()> {
        // Revalidate immediately before dispatch; the UI's snapshot can be stale.
        let details = self.read(self.read_details(id.clone())).await?;
        if !details.allows(action) {
            return Err(BackendError::Unavailable(format!(
                "{action} is no longer available for {}",
                id.name
            )));
        }
        let connection = self.connection(id.scope).await?;
        let manager = ManagerProxy::new(&connection).await?;
        if matches!(action, UnitAction::Enable | UnitAction::Disable) {
            if action == UnitAction::Enable {
                manager
                    .enable_unit_files(&[&id.name], false, false)
                    .await
                    .map_err(write_error)?;
            } else {
                manager
                    .disable_unit_files(&[&id.name], details.file_state == "enabled-runtime")
                    .await
                    .map_err(write_error)?;
            }
            // Reload failure must not hide the fact that the files already changed.
            tokio::time::timeout(self.timeouts.read, manager.reload())
                .await
                .map_err(|_| BackendError::PartialSuccess("reload timed out".into()))?
                .map_err(|e| BackendError::PartialSuccess(e.to_string()))?;
            return Ok(());
        }

        tokio::time::timeout(self.timeouts.operation, self.run_job(&manager, &id, action))
            .await
            .map_err(|_| BackendError::OutcomeUnknown("the operation timed out".into()))?
    }

    async fn run_job(
        &self,
        manager: &ManagerProxy<'_>,
        id: &UnitId,
        action: UnitAction,
    ) -> Result<()> {
        // Install the match before dispatch; fast jobs can finish before the reply.
        let mut jobs = manager.receive_job_removed().await?;
        let job = match action {
            UnitAction::Start => manager.start_unit(&id.name, "replace").await,
            UnitAction::Stop => manager.stop_unit(&id.name, "replace").await,
            UnitAction::Restart => manager.restart_unit(&id.name, "replace").await,
            _ => unreachable!("enablement actions returned above"),
        }
        .map_err(write_error)?;
        while let Some(signal) = jobs.next().await {
            let args = signal
                .args()
                .map_err(|e| BackendError::OutcomeUnknown(e.to_string()))?;
            if args.job() == &job {
                return if args.result() == "done" {
                    Ok(())
                } else {
                    Err(BackendError::JobFailed(args.result().clone()))
                };
            }
        }
        Err(BackendError::OutcomeUnknown(
            "the connection closed while waiting for the job".into(),
        ))
    }
}

fn write_error(error: zbus::Error) -> BackendError {
    match error {
        zbus::Error::MethodError(..) => BackendError::Dbus(error),
        _ => BackendError::OutcomeUnknown(error.to_string()),
    }
}

impl SystemdBackend for DbusBackend {
    fn list_units(&self, scope: Scope) -> BoxFuture<'_, Result<Vec<UnitSummary>>> {
        async move {
            self.read(async {
                let connection = self.connection(scope).await?;
                let units = ManagerProxy::new(&connection).await?.list_units().await?;
                Ok(units
                    .into_iter()
                    .map(|u| UnitSummary {
                        id: UnitId { scope, name: u.0 },
                        description: u.1,
                        load: u.2,
                        active: u.3,
                        sub: u.4,
                    })
                    .collect())
            })
            .await
        }
        .boxed()
    }

    fn details(&self, id: UnitId) -> BoxFuture<'_, Result<UnitDetails>> {
        self.read(self.read_details(id)).boxed()
    }

    fn unit_files(&self, id: UnitId) -> BoxFuture<'_, Result<UnitFileContent>> {
        async move {
            self.read(async {
                let connection = self.connection(id.scope).await?;
                let unit = self.unit_proxy(&connection, &id).await?;
                let (fragment, drop_ins) =
                    tokio::try_join!(unit.fragment_path(), unit.drop_in_paths())?;
                let paths = std::iter::once(fragment)
                    .chain(drop_ins)
                    .filter(|p| !p.is_empty())
                    .map(PathBuf::from)
                    .collect();
                read_files(&id.name, paths).await
            })
            .await
        }
        .boxed()
    }

    fn execute(&self, id: UnitId, action: UnitAction) -> BoxFuture<'_, Result<()>> {
        // Mutating methods have their own 120s D-Bus timeout. Reload gets a
        // separate bounded wait so an already successful file change is never
        // misreported as an entirely unknown outcome when reload times out.
        self.run_action(id, action).boxed()
    }

    fn subscribe(&self, scope: Scope) -> BoxFuture<'_, Result<BoxStream<'static, BackendEvent>>> {
        async move {
            self.read(async {
                // A dedicated connection makes dropping this stream release its subscription.
                let connection = self.connect(scope).await?;
                let rule = MatchRule::builder()
                    .msg_type(Type::Signal)
                    .sender(SERVICE)?
                    .path_namespace(PATH)?
                    .build();
                let changes = MessageStream::for_match_rule(rule, &connection, Some(256)).await?;
                let owner_rule = MatchRule::builder()
                    .msg_type(Type::Signal)
                    .sender("org.freedesktop.DBus")?
                    .interface("org.freedesktop.DBus")?
                    .member("NameOwnerChanged")?
                    .add_arg(SERVICE)?
                    .build();
                let owners =
                    MessageStream::for_match_rule(owner_rule, &connection, Some(8)).await?;
                ManagerProxy::new(&connection).await?.subscribe().await?;
                let events = futures_util::stream::select(changes, owners);
                let stream =
                    futures_util::stream::unfold(Some((events, connection)), |state| async move {
                        let (mut events, connection) = state?;
                        loop {
                            match events.next().await {
                                Some(Ok(message)) => {
                                    let header = message.header();
                                    let member = header.member().map(|m| m.as_str()).unwrap_or("");
                                    if member == "NameOwnerChanged" {
                                        return Some((
                                            BackendEvent::Disconnected(
                                                "The systemd manager changed. Retry to reconnect."
                                                    .into(),
                                            ),
                                            None,
                                        ));
                                    }
                                    if matches!(
                                        member,
                                        "UnitNew"
                                            | "UnitRemoved"
                                            | "PropertiesChanged"
                                            | "JobRemoved"
                                            | "UnitFilesChanged"
                                            | "Reloading"
                                    ) {
                                        return Some((
                                            BackendEvent::Changed,
                                            Some((events, connection)),
                                        ));
                                    }
                                }
                                Some(Err(e)) => {
                                    return Some((BackendEvent::Disconnected(e.to_string()), None));
                                }
                                None => {
                                    return Some((
                                        BackendEvent::Disconnected(
                                            "The D-Bus connection closed.".into(),
                                        ),
                                        None,
                                    ));
                                }
                            }
                        }
                    });
                Ok(stream.boxed())
            })
            .await
        }
        .boxed()
    }

    fn reset(&self, scope: Scope) -> BoxFuture<'_, ()> {
        async move {
            self.connection_slot(scope).lock().await.take();
        }
        .boxed()
    }
}

async fn read_files(name: &str, paths: Vec<PathBuf>) -> Result<UnitFileContent> {
    let mut content = UnitFileContent {
        title: name.into(),
        text: String::new(),
        warnings: Vec::new(),
    };
    for path in paths {
        match tokio::fs::read_to_string(&path).await {
            Ok(text) => {
                content
                    .text
                    .push_str(&format!("# {}\n{text}\n\n", path.display()));
            }
            Err(error) => content
                .warnings
                .push(format!("{}: {error}", path.display())),
        }
    }
    if content.text.is_empty() {
        return Err(BackendError::Unavailable(if content.warnings.is_empty() {
            "This unit has no unit file on disk.".into()
        } else {
            content.warnings.join("\n")
        }));
    }
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_fragment_and_drop_ins_in_order_and_reports_partial_failure() {
        let dir = tempfile::tempdir().unwrap();
        let fragment = dir.path().join("example.service");
        let drop_in = dir.path().join("override.conf");
        tokio::fs::write(&fragment, "[Service]\nExecStart=/usr/bin/true")
            .await
            .unwrap();
        tokio::fs::write(&drop_in, "[Service]\nNice=5")
            .await
            .unwrap();
        let content = read_files(
            "example.service",
            vec![fragment, drop_in, dir.path().join("missing")],
        )
        .await
        .unwrap();
        assert!(content.text.find("ExecStart").unwrap() < content.text.find("Nice=5").unwrap());
        assert_eq!(content.warnings.len(), 1);
        assert!(content.text.contains("# "));
        assert!(read_files("transient.scope", vec![]).await.is_err());
        assert!(
            read_files("missing.service", vec![dir.path().join("absent")])
                .await
                .is_err()
        );
    }
}
