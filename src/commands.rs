
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::{ChannelMarker, RoleMarker}, Id};

#[derive(CommandModel, CreateCommand)]
#[command(autocomplete=false, name="reason", desc="Write a note to the modlog about your last moderation action.")]
pub(crate) struct ReasonCommand {
    /// Message to write to the modlog
    pub(crate) reason: String,
}


#[derive(CommandModel, CreateCommand)]
#[command(name="channel", desc="Set the modlog channel")]
pub(crate) struct ChannelCommand {
    /// Modlog channel
    pub(crate) channel: Id<ChannelMarker>,
}


#[derive(CommandModel, CreateCommand)]
#[command(name="add_moderator_role", desc="Add a moderator role to the list of mod roles.")]
pub(crate) struct AddModRoleCommand {
    /// Role to give moderatorerator access to
    pub(crate) role: Id<RoleMarker>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name="delete_moderator_role", desc="Remove a moderator role from the list of mod roles.")]
pub(crate) struct DeleteModRoleCommand {
    /// Role to revoke moderator access from
    pub(crate) role: Id<RoleMarker>,
}
