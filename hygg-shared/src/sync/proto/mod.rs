//! The JSON wire contract between the hygg client (`cli-text-reader` / `hygg`)
//! and the sync server (`hygg-server`). Every shape that crosses the network is
//! defined here exactly once, so a change on either side is a *compile* error
//! on the other: rename a field on the server and the client stops building
//! until it is reconciled.
//!
//! It lives in `hygg-shared` (MIT) on purpose. The client is AGPL-3.0 and the
//! server is Elastic-licensed; routing their shared types through a permissive
//! crate gives both sides static typing across the boundary without either
//! license reaching across it.
//!
//! Conventions:
//! - Outbound counts/offsets are unsigned (`u64`/`u32`); the server stores them
//!   as `i64`, so inbound DTOs mirror the columns as `i64`.
//! - `updated_at` / `created_at` / `server_time` are epoch milliseconds.
//! - Tombstones travel as `deleted: bool`; the server's `i64` (0/1) column is
//!   normalised to a bool at the DTO boundary.
//!
//! Module split (purely to keep each file within the LOC budget):
//! [`device`] identity/devices, [`books`] documents, [`push`] outbound ops,
//! [`pull`] inbound rows, [`events`] the SSE stream, [`denial`] refusals.

mod books;
mod denial;
mod device;
mod pull;
mod push;

pub mod events;

pub use crate::sync::mode::SyncMode;
pub use books::{
  BookDto, PutBlobResponse, SetSyncModeRequest, UpsertBookRequest,
  UpsertBookResponse,
};
pub use denial::DenialBody;
pub use device::{
  DeviceDto, MeResponse, RegisterDeviceRequest, RegisterDeviceResponse,
  RevokeDeviceResponse,
};
pub use pull::{
  BookmarkDto, HighlightDto, NoteDto, ProgressDto, PullQuery, PullResponse,
};
pub use push::{
  BookmarkData, HighlightData, NoteData, OpPayload, ProgressData, PushRequest,
  PushResponse, ReadingDayData, ReadingTimeData, SyncOp,
};
