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
#![feature(never_type)]
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{StreamExt};
use log::{as_error, debug, warn};
use pyo3::conversion::{FromPyObject, ToPyObject};
use pyo3::exceptions::{PyNotImplementedError, PyValueError};
use pyo3::types::{IntoPyDict, PyType};
use pyo3::{pyclass, pymethods, Py, PyAny, PyErr, PyObject, PyResult, Python};
use pyo3_asyncio::tokio::future_into_py;
use pythonize::pythonize;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::{Notify, RwLock};
use twilight_gateway::cluster::{Cluster, ShardScheme};
use twilight_model::gateway::event::Event;
use twilight_model::gateway::payload::outgoing::identify::IdentifyProperties;
use twilight_model::gateway::payload::outgoing::{RequestGuildMembers, UpdatePresence, UpdateVoiceState};
use twilight_model::gateway::Intents;

pyo3::import_exception!(hikari, ComponentStateConflictError);


enum BotMessage {}


#[pyclass]
struct BotManager {
    intents: Intents,
    token: String,
}
// shard_count: Option<Vec<u64>>,
// shard_ids: Option<Vec<u64>>,

fn _to_intents(intents: Option<u64>) -> PyResult<Intents> {
    intents
        .map(Intents::from_bits)
        .unwrap_or_else(|| {
            Some(Intents::all() & !(Intents::GUILD_MEMBERS | Intents::GUILD_PRESENCES | Intents::MESSAGE_CONTENT))
        })
        .ok_or_else(|| PyValueError::new_err("Invalid intent(s) passed"))
}

fn _fetch_shards_info(token: &str) -> Result<ShardScheme, Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_current_thread().build()?.block_on(async {
        twilight_http::Client::new(token.to_string())
            .gateway()
            .authed()
            .exec()
            .await?
            .model()
            .await
            .map(|gateway| ShardScheme::Range {
                from: 0,
                to: gateway.shards - 1,
                total: gateway.shards,
            })
            .map_err(Box::from)
    })
}

#[pymethods]
impl BotManager {
    #[new]
    #[args(token, "/", "*", intents = "None")]
    fn new(token: String, intents: Option<u64>) -> PyResult<Self> {
        Ok(Self {
            intents: _to_intents(intents)?,
            token,
        })
    }

    #[args(self, "/")]
    fn start(&self) {
    }
}

#[pyclass]
struct Shard {
    cluster: Arc<RwLock<Option<Arc<Cluster>>>>,
    intents: u64,
    shard_count: u64,
    shard_id: u64,
}

impl Shard {
    fn new(cluster: Arc<RwLock<Option<Arc<Cluster>>>>, shard_count: u64, shard_id: u64, intents: Intents) -> Self {
        Self {
            cluster,
            intents: intents.bits(),
            shard_count,
            shard_id,
        }
    }
}

#[pymethods]
impl Shard {
    #[getter(heartbeat_latency)]
    fn get_heartbeat_latency(&self) -> f64 {
        let cluster = match self.cluster.try_read() {
            Ok(cluster) if cluster.is_some() => cluster.clone().unwrap(),
            _ => return f64::NAN,
        };

        cluster
            .shard(self.shard_id)
            .and_then(|shard| {
                shard
                    .info()
                    .ok()
                    .and_then(|info| info.latency().recent().back().map(Duration::as_secs_f64))
            })
            .unwrap_or(f64::NAN)
    }

    #[getter(id)]
    fn get_id(&self) -> u64 {
        self.shard_id
    }

