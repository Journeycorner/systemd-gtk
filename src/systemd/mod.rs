pub(crate) mod unit;

use crate::systemd::unit::UnitObject;
use std::str::FromStr;
use std::string::ToString;
use std::sync::LazyLock;
use strum_macros::*;
use systemctl::{SystemCtl, Unit};

#[derive(Debug, PartialEq, EnumString)]
#[strum(serialize_all = "snake_case")]
enum State {
    /// Started, bound, plugged in, ..., depending on the unit type.
    Active,
    /// Stopped, unbound, unplugged, ..., depending on the unit type.
    Inactive,
    /// Similar to inactive, but the unit failed in some way (process returned error code on exit, crashed, an operation timed out, or after too many restarts).
    Failed,
    /// Changing from inactive to active.
    Activating,
    /// Changing from active to inactive.
    Deactivating,
    /// Unit is inactive and a maintenance operation is in progress.
    Maintenance,
    /// Unit is active and it is reloading its configuration.
    Reloading,
}

#[derive(Debug)]
pub enum SystemCtrlAction {
    Start,
    Stop,
    Restart,
    Enable,
    Disable,
}

impl SystemCtrlAction {
    pub fn available_actions(unit_object: &UnitObject) -> Vec<SystemCtrlAction> {
        let state: State = State::from_str(unit_object.state().as_str()).unwrap();
        use crate::systemd::SystemCtrlAction::*;
        match state {
            State::Active => vec![Stop, Restart, Disable],
            State::Inactive => vec![Start, Enable],
            State::Failed => vec![],
            State::Activating => vec![],
            State::Deactivating => vec![],
            State::Maintenance => vec![],
            State::Reloading => vec![],
        }
    }
}

static SYSTEM_CTL: LazyLock<SystemCtl, fn() -> SystemCtl> = LazyLock::new(|| {
    SystemCtl::builder()
        .additional_args(vec![
            String::from("--all"),
            String::from("--no-legend"),
            String::from("--no-pager"),
        ])
        .build()
});

/// Lists all units.
///
/// This function retrieves a list of all systemd units.
/// It wraps the `systemctl list-units` command, which provides detailed information about all units.
///
/// # Returns
/// - A `Vec<UnitObject>` containing all units.
///
/// # Errors
/// - Returns an error if the units could not be listed.
///
/// # Related `systemctl` command
/// The equivalent systemctl command is:
/// ```
/// systemctl list-units
/// ```
/// This command will list all units currently loaded in memory, including information such as load state, active state, and sub-state.
///
/// See `man systemctl` for more details.
pub fn units() -> Vec<UnitObject> {
    SYSTEM_CTL
        .list_units_full(None, None, None)
        .unwrap()
        .iter()
        .map(|u| UnitObject::new(u.to_owned()))
        .collect::<Vec<UnitObject>>()
}

/// Starts the specified unit.
///
/// This function attempts to start the given systemd unit.
/// It wraps the `systemctl start` command, which activates a unit.
///
/// # Parameters
/// - `unit`: The unit object to be started.
///
/// # Errors
/// - Returns an error if the unit could not be started.
///
/// # Related `systemctl` command
/// The equivalent systemctl command is:
/// ```
/// systemctl start [UNIT]
/// ```
/// This command will start (activate) the specified unit immediately.
///
/// See `man systemctl` for more details.
pub fn start(unit: UnitObject) {
    SYSTEM_CTL
        .start(unit.unit_name().as_str())
        .expect("Could not start unit");
}

/// Stops the specified unit.
///
/// This function attempts to stop the given systemd unit.
/// It wraps the `systemctl stop` command, which deactivates a unit.
///
/// # Parameters
/// - `unit`: The unit object to be stopped.
///
/// # Errors
/// - Returns an error if the unit could not be stopped.
///
/// # Related `systemctl` command
/// The equivalent systemctl command is:
/// ```
/// systemctl stop [UNIT]
/// ```
/// This command will stop (deactivate) the specified unit immediately.
///
/// See `man systemctl` for more details.
pub fn stop(unit: UnitObject) {
    SYSTEM_CTL
        .stop(unit.unit_name().as_str())
        .expect("Could not stop unit");
}

