// BSD 3-Clause License
//
// Copyright (c) 2022, Lucina
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

use pyo3::exceptions::PyNotImplementedError;
use pyo3::types::{PyInt, PyType};
use pyo3::{PyAny, PyObject, PyResult, Python, ToPyObject};
use pyo3_anyio::tokio::fut_into_coro;
use tokio::sync::RwLock;
use twilight_gateway::cluster::Cluster;
use twilight_model::gateway::payload::outgoing::request_guild_members::{
    RequestGuildMemberId, RequestGuildMembersInfo,
};
use twilight_model::gateway::payload::outgoing::{RequestGuildMembers, UpdatePresence, UpdateVoiceState};
use twilight_model::gateway::{Intents, OpCode};
use twilight_model::id::marker::GuildMarker;
use twilight_model::id::Id;

pyo3::import_exception!(hikari, ComponentStateConflictError);


fn _flatten_undefined<'a>(undefined: &PyAny, value: Option<&'a PyAny>) -> Option<&'a PyAny> {
    value.and_then(|value| if value.is(undefined) { None } else { Some(value) })
}

#[pyo3::pyclass]
pub struct Shard {
    cluster: Arc<RwLock<Option<Arc<Cluster>>>>,
    intents: u64,
    shard_count: u64,
    shard_id: u64,
}

impl Shard {
    pub fn new(cluster: Arc<RwLock<Option<Arc<Cluster>>>>, shard_count: u64, shard_id: u64, intents: Intents) -> Self {
        Self {
            cluster,
            intents: intents.bits(),
            shard_count,
            shard_id,
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

    fn get_user_id<'p>(&self, py: Python<'p>) -> PyResult<&'p PyAny> {
        PyNotImplementedError::new_err("Not implemented"),
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
        status: Option<PyObject>,
        idle_since: Option<Option<PyObject>>,
        activity: Option<Option<PyObject>>,
        afk: Option<PyObject>,
    ) -> PyResult<&'p PyAny> {
        unimplemented!()
    }

    #[args(guild, channel, "*", self_mute = "None", self_deaf = "None")]
    pub fn update_voice_state<'p>(
        &self,
        py: Python<'p>,
        guild: PyObject,
        channel: Option<PyObject>,
        self_mute: Option<PyObject>,
        self_deaf: Option<PyObject>,
    ) -> PyResult<&'p PyAny> {
        unimplemented!()
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
        guild: PyObject,
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

        let message = RequestGuildMembers {
            op: OpCode::RequestGuildMembers,
            d: RequestGuildMembersInfo {
                guild_id: py_int.call1((guild,))?.extract::<u64>().map(Id::new)?,
                presences: _flatten_undefined(undefined, include_presences)
                    .map(|p| p.extract::<bool>())
                    .transpose()?,
                query: Some(query),
                limit: Some(limit),
                user_ids: _flatten_undefined(undefined, users)
                    .map(|ids| ids.iter())
                    .transpose()?
                    .map(|iterator| {
                        iterator.map(|v| {
                            v.and_then(|v_| py_int.call1((v_,)))
                                .and_then(PyAny::extract::<u64>)
                                .map(Id::new)
                        })
                    })
                    .map(|iterator| iterator.collect::<PyResult<Vec<_>>>())
                    .transpose()?
                    .map(RequestGuildMemberId::Multiple),
                nonce: nonce.map(|n| n.extract::<String>()).transpose()?,
            },
        };

        fut_into_coro(py, async move {
            cluster
                .read()
                .await
                .as_ref()
                .ok_or_else(|| pyo3::PyErr::new::<ComponentStateConflictError, _>(("Bot isn't running",)))?
                .command(shard_id, &message)
                .await
                .unwrap(); // TODO: error handling
            Ok(())
        })
    }
}
