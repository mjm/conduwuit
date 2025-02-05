use clap::Subcommand;
use ruma::events::room::message::RoomMessageEventContent;

use self::appservice_command::{list, register, show, unregister};
use crate::Result;

pub(crate) mod appservice_command;

#[cfg_attr(test, derive(Debug))]
#[derive(Subcommand)]
pub(crate) enum AppserviceCommand {
	/// - Register an appservice using its registration YAML
	///
	/// This command needs a YAML generated by an appservice (such as a bridge),
	/// which must be provided in a Markdown code block below the command.
	///
	/// Registering a new bridge using the ID of an existing bridge will replace
	/// the old one.
	Register,

	/// - Unregister an appservice using its ID
	///
	/// You can find the ID using the `list-appservices` command.
	Unregister {
		/// The appservice to unregister
		appservice_identifier: String,
	},

	/// - Show an appservice's config using its ID
	///
	/// You can find the ID using the `list-appservices` command.
	Show {
		/// The appservice to show
		appservice_identifier: String,
	},

	/// - List all the currently registered appservices
	List,
}

pub(crate) async fn process(command: AppserviceCommand, body: Vec<&str>) -> Result<RoomMessageEventContent> {
	Ok(match command {
		AppserviceCommand::Register => register(body).await?,
		AppserviceCommand::Unregister {
			appservice_identifier,
		} => unregister(body, appservice_identifier).await?,
		AppserviceCommand::Show {
			appservice_identifier,
		} => show(body, appservice_identifier).await?,
		AppserviceCommand::List => list(body).await?,
	})
}