    #[getter(intents)]
    fn get_intents<'p>(&self, py: Python<'p>) -> PyResult<&'p PyAny> {
        py.import("hikari")?
            .call_method1("Intents", (self.intents.to_object(py),))
    }

    #[getter(is_alive)]
    fn is_alive(&self) -> bool {
        true
    }

    #[getter(shard_count)]
    fn get_shard_count(&self) -> u64 {
        self.shard_count
    }

    fn get_get_user_id<'p>(&self, py: Python<'p>) -> PyResult<&'p PyAny> {
        future_into_py::<_, ()>(
            py,
            async move { Err(PyNotImplementedError::new_err("Not implemented")) },
        )
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
    fn update_presence<'p>(
        &'p self,
        py: Python,
        status: Option<PyObject>,
        idle_since: Option<Option<PyObject>>,
        activity: Option<Option<PyObject>>,
        afk: Option<PyObject>,
    ) -> PyResult<()> {
        Ok(())
    }

    #[args(guild, channel, "*", self_mute = "None", self_deaf = "None")]
    fn update_voice_state(
        &self,
        guild: PyObject,
        channel: Option<PyObject>,
        self_mute: Option<PyObject>,
        self_deaf: Option<PyObject>,
    ) -> PyResult<()> {
        Ok(())
    }

    #[args(
        guild,
        "*",
        include_presences = "None",
        query = "\"\"",
        limit = "0",
        users = "None",
        nonce = "None"
    )]
    fn request_guild_members(
        &self,
        guild: PyObject,
        include_presences: Option<PyObject>,
        query: &str,
        limit: u64,
        users: Option<PyObject>,
        nonce: Option<PyObject>,
    ) -> PyResult<()> {
        Ok(())
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
    #[getter(entity_factory)]
    fn get_entity_factory(&self, py: Python) -> PyObject {
        self.entity_factory.as_ref().unwrap().clone_ref(py)
    }

    #[getter(executor)]
    fn get_executor(&self, py: Python) -> PyObject {
        py.None()
    }

    #[getter(http_settings)]
    fn get_http_settings(&self, py: Python) -> PyResult<PyObject> {
        self.rest.as_ref().unwrap().getattr(py, "http_settings")
    }

    #[getter(proxy_settings)]
    fn get_proxy_settings(&self, py: Python) -> PyResult<PyObject> {
        self.rest.as_ref().unwrap().getattr(py, "proxy_settings")
    }

    #[getter(rest)]
    fn get_rest(&self, py: Python) -> PyObject {
        self.rest.as_ref().unwrap().clone_ref(py)
    }
}

#[pyclass(unsendable)]
struct Bot {
    cluster: Arc<RwLock<Option<Arc<Cluster>>>>,
    intents: Intents,
    intents_py: PyObject,
    notify: Arc<Notify>,
    refs: Py<_BotRefs>,
    shard_config: Option<ShardScheme>,
    shards: Arc<RwLock<HashMap<u64, Py<Shard>>>>,
    token: String,
}

impl Bot {
    // async fn send<'p>(
    //     &self,
    //     cluster: Arc<RwLock<Option<Arc<Cluster>>>>,
    //     id: u64,
    //     message: twilight_gateway::shard::raw_message::Message,
    // ) -> PyResult<()> {
    //     cluster
    //         .read()
    //         .await
    //         .ok_or_else(|| PyErr::new::<ComponentStateConflictError, _>(("Bot
    // isn't running",)))?         .send(id, message)
    //         .await
    // .map_err(|_| PyRuntimeError::new_err("Failed to send message") }
}


#[pymethods]
impl Bot {
    #[getter(entity_factory)]
    fn get_entity_factory(&self, py: Python) -> PyObject {
        self.refs.borrow(py).get_entity_factory(py)
    }

    #[getter(event_factory)]
    fn get_event_factory(&self, py: Python) -> PyObject {
        self.refs.borrow(py).event_factory.as_ref().unwrap().clone_ref(py)
    }

    #[getter(event_manager)]
    fn get_event_manager(&self, py: Python) -> PyObject {
        self.refs.borrow(py).event_manager.as_ref().unwrap().clone_ref(py)
    }

    #[getter(executor)]
    fn get_executor(&self, py: Python) -> PyObject {
        py.None()
    }

    #[getter(heartbeat_latencies)]
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

    #[getter(heartbeat_latency)]
    fn get_heartbeat_latency(&self) -> f64 {
        let cluster = match self.cluster.try_read() {
            Ok(cluster) if cluster.is_some() => cluster,
            _ => return f64::NAN,
        };

        let latencies = cluster
            .as_ref()
            .unwrap()
            .shards()
            .filter_map(|shard| shard.info().ok())
            .filter_map(|info| info.latency().recent().back().map(Duration::as_secs_f64))
            .collect::<Vec<f64>>();

        let len = latencies.len();
        if len == 0 {
            f64::NAN
        } else {
            latencies.into_iter().sum::<f64>() / len as f64
        }
    }

    #[getter(http_settings)]
    fn get_http_settings(&self, py: Python) -> PyResult<PyObject> {
        self.refs.borrow(py).get_http_settings(py)
    }

