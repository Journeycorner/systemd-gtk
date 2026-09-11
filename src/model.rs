use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    #[default]
    System,
    User,
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::System => "System",
            Self::User => "User",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnitId {
    pub scope: Scope,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitSummary {
    pub id: UnitId,
    pub description: String,
    pub load: String,
    pub active: String,
    pub sub: String,
}

impl UnitSummary {
    /// The suffix identifies the systemd unit type, including future types.
    pub fn unit_type(&self) -> &str {
        self.id
            .name
            .rsplit_once('.')
            .map(|(_, suffix)| suffix)
            .filter(|suffix| !suffix.is_empty())
            .unwrap_or("unknown")
    }

    pub fn matches(&self, query: &str) -> bool {
        self.matches_normalized(&query.to_lowercase())
    }

    /// Use a pre-normalized query when filtering an entire snapshot.
    pub fn matches_normalized(&self, query: &str) -> bool {
        query.is_empty()
            || self.id.name.to_lowercase().contains(query)
            || self.description.to_lowercase().contains(query)
    }
}

pub fn compare_names(a: &str, b: &str) -> Ordering {
    fn parts(name: &str) -> (&str, &str) {
        name.rsplit_once('.').map_or((name, ""), |(n, s)| (n, s))
    }
    let (an, at) = parts(a);
    let (bn, bt) = parts(b);
    at.cmp(bt)
        .then_with(|| an.to_lowercase().cmp(&bn.to_lowercase()))
        .then_with(|| a.cmp(b))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitAction {
    Start,
    Stop,
    Restart,
    Enable,
    Disable,
}

impl UnitAction {
    pub const ALL: [Self; 5] = [
        Self::Start,
        Self::Stop,
        Self::Restart,
        Self::Enable,
        Self::Disable,
    ];

    pub fn needs_confirmation(self) -> bool {
        matches!(self, Self::Stop | Self::Restart | Self::Disable)
    }
}

impl fmt::Display for UnitAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Start => "Start",
            Self::Stop => "Stop",
            Self::Restart => "Restart",
            Self::Enable => "Enable",
            Self::Disable => "Disable",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitDetails {
    pub unit: UnitSummary,
    pub can_start: bool,
    pub can_stop: bool,
    pub refuse_start: bool,
    pub refuse_stop: bool,
    pub job_pending: bool,
    pub file_state: String,
}

impl UnitDetails {
    pub fn allows(&self, action: UnitAction) -> bool {
        if self.job_pending {
            return false;
        }
        let start = self.can_start && !self.refuse_start && self.unit.load != "masked";
        let stop = self.can_stop && !self.refuse_stop;
        match action {
            UnitAction::Start => {
                start && matches!(self.unit.active.as_str(), "inactive" | "failed")
            }
            UnitAction::Stop => stop && self.unit.active == "active",
            UnitAction::Restart => start && stop && self.unit.active == "active",
            UnitAction::Enable => {
                matches!(self.file_state.as_str(), "disabled" | "enabled-runtime")
            }
            UnitAction::Disable => {
                matches!(self.file_state.as_str(), "enabled" | "enabled-runtime")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitFileContent {
    pub title: String,
    pub text: String,
    pub warnings: Vec<String>,
}

/// Each request belongs to a scope generation, and a particular request within it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ticket(u64, u64);

#[derive(Debug, Default)]
pub struct SessionState {
    pub scope: Scope,
    pub selected: Option<UnitId>,
    pub pending: Option<(UnitId, UnitAction)>,
    epoch: u64,
    list_serial: u64,
    detail_serial: u64,
}

impl SessionState {
    pub fn switch_scope(&mut self, scope: Scope) -> bool {
        if self.pending.is_some() || scope == self.scope {
            return false;
        }
        self.scope = scope;
        self.invalidate();
        self.selected = None;
        true
    }

    pub fn invalidate(&mut self) {
        self.epoch += 1;
    }

    pub fn list_ticket(&mut self) -> Ticket {
        self.list_serial += 1;
        Ticket(self.epoch, self.list_serial)
    }

    pub fn accepts_list(&self, ticket: Ticket) -> bool {
        ticket == Ticket(self.epoch, self.list_serial)
    }

    pub fn select(&mut self, selected: Option<UnitId>) -> Ticket {
        self.selected = selected;
        self.detail_serial += 1;
        Ticket(self.epoch, self.detail_serial)
    }

    pub fn accepts_details(&self, ticket: Ticket, id: &UnitId) -> bool {
        ticket == Ticket(self.epoch, self.detail_serial) && self.selected.as_ref() == Some(id)
    }

    pub fn begin_action(&mut self, details: &UnitDetails, action: UnitAction) -> bool {
        if self.pending.is_some()
            || self.selected.as_ref() != Some(&details.unit.id)
            || !details.allows(action)
        {
            return false;
        }
        self.pending = Some((details.unit.id.clone(), action));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit() -> UnitSummary {
        UnitSummary {
            id: UnitId {
                scope: Scope::System,
                name: "Example.service".into(),
            },
            description: "A useful daemon".into(),
            load: "loaded".into(),
            active: "inactive".into(),
            sub: "dead".into(),
        }
    }

    fn details() -> UnitDetails {
        UnitDetails {
            unit: unit(),
            can_start: true,
            can_stop: true,
            refuse_start: false,
            refuse_stop: false,
            job_pending: false,
            file_state: "disabled".into(),
        }
    }

    #[test]
    fn unit_types_use_the_final_suffix_and_preserve_unfamiliar_types() {
        let mut unit = unit();
        for suffix in [
            "service",
            "device",
            "timer",
            "socket",
            "target",
            "mount",
            "automount",
            "swap",
            "path",
            "slice",
            "scope",
            "future",
        ] {
            unit.id.name = format!("example.instance@one.{suffix}");
            assert_eq!(unit.unit_type(), suffix);
        }
        for name in ["", "plain", "trailing."] {
            unit.id.name = name.into();
            assert_eq!(unit.unit_type(), "unknown");
        }
    }

    #[test]
    fn search_matches_name_and_description_without_case() {
        let unit = unit();
        for query in ["", "EXAMPLE", "USEFUL", ".service"] {
            assert!(unit.matches(query));
        }
        assert!(!unit.matches("missing"));
    }

    #[test]
    fn sorting_groups_types_then_names_and_handles_missing_suffix() {
        let mut names = [
            "z.service",
            "A.timer",
            "B.service",
            "a.service",
            "plain",
            "a.b.service",
        ];
        names.sort_by(|a, b| compare_names(a, b));
        assert_eq!(
            names,
            [
                "plain",
                "a.service",
                "a.b.service",
                "B.service",
                "z.service",
                "A.timer"
            ]
        );
    }

    #[test]
    fn enablement_and_runtime_state_are_independent() {
        for active in ["active", "inactive", "failed", "future-state"] {
            let mut d = details();
            d.unit.active = active.into();
            assert!(d.allows(UnitAction::Enable));
            assert!(!d.allows(UnitAction::Disable));
            d.file_state = "enabled".into();
            assert!(!d.allows(UnitAction::Enable));
            assert!(d.allows(UnitAction::Disable));
        }
        let mut d = details();
        d.file_state = "enabled-runtime".into();
        assert!(d.allows(UnitAction::Enable) && d.allows(UnitAction::Disable));
        for state in ["static", "masked", "linked", "indirect", "future-state", ""] {
            d.file_state = state.into();
            assert!(!d.allows(UnitAction::Enable) && !d.allows(UnitAction::Disable));
        }
    }

    #[test]
    fn runtime_actions_respect_capabilities_restrictions_and_jobs() {
        let mut d = details();
        d.unit.active = "failed".into();
        assert!(d.allows(UnitAction::Start));
        d.refuse_start = true;
        assert!(!d.allows(UnitAction::Start));
        d.refuse_start = false;
        d.unit.load = "masked".into();
        assert!(!d.allows(UnitAction::Start));
        d.unit.load = "loaded".into();
        d.unit.active = "active".into();
        assert!(d.allows(UnitAction::Stop) && d.allows(UnitAction::Restart));
        d.can_stop = false;
        assert!(!d.allows(UnitAction::Stop) && !d.allows(UnitAction::Restart));
        d.can_stop = true;
        d.refuse_stop = true;
        assert!(!d.allows(UnitAction::Stop) && !d.allows(UnitAction::Restart));
        for state in [
            "activating",
            "deactivating",
            "reloading",
            "maintenance",
            "unknown",
        ] {
            d.unit.active = state.into();
            assert!(!d.allows(UnitAction::Start));
            assert!(!d.allows(UnitAction::Stop));
        }
        d.job_pending = true;
        assert!(UnitAction::ALL.into_iter().all(|a| !d.allows(a)));
    }

    #[test]
    fn stale_reads_and_scope_results_are_rejected() {
        let mut state = SessionState::default();
        let old_list = state.list_ticket();
        let new_list = state.list_ticket();
        assert!(!state.accepts_list(old_list));
        assert!(state.accepts_list(new_list));
        let id = unit().id;
        let old_detail = state.select(Some(id.clone()));
        state.select(None);
        assert!(!state.accepts_details(old_detail, &id));
        assert!(state.switch_scope(Scope::User));
        assert!(!state.accepts_list(new_list));
        assert!(!state.accepts_details(old_detail, &id));
    }

    #[test]
    fn operations_keep_the_original_target_and_prevent_duplicates_and_scope_switches() {
        let mut state = SessionState::default();
        let d = details();
        state.select(Some(d.unit.id.clone()));
        assert!(state.begin_action(&d, UnitAction::Start));
        assert!(!state.begin_action(&d, UnitAction::Start));
        assert!(!state.switch_scope(Scope::User));
        state.select(None);
        assert_eq!(state.pending.as_ref().map(|p| &p.0), Some(&d.unit.id));
        state.pending = None;
        assert!(state.switch_scope(Scope::User));
    }
}
