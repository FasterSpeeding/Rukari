// BSD 3-Clause License
//
// Copyright (c) 2022-2025, Lucina
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// * Redistributions of source code must retain the above copyright notice, this
//   list of conditions and the following disclaimer.
//
// * Redistributions in binary form must reproduce the above copyright notice,
//   this list of conditions and the following disclaimer in the documentation
//   and/or other materials provided with the distribution.
//
// * Neither the name of the copyright holder nor the names of its contributors
//   may be used to endorse or promote products derived from this software
//   without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
use std::sync::Arc;
use std::time::Duration;

use pyo3::exceptions::{PyNotImplementedError, PyValueError};
use pyo3::types::{PyInt, PyType};
use pyo3::{PyAny, PyErr, PyResult, Python, ToPyObject};
use pyo3_anyio::tokio::fut_into_coro;
use tokio::sync::RwLock;
use twilight_gateway::cluster::Cluster;
use twilight_model::gateway::payload::outgoing::request_guild_members::{
    RequestGuildMemberId, RequestGuildMembersInfo,
};
use twilight_model::gateway::payload::outgoing::update_presence::UpdatePresencePayload;
use twilight_model::gateway::payload::outgoing::{RequestGuildMembers, UpdatePresence, UpdateVoiceState};
use twilight_model::gateway::presence::{Activity, ActivityType, Status};
use twilight_model::gateway::{Intents, OpCode};
use twilight_model::id::marker::{ChannelMarker, GuildMarker};
use twilight_model::id::Id;

pyo3::import_exception!(hikari, ComponentStateConflictError);


fn _flatten_undefined<'a>(undefined: &PyAny, value: Option<&'a PyAny>) -> Option<&'a PyAny> {
    value.and_then(|value| if value.is(undefined) { None } else { Some(value) })
}

#[derive(Clone)]
pub struct ShardState {
    pub self_mute: bool,
    pub self_deaf: bool,
    pub activity: Option<Activity>,
    pub afk: bool,
    pub afk_since: Option<u64>,
    pub status: Status,
}

impl ShardState {
    pub fn new() -> Self {
        ShardState {
            self_mute: false,
            self_deaf: false,
            activity: None,
            afk: false,
            afk_since: None,
            status: Status::Online,
        }
    }

    fn to_presence_payload(&self) -> UpdatePresencePayload {
        UpdatePresencePayload {
            activities: self.activity.as_ref().map(|v| vec![v.clone()]).unwrap_or_default(),
            afk: self.afk,
            since: self.afk_since,
            status: self.status,
        }
    }

    fn to_voice_payload(&self, guild_id: Id<GuildMarker>, channel_id: Option<Id<ChannelMarker>>) -> UpdateVoiceState {
        UpdateVoiceState::new(guild_id, channel_id, self.self_deaf, self.self_mute)
    }

    fn set_activity(&mut self, kind: u8, name: String, url: Option<String>) -> PyResult<()> {
        self.activity = Some(Activity {
            application_id: None,
            assets: None,
            buttons: Vec::new(),
            created_at: None,
            details: None,
            emoji: None,
            flags: None,
            id: None,
            instance: None,
            kind: ActivityType::from(kind),
            name,
            party: None,
            secrets: None,
            state: None,
            timestamps: None,
            url,
        });
        Ok(())
    }
}

impl Default for ShardState {
    fn default() -> Self {
        Self::new()
    }
}


#[pyo3::pyclass]
pub struct Shard {
    cluster: Arc<RwLock<Option<Arc<Cluster>>>>,
    intents: u64,
    shard_count: u64,
    shard_id: u64,
    state: Arc<RwLock<ShardState>>,
}

impl Shard {
    pub fn new(
        cluster: Arc<RwLock<Option<Arc<Cluster>>>>,
        state: ShardState,
        shard_count: u64,
        shard_id: u64,
        intents: Intents,
    ) -> Self {
        Self {
            cluster,
            intents: intents.bits(),
            shard_count,
            shard_id,
            state: Arc::new(RwLock::new(state)),
        }
    }
}

#[pyo3::pymethods]
impl Shard {
    #[getter]
    fn get_heartbeat_latency(&self) -> f64 {
        let cluster = match self.cluster.try_read() {
            Ok(cluster) if cluster.is_some() => cluster,
            _ => return f64::NAN,
        };

        cluster
            .as_ref()
            .unwrap()
            .shard(self.shard_id)
            .and_then(|shard| {
                shard
                    .info()
                    .ok()
                    .and_then(|info| info.latency().recent().back().map(Duration::as_secs_f64))
            })
            .unwrap_or(f64::NAN)
    }

    #[getter]
    fn get_id(&self) -> u64 {
        self.shard_id
    }