    #[getter(is_alive)]
    fn get_is_alive(&self) -> bool {
        self.cluster
            .try_read()
            .map(|cluster| cluster.is_some())
            .unwrap_or(false)
    }

    #[getter(intents)]
    fn get_intents(&self, py: Python) -> PyObject {
        self.intents_py.clone_ref(py)
    }

    #[getter(proxy_settings)]
    fn get_proxy_settings(&self, py: Python) -> PyResult<PyObject> {
        self.refs.borrow(py).get_proxy_settings(py)
    }

    #[getter(rest)]
    fn get_rest(&self, py: Python) -> PyObject {
        self.refs.borrow(py).get_rest(py)
    }

    #[getter(shard_count)]
    fn get_shard_count(&self) -> u64 {
        let cluster = match self.cluster.try_read() {
            Ok(cluster) if cluster.is_some() => cluster,
            _ => return 0,
        };

        cluster
            .as_ref()
            .unwrap()
            .shards()
            .next()
            .map(|shard| shard.config().shard()[1])
            .unwrap_or(0)
    }

    #[getter(shards)]
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
        shards = "None"
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
    ) -> PyResult<Self> {
        let intents = _to_intents(intents)?;

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
            ("rest_url", py.None()),
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
            notify: Arc::new(Notify::new()),
            refs,
            shards: Arc::new(RwLock::new(HashMap::new())),
            shard_config: shards.map(|(from, to, total)| ShardScheme::Range { from, to, total }),
            token,
        })
    }

    fn close<'p>(&mut self, py: Python<'p>) -> PyResult<&'p PyAny> {
        if self.get_is_alive() {
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
        let call_soon_threadsafe = pyo3_asyncio::get_running_loop(py)?
            .getattr("call_soon_threadsafe")?
            .to_object(py);

        let consume_raw_event = self.get_event_manager(py).getattr(py, "consume_raw_event")?;
        let intents = self.intents;
        let notify = self.notify.clone();
        // TODO: error handling
        let shard_config = self
            .shard_config
            .clone()
            .unwrap_or_else(|| _fetch_shards_info(&self.token).unwrap());
        let shard_count = shard_config.total();
        let shards = self.shards.clone();
        let mut shards_write: HashMap<u64, Py<Shard>> = HashMap::new();
        let token = self.token.clone();

        for shard_id in shard_config.iter() {
            let shard = Shard::new(cluster_arc.clone(), shard_count, shard_id, intents);
            shards_write.insert(shard_id, Py::new(py, shard)?);
        }

        future_into_py(py, async move {
            *shards.write().await = shards_write;
            let shards_read = shards.read().await.clone();

            let cluster = Cluster::builder(token, intents)
                .event_types(twilight_gateway::EventTypeFlags::SHARD_PAYLOAD)
                .identify_properties(IdentifyProperties::new("rukari", "rukari", std::env::consts::OS))
                .shard_scheme(shard_config);

            // TODO: handle error
            let (cluster, events) = cluster.build().await.unwrap();

            let cluster = Arc::new(cluster);
            *cluster_arc.write().await = Some(cluster.clone());

            tokio::spawn(async move {
                events
                    .map(Ok)
                    .forward(futures_util::sink::unfold((), move |(), item| {
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
                            let py_data = match pythonize(py, &data) {
                                Ok(py_data) => py_data,
                                Err(err) => {
                                    warn!(err = as_error!(err); "Failed to deserialize JSON");
                                    return;
                                }
                            };

                            // TODO: error handling
                            call_soon_threadsafe
                                .call1(py, (&consume_raw_event, name, shard.clone_ref(py), py_data))
                                .unwrap();
                        });

                        _FinishedFuture::new()
                    }))
                    .await
                    .unwrap(); // TODO: handle error
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
        &'p self,
        py: Python,
        status: Option<PyObject>,
        idle_since: Option<Option<PyObject>>,
        activity: Option<Option<PyObject>>,
        afk: Option<PyObject>,
    ) -> PyResult<()> {
        Ok(())
        // let cluster = self.cluster.clone();
        // let undefined = py.import("hikari")?.getattr("UNDEFINED")?;
        // future_into_py(
        //     py,
        //     self.send(
        //         &cluster,
        //         0,
        //         UpdatePresence::new(
        //             Vec::new(),
        //             _flatten_undefined(undefined, afk)
        //                 .map(bool::extract)
        //                 .unwrap_or_else(|| Ok(false))?,
        //             _flatten_undefined(undefined, idle_since).map(|since|
        // since),             _flatten_undefined(undefined, status),
        //         ),
        //     ),
        // )
    }

    #[args(guild, channel, "*", self_mute = "None", self_deaf = "None")]
    fn update_voice_state(
        &self,
        guild: PyObject,
        channel: Option<PyObject>,
        self_mute: Option<PyObject>,
        self_deaf: Option<PyObject>,
    ) -> PyResult<()> {
        Ok(())
        // let cluster = self.cluster.clone();
        // let undefined = py.import("hikari")?.getattr("UNDEFINED")?;
        // future_into_py(
        //     py,
        //     self.send(
        //         &cluster,
    }

    #[args(
        guild,
        "*",
        include_presences = "None",
        query = "\"\"",
        limit = "0",
        users = "None",
        nonce = "None"
    )]
    fn request_guild_members(
        &self,
        guild: PyObject,
        include_presences: Option<PyObject>,
        query: &str,
        limit: u64,
        users: Option<PyObject>,
        nonce: Option<PyObject>,
    ) -> PyResult<()> {
        Ok(())
        //     let cluster = self.cluster.clone();
        //     let undefined = py.import("hikari")?.getattr("UNDEFINED")?;
        //     future_into_py(
        //         py,
        //         self.send(
        //             &cluster,
        //             RequestGuildMembers
    }
}

