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
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use log::{as_error, debug, warn};
use pyo3::exceptions::PyValueError;
use pyo3::types::IntoPyDict;
use pyo3::{pyclass, pymethods, Py, PyAny, PyErr, PyObject, PyResult, Python, ToPyObject};
use pyo3_asyncio::tokio::{future_into_py, local_future_into_py};
use serde_json::Value;
use tokio::sync::RwLock;
use twilight_gateway::cluster::{Cluster, ShardScheme};
use twilight_model::gateway::event::Event;
use twilight_model::gateway::payload::outgoing::identify::IdentifyProperties;
use twilight_model::gateway::Intents;

use crate::shard::Shard;

pyo3::import_exception!(hikari, ComponentStateConflictError);


struct _FinishedFuture {}

impl _FinishedFuture {
    fn new() -> Self {
        Self {}
    }
}

impl std::future::Future for _FinishedFuture {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        std::task::Poll::Ready(())
    }
}


#[pyclass(unsendable)]
struct _BotRefs {
    entity_factory: Option<PyObject>,
    event_factory: Option<PyObject>,
    event_manager: Option<PyObject>,
    rest: Option<PyObject>,
}

#[pymethods]
impl _BotRefs {
    #[getter]
    fn get_entity_factory(&self, py: Python) -> PyObject {
        self.entity_factory.as_ref().unwrap().clone_ref(py)
    }

    #[getter]
    fn get_executor(&self, py: Python) -> PyObject {
        py.None()
    }

    #[getter]
    fn get_http_settings(&self, py: Python) -> PyResult<PyObject> {
        self.rest.as_ref().unwrap().getattr(py, "http_settings")
    }

    #[getter]
    fn get_proxy_settings(&self, py: Python) -> PyResult<PyObject> {
        self.rest.as_ref().unwrap().getattr(py, "proxy_settings")
    }

    #[getter]
    fn get_rest(&self, py: Python) -> PyObject {
        self.rest.as_ref().unwrap().clone_ref(py)
    }
}

#[pyclass(unsendable)]
pub struct Bot {
    cluster: Arc<RwLock<Option<Arc<Cluster>>>>,
    gateway_url: String,
    intents: Intents,
    intents_py: PyObject,
    notify: Arc<tokio::sync::Notify>,
    refs: Py<_BotRefs>,
    shard_config: Option<ShardScheme>,
    shards: Arc<RwLock<HashMap<u64, Py<Shard>>>>,
    token: String,
}

impl Bot {
    fn get_shard<'p>(&self, py: Python<'p>, guild: &PyObject) -> PyResult<Py<crate::shard::Shard>> {
        if !self.get_is_alive() {
            return Err(PyErr::new::<ComponentStateConflictError, _>(("Bot isn't running",)));
        }

        let guild = guild.extract::<u64>(py)?;
        let shard_config = self.shard_config.as_ref().unwrap();
        let shard_id = (guild >> 22) % shard_config.total();

        self.shards
            .try_read()
            .map_err(|_| PyErr::new::<ComponentStateConflictError, _>(("Bot isn't running",)))?
            .get(&shard_id)
            .map(|value| value.clone_ref(py))
            .ok_or_else(|| PyValueError::new_err(("Shard not found",)))
    }
}


#[pymethods]
impl Bot {
    #[getter]
    fn get_entity_factory(&self, py: Python) -> PyObject {
        self.refs.borrow(py).get_entity_factory(py)
    }

    #[getter]
    fn get_event_factory(&self, py: Python) -> PyObject {
        self.refs.borrow(py).event_factory.as_ref().unwrap().clone_ref(py)
    }

    #[getter]
    fn get_event_manager(&self, py: Python) -> PyObject {
        self.refs.borrow(py).event_manager.as_ref().unwrap().clone_ref(py)
    }

    #[getter]
    fn get_executor(&self, py: Python) -> PyObject {
        py.None()
    }

    #[getter]
    fn get_heartbeat_latencies(&self) -> HashMap<u64, f64> {
        let cluster = match self.cluster.try_read() {
            Ok(cluster) if cluster.is_some() => cluster,
            _ => return HashMap::new(),
        };

        cluster
            .as_ref()
            .unwrap()
            .shards()
            .map(|shard| {
                let latency = shard
                    .info()
                    .ok()
                    .and_then(|info| info.latency().recent().back().map(Duration::as_secs_f64))
                    .unwrap_or(f64::NAN);

                (shard.config().shard()[0], latency)
            })
            .collect()
    }

