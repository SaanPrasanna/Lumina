//! IPC commands exposed to the UI, grouped by feature. Each file maps 1:1 to
//! `apps/desktop/src/features/<feature>/api.ts`.

pub mod app;
pub mod editor;
pub mod env_path;
pub mod packages;
pub mod profiles;
pub mod quickapp;
pub mod services;
pub mod settings;
pub mod sites;
pub mod ssl;
pub mod tunnel;
