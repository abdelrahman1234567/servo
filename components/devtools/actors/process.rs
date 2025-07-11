/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Liberally derived from the [Firefox JS implementation].
//!
//! [Firefox JS implementation]: https://searchfox.org/mozilla-central/source/devtools/server/actors/descriptors/process.js

use serde::Serialize;
use serde_json::{Map, Value};

use crate::StreamId;
use crate::actor::{Actor, ActorError, ActorRegistry};
use crate::actors::root::DescriptorTraits;
use crate::protocol::ClientRequest;

#[derive(Serialize)]
struct ListWorkersReply {
    from: String,
    workers: Vec<u32>, // TODO: use proper JSON structure.
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessActorMsg {
    actor: String,
    id: u32,
    is_parent: bool,
    is_windowless_parent: bool,
    traits: DescriptorTraits,
}

pub struct ProcessActor {
    name: String,
}

impl Actor for ProcessActor {
    fn name(&self) -> String {
        self.name.clone()
    }

    /// The process actor can handle the following messages:
    ///
    /// - `listWorkers`: Returns a list of web workers, not supported yet.
    fn handle_message(
        &self,
        request: ClientRequest,
        _registry: &ActorRegistry,
        msg_type: &str,
        _msg: &Map<String, Value>,
        _id: StreamId,
    ) -> Result<(), ActorError> {
        match msg_type {
            "listWorkers" => {
                let reply = ListWorkersReply {
                    from: self.name(),
                    workers: vec![],
                };
                request.reply_final(&reply)?
            },

            _ => return Err(ActorError::UnrecognizedPacketType),
        };
        Ok(())
    }
}

impl ProcessActor {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub fn encodable(&self) -> ProcessActorMsg {
        ProcessActorMsg {
            actor: self.name(),
            id: 0,
            is_parent: true,
            is_windowless_parent: false,
            traits: Default::default(),
        }
    }
}