    #[getter]
    fn get_heartbeat_latency(&self) -> f64 {
        let latencies = self
            .cluster
            .try_read()
            .ok()
            .and_then(|read| {
                read.as_ref().map(|cluster| {
                    cluster
                        .shards()
                        .filter_map(|shard| shard.info().ok())
                        .filter_map(|info| info.latency().recent().back().map(Duration::as_secs_f64))
                        .collect::<Vec<f64>>()
                })
            })
            .unwrap_or_default();


        let len = latencies.len();
        if len == 0 {
            f64::NAN
        } else {
            latencies.into_iter().sum::<f64>() / len as f64
        }
    }

    #[getter]
    fn get_http_settings(&self, py: Python) -> PyResult<PyObject> {
        self.refs.borrow(py).get_http_settings(py)
    }

    #[getter]
    fn get_is_alive(&self) -> bool {
        self.cluster
            .try_read()
            .map(|cluster| cluster.is_some())
            .unwrap_or(false)
    }

    #[getter]
    fn get_intents(&self, py: Python) -> PyObject {
        self.intents_py.clone_ref(py)
    }

    #[getter]
    fn get_proxy_settings(&self, py: Python) -> PyResult<PyObject> {
        self.refs.borrow(py).get_proxy_settings(py)
    }

    #[getter]
    fn get_rest(&self, py: Python) -> PyObject {
        self.refs.borrow(py).get_rest(py)
    }

    #[getter]
    fn get_shard_count(&self) -> u64 {
        self.cluster
            .try_read()
            .ok()
            .and_then(|cluster| {
                cluster
                    .as_ref()
                    .and_then(|cluster| cluster.shards().next().map(|shard| shard.config().shard()[1]))
            })
            .unwrap_or(0)
    }

    #[getter]
    fn get_shards(&self, py: Python) -> HashMap<u64, Py<Shard>> {
        self.shards.try_read().map(|map| map.clone()).unwrap_or_default()
    }