/// Restarts the specified unit.
///
/// This function attempts to restart the given systemd unit.
/// It wraps the `systemctl restart` command, which stops and then starts a unit.
///
/// # Parameters
/// - `unit`: The unit object to be restarted.
///
/// # Errors
/// - Returns an error if the unit could not be restarted.
///
/// # Related `systemctl` command
/// The equivalent systemctl command is:
/// ```
/// systemctl restart [UNIT]
/// ```
/// This command will stop and then start the specified unit.
///
/// See `man systemctl` for more details.
pub fn restart(unit: UnitObject) {
    SYSTEM_CTL
        .restart(unit.unit_name().as_str())
        .expect("Could not restart unit");
}

/// Enables the specified unit.
///
/// This function attempts to enable the given systemd unit.
/// It wraps the `systemctl enable` command, which allows a unit to be started on boot.
///
/// # Parameters
/// - `unit`: The unit object to be enabled.
///
/// # Errors
/// - Returns an error if the unit could not be enabled.
///
/// # Related `systemctl` command
/// The equivalent systemctl command is:
/// ```
/// systemctl enable [UNIT]
/// ```
/// This command will enable the specified unit, making it start automatically on boot.
///
/// See `man systemctl` for more details.
pub fn enable(unit: UnitObject) {
    SYSTEM_CTL
        .enable(unit.unit_name().as_str())
        .expect("Could not enable unit");
}

/// Disables the specified unit.
///
/// This function attempts to disable the given systemd unit.
/// It wraps the `systemctl disable` command, which prevents a unit from starting on boot.
///
/// # Parameters
/// - `unit`: The unit object to be disabled.
///
/// # Errors
/// - Returns an error if the unit could not be disabled.
///
/// # Related `systemctl` command
/// The equivalent systemctl command is:
/// ```
/// systemctl disable [UNIT]
/// ```
/// This command will disable the specified unit, preventing it from starting automatically on boot.
///
/// See `man systemctl` for more details.
pub fn disable(unit: UnitObject) {
    SYSTEM_CTL
        .disable(unit.unit_name().as_str())
        .expect("Could not disable unit");
}

/// Displays the contents of the unit file for the specified unit.
///
/// This function attempts to retrieve and display the contents of the unit file.
/// It wraps the `systemctl cat` command, which outputs the unit file's content.
///
/// # Parameters
/// - `unit`: The unit object whose configuration file content is to be displayed.
///
/// # Returns
/// - A `Result<String>` containing the content of the unit file.
///
/// # Errors
/// - Returns an error if the unit file could not be displayed.
///
/// # Related `systemctl` command
/// The equivalent systemctl command is:
/// ```
/// systemctl cat [UNIT]
/// ```
/// This command will display the contents of the specified unit's file.
///
/// See `man systemctl` for more details.
pub fn cat(unit: UnitObject) -> std::io::Result<String> {
    SYSTEM_CTL.cat(unit.unit_name().as_str())
}

/// Displays detailed information about the specified unit.
///
/// This function attempts to retrieve detailed information about the given systemd unit.
/// It wraps the `systemctl show` command, which provides unit properties and their values.
///
/// # Parameters
/// - `unit`: The unit object to be inspected.
///
/// # Returns
/// - A `Unit` object containing detailed properties of the unit.
///
/// # Panics
/// - This function will panic if the unit information could not be retrieved.
///
/// # Related `systemctl` command
/// The equivalent systemctl command is:
/// ```
/// systemctl show [UNIT]
/// ```
/// This command will output all available properties of the specified unit, such as status, dependencies, and more.
///
/// See `man systemctl` for more details.
pub fn show(unit: UnitObject) -> Unit {
    SYSTEM_CTL
        .create_unit(unit.unit_name().as_str())
        .expect("Could not retrieve unit information")
}
