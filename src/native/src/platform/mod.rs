//! Platform: transversal backend plumbing shared by every domain.
//! Today: SQLite connection setup. Each domain owns its own schema + store.
pub mod db;
