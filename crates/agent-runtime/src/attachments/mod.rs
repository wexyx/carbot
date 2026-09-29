mod prepared;
mod store;
#[cfg(test)]
mod tests;

pub(crate) use prepared::AttachmentImage;
pub(crate) use prepared::PreparedAttachments;
pub use store::{Attachment, AttachmentStore, MAX_FILE_BYTES};
