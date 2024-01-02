use std::sync::Arc;

use twilight_model::application::interaction::{application_command::CommandData, Interaction};
use twilight_model::channel::message::MessageFlags;
use twilight_model::channel::message::embed::EmbedField;
use twilight_model::guild::{Permissions, PartialMember};
use twilight_model::id::Id;
use twilight_model::id::marker::{GuildMarker, ChannelMarker, RoleMarker};
use twilight_util::builder::InteractionResponseDataBuilder;
use twilight_util::builder::embed::EmbedBuilder;
use twl_fw::InteractionHandler;
use twl_fw::response;

use anyhow::anyhow;

use crate::{get_config, save_config};

use toml_edit::value;


pub(crate) async fn delete_message(handler: Arc<InteractionHandler>, inter: Interaction, data: CommandData) -> anyhow::Result<()> {
    // incantation to get the message object we were invoked with
    let offending_message = data.resolved.unwrap().messages.remove(&Id::new(data.target_id.unwrap().get())).unwrap();

    let guild_id = inter.guild_id
        .or(data.guild_id)
        .ok_or(anyhow!("Cannot figure out what guild this command is being run in."))?;

    let moderator_user = inter.user.as_ref()
        .or(inter.member.as_ref().and_then(|x: &PartialMember|x.user.as_ref()))
        .ok_or(anyhow!("Can't figure out who sent this interaction"))?;

    if let Some(modlog_channel_id) = get_modlog_channel(guild_id) {
        handler.client.create_message(modlog_channel_id).embeds(&[
                EmbedBuilder::new()
                        .title(format!("Message removed by moderator"))
                        .description(offending_message.content)
                        .field(EmbedField{name: "Sent by".to_string(), value: format_user(&offending_message.author), inline: false})
                        .field(EmbedField{name: "Deleted by".to_string(), value: format_user(&moderator_user), inline: false})
                        .build()
        ])?.await?;
    } else {
        response!(handler, inter, "The modlog channel in this server has not been set up yet.  Moderation action will be logged to the logfile only.");
    };
    
    dbg!(&moderator_user.email);


    handler.client.delete_message(offending_message.channel_id, offending_message.id).await?;

    response!(ephemeral; handler, inter, "Deletion action logged");
    Ok(())
}

fn format_user(user: &twilight_model::user::User) -> String {
    match user.discriminator {
        0 => format!("@{} (User ID: {})", user.name, user.id),
        disc => format!("{}#{:04} (User ID: {})", user.name, disc, user.id),
    }
}

pub(crate) async fn purge_hour(handler: Arc<InteractionHandler>, inter: Interaction, data: CommandData) -> anyhow::Result<()> {
    response!(handler, inter, "Purge hour command received.  It does not do anything yet.");
    Ok(())
}

pub(crate) async fn configure(handler: Arc<InteractionHandler>, inter: Interaction, data: CommandData) -> anyhow::Result<()> {
    let guild_id = inter.guild_id
        .or(data.guild_id)
        .ok_or(anyhow!("Cannot figure out what guild this command is being run in."))?;

    let me = handler.cache().current_user().ok_or(anyhow!("Didn't get current_user from READY event..."))?;


    // this is Rust, you're just going to have to get used to seeing lines like this one
    let channel_id = inter.channel.as_ref().ok_or(anyhow!("Command is not being run in a channel????"))?.id;

    // make sure the channel is in the cache, otherwise we can't get permissions for it.
    /*
    if handler.cache().channel(channel_id).is_none() {
        handler.cache().update(&handler.client.channel(channel_id).await?.model().await?);
    }

    if !handler.cache().permissions().in_channel(me.id, channel_id)?.contains(Permissions::SEND_MESSAGES) {
        response!(handler, inter, "The bot does not have permission to send messages in this channlel.  Please fix the permissions and run this command from the channel you want to be the mod log channel.");
        return Ok(());
    }
    */
    
    update_config(guild_id, Id::new(12345), channel_id);
    
    response!(handler, inter, "Configuration successful.  This is now the modlog channel.");
    Ok(())
}

// this is a separate function to work around a bug in rustc that marks a function as non-Send 
// (and thus does not allow us to use it with tokio) 
// any non-Send value (the mutex guard) is assigned to a variable, regardless of whether it is held across an await
// point
fn update_config(guild_id: Id<GuildMarker>, role_id: Id<RoleMarker>, channel_id: Id<ChannelMarker>) {
    let mut config = get_config().lock().unwrap();
    let guild_config = &mut config[guild_id.to_string().as_str()];
    //guild_config["role_id"] = value(0);
    // fun fact!  this cast will cause an overflow eventually and cause the IDs stored in the
    // config file to be negative for no reason!
    // but the only alternative I can see to doing it this way is converting the number to a string
    // which is arguably even uglier!
    guild_config["modlog_channel_id"] = value(channel_id.get() as i64);
    if let Some(t) = guild_config.as_inline_table() {
        *guild_config = toml_edit::Item::Table(t.clone().into_table());
    }
    std::mem::drop(config); // release our held mutex so that save_config() can acquire it again
    save_config();
}

// ditto
fn get_modlog_channel(guild_id: Id<GuildMarker>) -> Option<Id<ChannelMarker>> {
    let config = get_config().lock().unwrap();

    let guild_config = config.get(guild_id.get().to_string().as_str())?;
    let channel_id = guild_config.get("modlog_channel_id")?.as_integer()? as u64;
    Some(Id::new(channel_id))
}
