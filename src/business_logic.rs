use std::sync::Arc;

use twilight_interactions::command::CommandModel;
use twilight_model::application::interaction::{application_command::CommandData, Interaction};
use twilight_model::channel::message::MessageFlags;
use twilight_model::channel::message::embed::EmbedField;
use twilight_model::guild::{Permissions, PartialMember, Member};
use twilight_model::id::Id;
use twilight_model::id::marker::{GuildMarker, ChannelMarker, RoleMarker};
use twilight_model::user::User;
use twilight_util::builder::InteractionResponseDataBuilder;
use twilight_util::builder::embed::EmbedBuilder;
use twl_fw::InteractionHandler;
use twl_fw::response;

use anyhow::anyhow;

use crate::commands::{ReasonCommand, ChannelCommand, AddModRoleCommand};
use crate::{get_config, save_config};

use toml_edit::value;

fn get_guild(inter: &Interaction, data: &CommandData) -> anyhow::Result<Id<GuildMarker>> {
    inter.guild_id
        .or(data.guild_id)
        .ok_or(anyhow!("Cannot figure out what guild this command is being run in."))
}

fn get_initiating_user(inter: &Interaction) -> anyhow::Result<&User> {
    inter.user.as_ref()
        .or(inter.member.as_ref().and_then(|x: &PartialMember|x.user.as_ref()))
        .ok_or(anyhow!("Can't figure out who sent this interaction"))
}


pub(crate) async fn delete_message(handler: Arc<InteractionHandler>, inter: Interaction, data: CommandData) -> anyhow::Result<()> {
    let guild_id = get_guild(&inter, &data)?;
    let moderator_user = get_initiating_user(&inter)?;

    if !inter.member.as_ref().is_some_and(|member| is_user_a_moderator(&handler, member, guild_id)) {
        response!(ephemeral; handler, inter, "You do not have permission to use that command.");
        return Ok(());
    }

    // incantation to get the message object we were invoked with
    let offending_message = data.resolved.unwrap().messages.remove(&Id::new(data.target_id.unwrap().get())).unwrap();

    if let Some(modlog_channel_id) = get_modlog_channel(guild_id) {
        let mut builder = EmbedBuilder::new()
                    .title(format!("Message removed by moderator"))
                    .description(offending_message.content)
                    .field(EmbedField{name: "Sent by".to_string(), value: format_user(&offending_message.author), inline: false})
                    .field(EmbedField{name: "Deleted by".to_string(), value: format_user(&moderator_user), inline: false});
        if let Some(channel) = &inter.channel {
            // this *should* always be present but i'm not taking ANY chances
            builder = builder.field(EmbedField {name: "Channel".to_string(), value: format!("<#{}>", channel.id), inline: false});
        }
        for attachment in offending_message.attachments {
            builder = builder.field(EmbedField {name: format!("Attachment: {}", attachment.filename), value: attachment.proxy_url, inline: false});
        }
        handler.client.create_message(modlog_channel_id).embeds(&[builder.build()])?.await?;
    } else {
        response!(handler, inter, "The modlog channel in this server has not been set up yet.  Moderation action will be logged to the logfile only.");
    };
    
    dbg!(&moderator_user.email);


    handler.client.delete_message(offending_message.channel_id, offending_message.id).await?;

    response!(ephemeral; handler, inter, "Message deleted");
    Ok(())
}

fn format_user(user: &twilight_model::user::User) -> String {
    match user.discriminator {
        0 => format!("@{} (<@{}>)", user.name, user.id),
        disc => format!("{}#{:04} (<@{}>)", user.name, disc, user.id),
    }
}

pub(crate) async fn purge_hour(handler: Arc<InteractionHandler>, inter: Interaction, data: CommandData) -> anyhow::Result<()> {
    let guild_id = get_guild(&inter, &data)?;
    if !inter.member.as_ref().is_some_and(|member| is_user_a_moderator(&handler, member, guild_id)) {
        response!(ephemeral; handler, inter, "You do not have permission to use that command.");
        return Ok(());
    }
    response!(handler, inter, "Purge hour command received.  It does not do anything yet.");
    Ok(())
}

