use std::sync::Mutex;
use std::fs::File;
use std::io::Write;

use std::time::SystemTime;

use once_cell::sync::Lazy;
use twilight_model::{user::User, id::{Id, marker::ChannelMarker}, channel::Message};

use crate::get_output_path;

use smb_log_format::{ModLogEntry, ModLogAction, ModLogMessage};

static LOGFILE: Lazy<Option<Mutex<File>>> = Lazy::new(|| {
    let a = std::fs::OpenOptions::new().create(true).truncate(false).append(true).open(get_output_path().join("modlog.ndjson")).map(Into::into);
    if let Err(e) = &a {
        tracing::warn!("Unable to open logfile! Error message was: {}  Modlog will be written to Discord only!", e);
    }
    a.ok()
});

pub trait ModLogEntryExt {
    fn new(moderator: &User, channel: Option<Id<ChannelMarker>>, timestamp: SystemTime, action: ModLogAction) -> Self;
    fn log(&self);
}

pub trait ModLogMessageExt {
    async fn from_message(msg: &Message) -> Self;
}

impl ModLogEntryExt for ModLogEntry {
    fn new(moderator_user: &User, channel_id: Option<Id<ChannelMarker>>, timestamp: SystemTime, action: ModLogAction) -> Self {
        Self {
            channel_id: channel_id.map(|x|x.get()).unwrap_or(0),
            moderator_id: moderator_user.id.get(),
            moderator_name: moderator_user.name.clone(),
            moderator_discrim: moderator_user.discriminator,
            timestamp,
            action,
        }
    }
    fn log(&self) {
        if let Some(logfile) = &*LOGFILE {
            let mut logfile = logfile.lock().unwrap(); // .lock() will only fail if another thread panicked wile holding the mutex
            if let Err(e) = self.write(&mut *logfile){
                tracing::error!("Error writing moderation action to logfile! {}", e);
                return;
            }
        }
    }
}

impl ModLogMessageExt for ModLogMessage {
    async fn from_message(message: &Message) -> Self {
        let mut attachments = Vec::new();
        if !message.attachments.is_empty() {
            let path = get_output_path().join(message.id.to_string());
            let _ = std::fs::create_dir(&path);
            for attachment in message.attachments.iter() {
                let res = download_file(&attachment.proxy_url, path.join(&attachment.filename)).await;
                match res {
                    Ok(()) => attachments.push(attachment.filename.clone()),
                    Err(e) =>  {
                        tracing::error!("Error downloading attachment \"{}\" on message {}: {}", attachment.filename, message.id, e);
                    }
                }
            }
        }
        Self {
            id: message.id.get(),
            content: message.content.clone(),
            author_id: message.author.id.get(),
            author_name: message.author.name.clone(),
            author_discrim: message.author.discriminator,
            attachments,
        }
    }
}

async fn download_file(url: &str, filename: std::path::PathBuf) -> anyhow::Result<()> {
    let mut resp = reqwest::get(url).await?;
    let mut out = std::fs::File::create(filename)?;
    while let Some(data) = resp.chunk().await? {
        out.write_all(&data)?;
    }
    Ok(())
}