    #[new]
    #[args(
        token,
        "/",
        "*",
        http_settings = "None",
        intents = "None",
        proxy_settings = "None",
        max_rate_limit = "300.0",
        max_retries = "3",
        shards = "None",
        rest_url = "None",
        gateway_url = "\"wss://gateway.discord.gg\""
    )]
    fn new(
        py: Python,
        token: String,
        http_settings: Option<PyObject>,
        intents: Option<u64>,
        proxy_settings: Option<PyObject>,
        max_rate_limit: f64,
        max_retries: i64,
        shards: Option<(u64, u64, u64)>,
        rest_url: Option<&str>,
        gateway_url: &str,
    ) -> PyResult<Self> {
        let intents = crate::to_intents(intents)?;

        let config = py.import("hikari.impl.config")?;
        let http_settings = http_settings
            .map::<PyResult<PyObject>, _>(Ok)
            .unwrap_or_else(|| Ok(config.call_method0("HTTPSettings")?.to_object(py)))?;
        let proxy_settings = proxy_settings
            .map::<PyResult<PyObject>, _>(Ok)
            .unwrap_or_else(|| Ok(config.call_method0("ProxySettings")?.to_object(py)))?;

        let refs = Py::new(py, _BotRefs {
            entity_factory: None,
            event_factory: None,
            event_manager: None,
            rest: None,
        })?;

        let entity_factory = py
            .import("hikari.impl.entity_factory")?
            .call_method1("EntityFactoryImpl", (refs.clone_ref(py),))?
            .to_object(py);

        let event_factory = py
            .import("hikari.impl.event_factory")?
            .call_method1("EventFactoryImpl", (refs.clone_ref(py),))?
            .to_object(py);

        let event_manager = py
            .import("hikari.impl.event_manager")?
            .call_method1(
                "EventManagerImpl",
                (
                    entity_factory.clone_ref(py),
                    event_factory.clone_ref(py),
                    intents.bits().to_object(py),
                ),
            )?
            .to_object(py);

        let rest_kwargs = [
            ("cache", py.None()),
            ("entity_factory", entity_factory.clone_ref(py)),
            ("executor", py.None()),
            ("http_settings", http_settings.clone_ref(py)),
            ("max_rate_limit", max_rate_limit.to_object(py)),
            ("max_retries", max_retries.to_object(py)),
            ("proxy_settings", proxy_settings),
            ("token", token.to_object(py)),
            ("token_type", "Bot".to_object(py)),
            ("rest_url", rest_url.to_object(py)),
        ]
        .into_py_dict(py);
        let rest = py
            .import("hikari.impl.rest")?
            .getattr("RESTClientImpl")?
            .call((), Some(rest_kwargs))?
            .to_object(py);

        let mut refs_mut = refs.borrow_mut(py);
        refs_mut.entity_factory = Some(entity_factory);
        refs_mut.event_factory = Some(event_factory);
        refs_mut.event_manager = Some(event_manager);
        refs_mut.rest = Some(rest);
        drop(refs_mut);

        let intents_py = py
            .import("hikari.intents")?
            .call_method1("Intents", (intents.bits().to_object(py),))?
            .to_object(py);


        Ok(Self {
            cluster: Arc::new(RwLock::new(None)),
            intents,
            intents_py,
            notify: Arc::new(tokio::sync::Notify::new()),
            refs,
            shards: Arc::new(RwLock::new(HashMap::new())),
            shard_config: shards.map(|(from, to, total)| ShardScheme::Range { from, to, total }),
            token,
            gateway_url: gateway_url.to_owned(),
        })
    }

    fn close<'p>(&mut self, py: Python<'p>) -> PyResult<&'p PyAny> {
        if !self.get_is_alive() {
            return Err(PyErr::new::<ComponentStateConflictError, _>(("Bot isn't running",)));
        }

        let cluster = self.cluster.clone();
        let notify = self.notify.clone();
        future_into_py(py, async move {
            let mut cluster = cluster.write().await;
            cluster
                .as_ref()
                .ok_or_else(|| PyErr::new::<ComponentStateConflictError, _>(("Bot isn't running",)))?
                .down();

            *cluster = None;
            notify.notify_waiters();
            Ok(())
        })
    }

    fn join<'p>(&self, py: Python<'p>) -> PyResult<&'p PyAny> {
        let notify = self.notify.clone();
        future_into_py(py, async move {
            notify.notified().await;
            Ok(())
        })
    }

    fn run(&mut self, py: Python) -> PyResult<()> {
        if self.get_is_alive() {
            return Err(PyErr::new::<ComponentStateConflictError, _>(
                ("Bot is already running",),
            ));
        }

        let run_until_complete = py
            .import("asyncio")?
            .call_method0("new_event_loop")?
            .getattr("run_until_complete")?;
        run_until_complete.call1((self.start(py)?,))?;
        run_until_complete.call1((self.join(py)?,))?;
        Ok(())
    }

    fn start<'p>(&mut self, py: Python<'p>) -> PyResult<&'p PyAny> {
        if self.get_is_alive() {
            return Err(PyErr::new::<ComponentStateConflictError, _>(
                ("Bot is already running",),
            ));
        }

        let cluster_arc = self.cluster.clone();
        let consume_raw_event = self.get_event_manager(py).getattr(py, "consume_raw_event")?;
        let gateway_url = self.gateway_url.clone();
        let intents = self.intents;
        let notify = self.notify.clone();
        // TODO: error handling
        let shard_config = self
            .shard_config
            .get_or_insert_with(|| crate::fetch_shards_info(&self.token).unwrap())
            .clone();
        let shards = self.shards.clone();
        let token = self.token.clone();
        let shards_ = shard_config
            .iter()
            .map(|id| {
                let shard = Py::new(py, Shard::new(cluster_arc.clone(), shard_config.total(), id, intents))?;
                Ok((id, shard))
            })
            .collect::<PyResult<HashMap<u64, Py<Shard>>>>()?;

        future_into_py(py, async move {
            *shards.write().await = shards_;

            // TODO: handle error
            let (cluster, events) = Cluster::builder(token, intents)
                .event_types(twilight_gateway::EventTypeFlags::SHARD_PAYLOAD)
                .identify_properties(IdentifyProperties::new("rukari", "rukari", std::env::consts::OS))
                .shard_scheme(shard_config)
                .gateway_url(gateway_url)
                .build()
                .await
                .unwrap();

            let cluster = Arc::new(cluster);
            *cluster_arc.write().await = Some(cluster.clone());

            let handle_event = make_event_handler(shards.clone(), consume_raw_event).await?;
            tokio::spawn(async move {
                events.for_each_concurrent(None, handle_event).await;
                notify.notify_waiters();
                *cluster_arc.write().await = None;
                shards.write().await.clear();
            });

            cluster.up().await;
            Ok(())
        })
    }

    fn get_me(&self, py: Python) -> PyObject {
        py.None()
    }

    #[args("*", status = "None", idle_since = "None", activity = "None", afk = "None")]
    fn update_presence<'p>(
        &self,
        py: Python<'p>,
        status: Option<PyObject>,
        idle_since: Option<Option<PyObject>>,
        activity: Option<Option<PyObject>>,
        afk: Option<PyObject>,
    ) -> PyResult<&'p PyAny> {
        let shards = self.shards.as_ref();

        local_future_into_py(py, async move {
            // let coros = shards
            //     .try_read()
            //     .map_err(|err| PyErr::new::<ComponentStateConflictError, _>(("Bot isn't
            // running",)))?     .values()
            //     .map(|shard| shard.call_method1(py, "update_presence", (&status,
            // &idle_since, &activity, &afk)))     .filter_map(Result::ok)
            //     .collect::<Vec<_>>();

            // py.import("asyncio")?
            //     .call_method1("gather", pyo3::types::PyTuple::new(py, coros));
            Ok(())
        })
    }

    #[args(guild, channel, "*", self_mute = "None", self_deaf = "None")]
    fn update_voice_state<'p>(
        &self,
        py: Python<'p>,
        guild: PyObject,
        channel: Option<PyObject>,
        self_mute: Option<PyObject>,
        self_deaf: Option<PyObject>,
    ) -> PyResult<&'p PyAny> {
        self.get_shard(py, &guild)?
            .borrow(py)
            .update_voice_state(py, guild, channel, self_mute, self_deaf)
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
    fn request_guild_members<'p>(
        &self,
        py: Python<'p>,
        guild: PyObject,
        include_presences: Option<&PyAny>,
        query: String,
        limit: u64,
        users: Option<&PyAny>,
        nonce: Option<&PyAny>,
    ) -> PyResult<&'p PyAny> {
        self.get_shard(py, &guild)?.borrow(py).request_guild_members(
            py,
            guild,
            include_presences,
            query,
            limit,
            users,
            nonce,
        )
    }
}

