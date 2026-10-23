//! One folder per use case.

pub mod admin_list_connections;
pub mod connect_imap;
pub mod disconnect;
pub mod discover_imap;
pub mod dispatch_due;
pub mod get_connection;
pub mod imap_idle;
pub mod list_connections;
pub mod member_left;
pub mod queue_send;
pub mod request_changes;
pub mod retag_bindings;
pub mod send_message;
pub mod start_connection;
pub mod sync_now;
pub mod sync_stream;
pub mod workspace_deleting;
