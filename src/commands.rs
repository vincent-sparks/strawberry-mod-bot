
use twilight_interactions::command::{CommandModel, CreateCommand};

#[derive(CommandModel, CreateCommand)]
#[command(autocomplete=false, name="reason", desc="Write a note to the modlog about your last moderation action.")]
pub(crate) struct ReasonCommand {
    /// Message to write to the modlog
    pub(crate) reason: String,
}


#[derive(CommandModel, CreateCommand)]
#[command(name="configure_moderation", desc="Add a reason for your last mod action, written to the modlog.")]
pub(crate) struct InitCommand {
    // TODO add parameter for roles who are allowed to use mod commands
}
