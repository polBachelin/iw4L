use bevy_ecs::prelude::Resource;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::ClientId;

/// Which side a script asks bots for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptBotSide {
    Enemy,
    Ally,
}

/// Bots a match script asked the session for. A script cannot create a client
/// (`addtestclient` is refused: clients join through the session), so a
/// request waits here until the host's bot roster mints the clients.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptBotRequest {
    pub count: u32,
    pub side: ScriptBotSide,
    pub archetype: Arc<str>,
    /// The archetype's aim skill from the waves table; 1 is a default bot.
    pub skill: f32,
}

/// The script's side of bots it asked for: requests not yet drained by the
/// roster, and the archetype each minted client was bound to before it
/// joined. Kept apart from the script runtime because a `map_restart` keeps
/// the clients connected.
#[derive(Resource, Clone, Debug, Default)]
pub(crate) struct ScriptBots {
    requests: Vec<ScriptBotRequest>,
    archetypes: BTreeMap<ClientId, Arc<str>>,
}

impl ScriptBots {
    pub(crate) fn request(&mut self, request: ScriptBotRequest) {
        self.requests.push(request);
    }

    pub(crate) fn take_requests(&mut self) -> Vec<ScriptBotRequest> {
        std::mem::take(&mut self.requests)
    }

    pub(crate) fn has_requests(&self) -> bool {
        !self.requests.is_empty()
    }

    pub(crate) fn bind(&mut self, client: ClientId, archetype: Arc<str>) {
        self.archetypes.insert(client, archetype);
    }

    pub(crate) fn archetype(&self, client: ClientId) -> Option<&Arc<str>> {
        self.archetypes.get(&client)
    }

    pub(crate) fn release(&mut self, client: ClientId) {
        self.archetypes.remove(&client);
    }
}