    #[getter]
    fn get_intents<'p>(&self, py: Python<'p>) -> PyResult<&'p PyAny> {
        py.import("hikari")?
            .call_method1("Intents", (self.intents.to_object(py),))
    }

    #[getter]
    fn is_alive(&self) -> bool {
        true
    }

    #[getter]
    fn get_shard_count(&self) -> u64 {
        self.shard_count
    }

    fn get_user_id(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Not implemented"))
    }

    fn close(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err(
            "Cannot directly close this shard implementation",
        ))
    }

    fn join(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err(
            "Cannot directly join this shard implementation",
        ))
    }

    fn start(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err(
            "Cannot directly start this shard implementation",
        ))
    }

    #[args("*", status = "None", idle_since = "None", activity = "None", afk = "None")]
    pub fn update_presence<'p>(
        &self,
        py: Python<'p>,
        status: Option<&'p PyAny>,
        idle_since: Option<&'p PyAny>,
        activity: Option<&'p PyAny>,
        afk: Option<&'p PyAny>,
    ) -> PyResult<&'p PyAny> {
        let cluster = self.cluster.clone();
        let state = self.state.clone();
        let shard_id = self.shard_id;
        let undefined = py.import("hikari")?.getattr("UNDEFINED")?;
        let afk = _flatten_undefined(undefined, afk)
            .map(|v| v.extract::<bool>())
            .transpose()?;
        let activity = _flatten_undefined(undefined, activity)
            .map(|v| {
                Ok::<_, PyErr>((
                    v.getattr("type")?.extract::<u8>()?,
                    v.getattr("name")?.extract::<String>()?,
                    v.getattr("url")?.extract::<Option<String>>()?,
                ))
            })
            .transpose()?;

        let since = _flatten_undefined(undefined, idle_since)
            .map(|v| v.call_method0("timestamp"))
            .transpose()?
            .map(|v| v.extract::<f64>())
            .transpose()?
            .map(|v| (v * 1_000.) as u64);

        let status = _flatten_undefined(undefined, status)
            .map(|v| match v.extract::<String>().as_ref().map(String::as_str) {
                Ok("online") => Ok(Status::Online),
                Ok("idle") => Ok(Status::Idle),
                Ok("dnd") => Ok(Status::DoNotDisturb),
                Ok("offline") => Ok(Status::Offline),
                Ok(_) => Err(PyValueError::new_err("Invalid status")),
                Err(err) => Err(err.clone_ref(py)),
            })
            .transpose()?;

        fut_into_coro(py, async move {
            let mut state_write = state.write().await;

            if let Some(afk) = afk {
                state_write.afk = afk;
            }

            if let Some(since) = since {
                state_write.afk_since = Some(since);
            }

            if let Some(status) = status {
                state_write.status = status;
            }

            if let Some((kind, name, url)) = activity {
                state_write.set_activity(kind, name, url)?;
            }

            let message = UpdatePresence {
                d: state_write.to_presence_payload(),
                op: OpCode::PresenceUpdate,
            };
            drop(state_write);

            send_event(cluster, shard_id, message).await
        })
    }

    #[args(guild, channel, "*", self_mute = "None", self_deaf = "None")]
    pub fn update_voice_state<'p>(
        &self,
        py: Python<'p>,
        guild: &'p PyAny,
        channel: Option<&'p PyAny>,
        self_mute: Option<&'p PyAny>,
        self_deaf: Option<&'p PyAny>,
    ) -> PyResult<&'p PyAny> {
        let cluster = self.cluster.clone();
        let state = self.state.clone();
        let shard_id = self.shard_id;
        let undefined = py.import("hikari")?.getattr("UNDEFINED")?;
        let py_int = PyType::new::<PyInt>(py);

        let guild = to_id(py_int, guild)?;
        let channel = channel.map(|v| to_id(py_int, v)).transpose()?;

        let self_deaf = _flatten_undefined(undefined, self_deaf)
            .map(|p| p.extract::<bool>())
            .transpose()?;

        let self_mute = _flatten_undefined(undefined, self_mute)
            .map(|p| p.extract::<bool>())
            .transpose()?;

        fut_into_coro(py, async move {
            let mut state_write = state.write().await;

            if let Some(self_mute) = self_mute {
                state_write.self_mute = self_mute;
            }

            if let Some(self_deaf) = self_deaf {
                state_write.self_deaf = self_deaf;
            }

            let message = state_write.to_voice_payload(guild, channel);
            drop(state_write);

            send_event(cluster, shard_id, message).await
        })
    }

    #[args(
        guild,
        "*",
        include_presences = "None",
        query = "\"\".to_owned()",
        limit = "0",
        users = "None",
        nonce = "None"
    )]
    pub fn request_guild_members<'p>(
        &self,
        py: Python<'p>,
        guild: &PyAny,
        include_presences: Option<&PyAny>,
        query: String,
        limit: u64,
        users: Option<&PyAny>,
        nonce: Option<&PyAny>,
    ) -> PyResult<&'p PyAny> {
        let cluster = self.cluster.clone();
        let shard_id = self.shard_id;
        let undefined = py.import("hikari")?.getattr("UNDEFINED")?;
        let py_int = PyType::new::<PyInt>(py);

        let user_ids = _flatten_undefined(undefined, users)
            .map(|ids| ids.iter())
            .transpose()?
            .map(|iterator| iterator.map(|v| v.and_then(|v_| to_id(py_int, v_))))
            .map(|iterator| iterator.collect::<PyResult<Vec<_>>>())
            .transpose()?
            .map(RequestGuildMemberId::Multiple);

        let message = RequestGuildMembers {
            op: OpCode::RequestGuildMembers,
            d: RequestGuildMembersInfo {
                guild_id: to_id(py_int, guild)?,
                presences: _flatten_undefined(undefined, include_presences)
                    .map(|p| p.extract::<bool>())
                    .transpose()?,
                query: Some(query),
                limit: Some(limit),
                user_ids,
                nonce: nonce.map(|n| n.extract::<String>()).transpose()?,
            },
        };

        fut_into_coro(py, async move { send_event(cluster, shard_id, message).await })
    }
}


fn to_id<T>(py_int: &PyType, value: &PyAny) -> PyResult<Id<T>> {
    py_int.call1((value,)).and_then(PyAny::extract::<u64>).map(Id::new)
}


async fn send_event(
    cluster: Arc<RwLock<Option<Arc<Cluster>>>>,
    shard_id: u64,
    message: impl twilight_gateway::shard::Command,
) -> PyResult<()> {
    cluster
        .read()
        .await
        .as_ref()
        .ok_or_else(|| pyo3::PyErr::new::<ComponentStateConflictError, _>(("Bot isn't running",)))?
        .command(shard_id, &message)
        .await
        .unwrap(); // TODO: error handling
    Ok(())
}