pub(crate) async fn reason(handler: Arc<InteractionHandler>, inter: Interaction, data: CommandData) -> anyhow::Result<()> {
    let guild_id = get_guild(&inter, &data)?;
    let moderator_user = get_initiating_user(&inter)?;

    if !inter.member.as_ref().is_some_and(|member| is_user_a_moderator(&handler, member, guild_id)) {
        response!(ephemeral; handler, inter, "You do not have permission to use that command.");
        return Ok(());
    }

    let cmd = ReasonCommand::from_interaction(data.into())?;


    if let Some(modlog_channel_id) = get_modlog_channel(guild_id) {
        let mut builder = EmbedBuilder::new()
                        .title(format!("Reason added by moderator"))
                        .field(EmbedField {name: "Moderator".to_string(), value: format_user(&moderator_user), inline: false})
                        .description(cmd.reason);
        if let Some(channel) = &inter.channel {
            // this *should* always be present but i'm not taking ANY chances
            builder = builder.field(EmbedField {name: "Channel".to_string(), value: format!("<#{}>", channel.id), inline: false});
        }
        handler.client.create_message(modlog_channel_id).embeds(&[builder.build()])?.await?;
        response!(ephemeral; handler, inter, "Reason recorded in the modlog.");
    } else {
        response!(ephemeral; handler, inter, "The modlog channel in this server has not been set up yet.  Moderation action will be logged to the logfile only.");
    };
    
    // TODO logfile

    Ok(())
}

pub(crate) async fn channel(handler: Arc<InteractionHandler>, inter: Interaction, data: CommandData) -> anyhow::Result<()> {
    let guild_id = get_guild(&inter, &data)?;

    let cmd = ChannelCommand::from_interaction(data.into())?;

    // this is Rust, you're just going to have to get used to seeing lines like this one
    let channel_id = cmd.channel;

    /*
    // make sure the channel is in the cache, otherwise we can't get permissions for it.
    let me = handler.cache().current_user().ok_or(anyhow!("Didn't get current_user from READY event..."))?;
    if handler.cache().channel(channel_id).is_none() {
        handler.cache().cache_channel(&handler.client.channel(channel_id).await?.model().await?);
    }

    if !handler.cache().permissions().in_channel(me.id, channel_id)?.contains(Permissions::SEND_MESSAGES) {
        response!(handler, inter, "The bot does not have permission to send messages in this channlel.  Please fix the permissions and run this command from the channel you want to be the mod log channel.");
        return Ok(());
    }
    */
    
    update_config(guild_id, |guild_config| guild_config["modlog_channel_id"] = toml_edit::value(channel_id.get() as i64));
    
    response!(handler, inter, "Configuration successful.  <#{}> is now the modlog channel.", channel_id);
    Ok(())
}

pub(crate) async fn add_modrole(handler: Arc<InteractionHandler>, inter: Interaction, data: CommandData) -> anyhow::Result<()> {
    let guild_id = get_guild(&inter, &data)?;
    let cmd = AddModRoleCommand::from_interaction(data.into())?;
    let role_id = cmd.role.get();
    
    let mut message = String::new();

    update_config(guild_id, |guild_config| {
        let Some(mod_roles) = guild_config.as_table_mut().and_then(|table| table.entry("moderator_roles").or_insert(toml_edit::value(toml_edit::Array::new())).as_array_mut()) else {
            message = "Config file is not in a valid format.  No changes were made.".into();
            return;
        };
        if mod_roles.iter().any(|s| s.as_integer().is_some_and(|val| val==role_id as i64)) {
            message = "That role is already a moderator role.  ".into();
        } else {
            mod_roles.push(role_id as i64);
            message = format!("Successfully made <@&{}> a moderator role.  ", role_id);
        }
        let current_role_ids = mod_roles.iter().filter_map(|x| x.as_integer()).collect::<Vec<i64>>();
        message.push_str(&format_list_of_roles(&current_role_ids));
    });

    response!(ephemeral; handler, inter, "{}", message);

    Ok(())
}

