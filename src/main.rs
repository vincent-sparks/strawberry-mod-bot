#![feature(never_type)]
mod business_logic;
mod commands;
mod disk_log;

use std::{io::ErrorKind, sync::{Arc, Mutex}, path::PathBuf};
use std::env::VarError;

use commands::{ReasonCommand, ChannelCommand, AddModRoleCommand, DeleteModRoleCommand};
use twilight_http::Client;
use twilight_interactions::command::CreateCommand;
use twilight_model::{id::{Id, marker::GuildMarker}, application::command::CommandType};
use twilight_util::builder::command::CommandBuilder;

use twilight_gateway::{Intents, Shard, ShardId, Event};
use std::io::Write;


// If set, bot commands will be visible only in this guild.  Leave at None for production use.
//const DEBUG_GUILD: Option<Id<GuildMarker>> = None;
//const DEBUG_GUILD: Option<Id<GuildMarker>> = Some(Id::new(/* your numeric server ID here */));
const DEBUG_GUILD: Option<Id<GuildMarker>> = Some(Id::new(1191491525432070174));

use twl_fw::{CommandFunc, CommandMap, build_command};
use once_cell::sync::Lazy;
use phf::phf_map;

static DELETE_MESSAGE_COMMAND: Lazy<CommandFunc> = build_command!(|handler, inter, data| business_logic::delete_message(handler, inter, data));
static PURGE_HOUR_COMMAND: Lazy<CommandFunc> = build_command!(|handler, inter, data| business_logic::purge_hour(handler, inter, data));
static CHANNEL_COMMAND: Lazy<CommandFunc> = build_command!(|handler, inter, data| business_logic::channel(handler, inter, data));
static ADD_MODROLE_COMMAND: Lazy<CommandFunc> = build_command!(|handler, inter, data| business_logic::add_modrole(handler, inter, data));
static DEL_MODROLE_COMMAND: Lazy<CommandFunc> = build_command!(|handler, inter, data| business_logic::del_modrole(handler, inter, data));
static REASON_COMMAND: Lazy<CommandFunc> = build_command!(|handler, inter, data| business_logic::reason(handler, inter, data));

static COMMAND_MAP: CommandMap = phf_map! {
    "reason" => &REASON_COMMAND,
    "channel" => &CHANNEL_COMMAND,
    "add_moderator_role" => &ADD_MODROLE_COMMAND,
    "delete_moderator_role" => &DEL_MODROLE_COMMAND,
    "Delete message" => &DELETE_MESSAGE_COMMAND,
    "Purge last hour" => &PURGE_HOUR_COMMAND,
};

static mut GLOBAL_CONFIG: Option<Mutex<toml_edit::Document>> = None; 
static mut OUTPUT_PATH: Option<PathBuf> = None;


pub(crate) fn get_config() -> &'static Mutex<toml_edit::Document> {
    unsafe {
        GLOBAL_CONFIG.as_ref().unwrap()
    }
}

// if i had a *lot* more time, i would set something up with inotify to make sure the in memory
// config and on disk config could never get out of sync.  since i don't, you'll have to just never
// touch the config file while the bot is running.

pub(crate) fn save_config() {
    let document = get_config().lock().unwrap();
    let error_status = std::fs::File::create("config.toml").and_then(|mut file| file.write_all(document.to_string().as_bytes()));
    match error_status {
        Ok(()) => {},
        Err(e) => {
            tracing::error!("Failed to flush config to disk!  Error message was: {}.  Continuing anyway.", e);
        }
    }
}

pub(crate) fn get_output_path() -> &'static PathBuf {
    unsafe {OUTPUT_PATH.as_ref().unwrap()}
}

#[tokio::main(flavor="current_thread")]
async fn main() -> Result<!, Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    if let Err(e) = dotenvy::dotenv() {
        tracing::error!("Error reading the .env file: {}", e);
    }


    let authtoken = std::env::var("AUTHTOKEN").unwrap_or_else(|e| {
        match e {
            VarError::NotPresent => eprintln!("Environment variable AUTHTOKEN not set. Cannot sign into Discord.  Exiting."),
            VarError::NotUnicode(_) => eprintln!("Environment variable AUTHTOKEN is not valid UTF-8.  Exiting."),
        }
        std::process::exit(1);
    });

    let outputdir = std::env::var_os("OUTPUT_DIR").map_or_else(||std::env::current_dir().unwrap(), PathBuf::from);
    unsafe {
        OUTPUT_PATH = Some(outputdir);
    }
    
    let config: toml_edit::Document;
    match std::fs::read_to_string("config.toml") {
        Ok(s) => {
            config = s.parse()
                .unwrap_or_else(|e| {
                    tracing::error!("config.toml did not contain valid TOML.  Specific error was: {}.  Exiting.", e);
                    std::process::exit(1);
                });
        },
        Err(e) if e.kind() == ErrorKind::NotFound => {
            tracing::info!("Config file config.toml not found.  Creating it.");
            config = toml_edit::Document::default();
        },
        Err(e) => {
            tracing::error!("Error reading config.toml: {}.  Exiting.", e);
            std::process::exit(1);
        }
    }

    unsafe {
        GLOBAL_CONFIG = Some(Mutex::new(config));
    }
    
    let client = Arc::new(Client::new(authtoken.clone()));

    let application = client.current_user_application().await?.model().await?;
    let interaction_client = client.interaction(application.id);

    // the second str argument is a description, which Discord does not currrently support for
    // message commands (it will error if they aren't blank)
    let commands = [
        ReasonCommand::create_command().into(),
        ChannelCommand::create_command().into(),
        AddModRoleCommand::create_command().into(),
        DeleteModRoleCommand::create_command().into(),
        CommandBuilder::new("Delete message", "", CommandType::Message).build(),
        //CommandBuilder::new("Purge last hour", "", CommandType::Message).build(), // this is commented out until I can make it do something
    ];

    if let Some(guild_id) = DEBUG_GUILD {
        interaction_client.set_guild_commands(guild_id, &commands).await?;
    } else {
        interaction_client.set_global_commands(&commands).await?;
    }

    let handler = Arc::new(twl_fw::InteractionHandler::new(client.clone(), &COMMAND_MAP));
    
    let mut shard = Shard::new(ShardId::ONE, authtoken, Intents::GUILD_MESSAGE_REACTIONS | Intents::DIRECT_MESSAGE_REACTIONS);

    loop {
        let event = match shard.next_event().await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(?e, "error receiving event");
                if e.is_fatal() {
                    std::process::exit(0);
                } else {
                    continue;
                }
            }
        };
        handler.cache().update(&event);
        match event {
            Event::InteractionCreate(inter) => {
                tokio::spawn(handler.clone().handle(inter.0));
            },
            Event::Ready(ready) => {
                let config = get_config().lock().unwrap();
                tracing::info!("bot is in {} guilds, of which {} are configured", ready.guilds.len(), ready.guilds.iter().filter(|x| config.contains_key(x.id.to_string().as_str())).count());
                tracing::info!("Strawberry Moderator reporting for duty!");
            },
            _ => {},
        }
    }
}