async fn make_event_handler(
    shards: Arc<RwLock<HashMap<u64, Py<Shard>>>>,
    consume_raw_event: PyObject,
) -> PyResult<impl FnMut((u64, Event)) -> _FinishedFuture> {
    let call_soon_threadsafe = Python::with_gil(|py| {
        pyo3_asyncio::get_running_loop(py)?
            .getattr("call_soon_threadsafe")
            .map(|value| value.to_object(py))
    })?;
    let shards_read = shards.read().await.clone();

    Ok(move |item| {
        let (shard_id, payload) = match item {
            (shard_id, Event::ShardPayload(payload)) => (shard_id, payload),
            _ => unimplemented!(),
        };

        let shard = shards_read.get(&shard_id).unwrap();
        let parsed = match serde_json::from_slice::<Value>(&payload.bytes) {
            Ok(data) => data,
            Err(err) => {
                warn!(err = as_error!(err); "Failed to deserialize JSON");
                return _FinishedFuture::new();
            }
        };

        let (data, name) = match (parsed.get("d"), parsed.get("t").map(Value::as_str)) {
            (Some(data), Some(Some(name))) => (data, name),
            _ => {
                debug!("Failed to parse event data; this is likely a control event like heartbeat ACK");
                return _FinishedFuture::new();
            }
        };
        Python::with_gil(|py| {
            let data = match pythonize::pythonize(py, &data) {
                Ok(data) => data,
                Err(err) => {
                    warn!(err = as_error!(err); "Failed to deserialize JSON");
                    return;
                }
            };

            // TODO: error handling
            if let Err(err) = call_soon_threadsafe.call1(py, (&consume_raw_event, name, shard.clone_ref(py), data)) {
                warn!(err = as_error!(err); "Failed to call call_soon_threadsafe");
            }
        });

        _FinishedFuture::new()
    })
}