pub(crate) async fn del_modrole(handler: Arc<InteractionHandler>, inter: Interaction, data: CommandData) -> anyhow::Result<()> {
    let guild_id = get_guild(&inter, &data)?;
    let cmd = AddModRoleCommand::from_interaction(data.into())?;
    let role_id = cmd.role.get();
    
    let mut message = String::new();

    update_config(guild_id, |guild_config| {
        let Some(mod_roles) = guild_config.as_table_mut().and_then(|table| table.entry("moderator_roles").or_insert(toml_edit::value(toml_edit::Array::new())).as_array_mut()) else {
            message = "Config file is not in a valid format.  No changes were made.".into();
            return;
        };
        let pos = mod_roles.iter().position(|s| s.as_integer().is_some_and(|val| val==role_id as i64));
        if let Some(idx) = pos {
            mod_roles.remove(idx);
            message = format!("Successfully revoked moderator status from <@&{}>.  ", role_id);
        } else {
            message = "That role is already not a moderator role.  ".into();
        }
        let current_role_ids = mod_roles.iter().filter_map(|x| x.as_integer()).collect::<Vec<i64>>();
        message.push_str(&format_list_of_roles(&current_role_ids));
    });
    response!(ephemeral; handler, inter, "{}", message);

    Ok(())
}

// this is a separate function to work around a bug in rustc that marks a function as non-Send 
// (and thus does not allow us to use it with tokio) 
// any non-Send value (the mutex guard) is assigned to a variable, regardless of whether it is held across an await
// point
fn update_config<T>(guild_id: Id<GuildMarker>, action: impl FnOnce(&mut toml_edit::Item) -> T) -> T {
    let mut config = get_config().lock().unwrap();
    let guild_config = &mut config[guild_id.to_string().as_str()];
    //guild_config["role_id"] = value(0);
    // fun fact!  this cast will cause an overflow eventually and cause the IDs stored in the
    // config file to be negative for no reason!
    // but the only alternative I can see to doing it this way is converting the number to a string
    // which is arguably even uglier!
    let res = action(guild_config);
    if let Some(t) = guild_config.as_inline_table() {
        *guild_config = toml_edit::Item::Table(t.clone().into_table());
    }
    std::mem::drop(config); // release our held mutex so that save_config() can acquire it again
    save_config();
    res
}

// ditto
fn get_modlog_channel(guild_id: Id<GuildMarker>) -> Option<Id<ChannelMarker>> {
    let config = get_config().lock().unwrap();

    let guild_config = config.get(guild_id.get().to_string().as_str())?;
    let channel_id = guild_config.get("modlog_channel_id")?.as_integer()? as u64;
    Some(Id::new(channel_id))
}

fn format_list_of_roles(role_ids: &[i64]) -> String {
    if role_ids.is_empty() {
        return "No moderator roles are currently set.  Anyone who can see the moderator commands will be able to use them.".to_string();
    } else if role_ids.len() == 1 {
        format!("<@&{}> is currently the only moderator role.", role_ids[0] as u64)
    } else {
        let s = role_ids[..role_ids.len()-1].iter().map(|id| format!("<@&{}>", *id as u64)).collect::<Vec<_>>().join(", ");
        format!("Current moderator roles are {} and <@&{}>", s, role_ids[role_ids.len()-1] as u64)
    }
}

fn is_user_a_moderator(handler: &InteractionHandler, member: &PartialMember, guild_id: Id<GuildMarker>) -> bool {
    let config = get_config().lock().unwrap();
    let Some(moderator_roles) = config.get(guild_id.get().to_string().as_str()).and_then(|guild_config| guild_config.get("moderator_roles")) else {
        let guild_name = handler.cache().guild(guild_id).map(|guild| guild.name().to_owned()).unwrap_or_else(|| guild_id.to_string());
        tracing::warn!("No moderator roles have been configured in server \"{}\"!  Allowing anyone who can see them to use moderation commands!", guild_name);
        return true;
    };
    let Some(moderator_roles) = moderator_roles.as_array() else {
        let guild_name = handler.cache().guild(guild_id).map(|guild| guild.name().to_owned()).unwrap_or_else(|| guild_id.to_string());
        tracing::warn!("Configuration format in server {} is messed up.  Preventing anyone from using moderation commands.", guild_name);
        return false;
    };

    if moderator_roles.is_empty() {
        return true;
    }

    return moderator_roles.iter().filter_map(|entry| entry.as_integer()).any(|mod_role_id| member.roles.iter().any(|role| role.get() == mod_role_id as u64));
}