fn _flatten_undefined(undefined: &PyAny, value: Option<PyObject>) -> Option<PyObject> {
    value.and_then(|value| if value.is(undefined) { None } else { Some(value) })
}

struct _FinishedFuture {}

impl _FinishedFuture {
    fn new() -> Self {
        Self {}
    }
}

impl std::future::Future for _FinishedFuture {
    type Output = Result<(), !>;

    fn poll(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        std::task::Poll::Ready(Ok(()))
    }
}


#[pyo3::pymodule]
fn rukari(python: Python<'_>, module: &pyo3::types::PyModule) -> PyResult<()> {
    let _ = pyo3_log::try_init();

    let bot_type = PyType::new::<Bot>(python);
    let hikari = python.import("hikari")?;
    let traits = hikari.getattr("traits")?;
    hikari
        .getattr("api")?
        .getattr("GatewayShard")?
        .call_method1("register", (PyType::new::<Shard>(python),))?;

    module.add("__author__", "Faster Speeding")?;
    module.add("__ci__", "https://github.com/FasterSpeeding/Rukari/actions")?;
    module.add("__copyright__", "© 2022 Faster Speeding")?;
    module.add("__coverage__", "https://codeclimate.com/github/FasterSpeeding/Rukari")?;
    module.add("__docs__", "https://rukari.cursed.solutions/")?;
    module.add("__email__", "lucina@lmbyrne.dev")?;
    module.add("__issue_tracker__", "https://github.com/FasterSpeeding/Rukari/issues")?;
    module.add("__license__", "BSD")?;
    module.add("__url__", "https://github.com/FasterSpeeding/Rukari")?;
    module.add("__version__", "0.1.0")?;
    module.add_class::<BotManager>()?;
    module.add_class::<Bot>()?;

    assert!(
        bot_type.is_subclass(traits.getattr("EventFactoryAware")?.cast_as::<PyType>()?)?,
        "Bot isn't EventFactoryAware"
    );
    assert!(
        bot_type.is_subclass(traits.getattr("EventManagerAware")?.cast_as::<PyType>()?)?,
        "Bot isn't EventManagerAware"
    );
    assert!(
        bot_type.is_subclass(traits.getattr("IntentsAware")?.cast_as::<PyType>()?)?,
        "Bot isn't IntentsAware"
    );
    assert!(
        bot_type.is_subclass(traits.getattr("RESTAware")?.cast_as::<PyType>()?)?,
        "Bot isn't RESTAware"
    );
    assert!(
        bot_type.is_subclass(traits.getattr("Runnable")?.cast_as::<PyType>()?)?,
        "Bot isn't Runnable"
    );
    // This would require being voice aware which probably isn't going to happen
    // for now.
    // assert!(
    //     bot_type.is_subclass(traits.getattr("ShardAware")?.cast_as::<PyType>()?)?
    // ,     "Bot isn't ShardAware"
    // );

    Ok(())
}
