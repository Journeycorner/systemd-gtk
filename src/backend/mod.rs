mod dbus;
pub use dbus::{DbusBackend, Timeouts};

use crate::model::{Scope, UnitAction, UnitDetails, UnitFileContent, UnitId, UnitSummary};
use futures_util::{future::BoxFuture, stream::BoxStream};

pub type Result<T> = std::result::Result<T, BackendError>;

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("D-Bus: {0}")]
    Dbus(#[from] zbus::Error),
    #[error("The request timed out. Try refreshing.")]
    ReadTimeout,
    #[error(
        "The operation's outcome is unknown: {0}. Refresh before trying again; the job may still be running."
    )]
    OutcomeUnknown(String),
    #[error("The systemd job did not succeed: {0}")]
    JobFailed(String),
    #[error("Enablement changed, but the manager could not be reloaded: {0}")]
    PartialSuccess(String),
    #[error("{0}")]
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendEvent {
    Changed,
    Disconnected(String),
}

/// All arguments/results are owned Rust data. Implementations must not access GTK.
/// Test implementations must use explicit fake endpoints, never host defaults.
pub trait SystemdBackend: Send + Sync {
    fn list_units(&self, scope: Scope) -> BoxFuture<'_, Result<Vec<UnitSummary>>>;
    fn details(&self, id: UnitId) -> BoxFuture<'_, Result<UnitDetails>>;
    fn unit_files(&self, id: UnitId) -> BoxFuture<'_, Result<UnitFileContent>>;
    fn execute(&self, id: UnitId, action: UnitAction) -> BoxFuture<'_, Result<()>>;
    fn subscribe(&self, scope: Scope) -> BoxFuture<'_, Result<BoxStream<'static, BackendEvent>>>;
    fn reset(&self, scope: Scope) -> BoxFuture<'_, ()>;
}
