// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;

use crate::domain::{AppEntryRepository, EmbeddedIcons, HostDetector, IconLookup, PowerControl};

use super::dto::{PowerAction, ProgramEntry, StartIcon};
use super::list_app_entries::{load_programs, search_programs};
use super::power_control::perform_power_action;
use super::resolve_start_icon::resolve_start_icon;

/// Composition-root-facing facade over this applet's use cases. `interface`
/// holds one of these (built once, in `main`/`AppModel::init`) instead of
/// reaching into `domain` ports directly, so it only ever depends on
/// `application` types.
pub struct StartMenuService {
    app_repo: Arc<dyn AppEntryRepository>,
    power: Arc<dyn PowerControl>,
    host_detector: Arc<dyn HostDetector>,
    icon_lookup: Arc<dyn IconLookup>,
    embedded_icons: Arc<dyn EmbeddedIcons>,
}

impl StartMenuService {
    pub fn new(
        app_repo: Arc<dyn AppEntryRepository>,
        power: Arc<dyn PowerControl>,
        host_detector: Arc<dyn HostDetector>,
        icon_lookup: Arc<dyn IconLookup>,
        embedded_icons: Arc<dyn EmbeddedIcons>,
    ) -> Self {
        Self {
            app_repo,
            power,
            host_detector,
            icon_lookup,
            embedded_icons,
        }
    }

    pub fn resolve_start_icon(&self) -> StartIcon {
        resolve_start_icon(
            &*self.host_detector,
            &*self.icon_lookup,
            &*self.embedded_icons,
        )
    }

    pub fn load_programs(&self) -> Vec<ProgramEntry> {
        load_programs(&*self.app_repo)
    }

    pub fn search_programs(&self, programs: &[ProgramEntry], query: &str) -> Vec<ProgramEntry> {
        search_programs(programs, query)
    }

    pub async fn perform_power_action(&self, action: PowerAction) -> Result<(), String> {
        perform_power_action(&*self.power, action.into()).await
    }
}
