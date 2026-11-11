//! Cloud documents for the native hosts: the event-driven sync client, the account feed and
//! the device-local library that composes them.
mod attachments;
mod client;
mod cloud;
mod journal;
mod library;
pub mod local;
mod resolution;
#[cfg(test)]
use pop_testkit as test_support;
mod transport;

pub use client::{Client, ClientStatus, delete_account, invitation_document, local_document};
pub use cloud::sharing_link;
pub use library::{Entry, Error, Issue, Library};
pub use transport::account::LibraryUpdate;
