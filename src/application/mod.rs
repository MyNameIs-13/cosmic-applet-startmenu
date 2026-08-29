// SPDX-License-Identifier: GPL-3.0-only

//! Use cases orchestrating domain objects. Depends only on `domain`.
//! `interface` should depend only on what this module re-exports here —
//! never reach past it into `domain` or `infrastructure`.

mod dto;
mod list_app_entries;
mod power_control;
mod resolve_start_icon;
mod start_menu_service;

pub use dto::{PowerAction, ProgramEntry, StartIcon};
pub use start_menu_service::StartMenuService;
