//! Chat persistence + action log, scoped per user.
//! Sessions group a conversation; actions record real command executions
//! linked to the session that caused them. Plain chat never creates actions.
pub mod model;
pub mod routes;
pub mod store;
