// BSD 3-Clause License
//
// Copyright (c) 2022-2024, Lucina
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
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::{ready, Future, Ready};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use futures_util::StreamExt;
use log::{as_error, debug, warn};
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::types::{IntoPyDict, PyDict, PyTuple};
use pyo3::{pyclass, pymethods, IntoPy, Py, PyAny, PyErr, PyObject, PyResult, Python, ToPyObject};
use pyo3_anyio::tokio::{await_py1, fut_into_coro};
use serde_json::Value;
use tokio::sync::RwLock;
use twilight_gateway::cluster::{Cluster, ShardScheme};
use twilight_model::gateway::event::Event;
use twilight_model::gateway::payload::outgoing::identify::IdentifyProperties;
use twilight_model::gateway::Intents;

use crate::shard::{Shard, ShardState};

pyo3::import_exception!(hikari, ComponentStateConflictError);


static WRAP_GATHER: OnceLock<PyObject> = OnceLock::new();

fn gather(py: Python<'_>) -> PyResult<&PyAny> {
    WRAP_GATHER
        .get_or_try_init(|| {
            let globals = PyDict::new(py);
            py.run(
                r#"
import asyncio

def gather(*coros):
    await asyncio.gather(*coros)
            "#,
                Some(globals),
                None,
            )?;

            Ok::<_, PyErr>(globals.get_item("gather").unwrap().to_object(py))
        })
        .map(|value| value.as_ref(py))
}


#[pyclass(unsendable)]
struct _BotRefs {
    entity_factory: Option<PyObject>,
    event_factory: Option<PyObject>,
    event_manager: Option<PyObject>,
    rest: Option<PyObject>,
    shards: Arc<RwLock<HashMap<u64, Py<Shard>>>>,
    voice: Option<PyObject>,
}

#[pymethods]
impl _BotRefs {
    #[getter]
    fn get_entity_factory(&self, py: Python) -> PyObject {
        self.entity_factory.as_ref().unwrap().clone_ref(py)
    }

    #[getter]
    fn get_event_factory(&self, py: Python) -> PyObject {
        self.event_factory.as_ref().unwrap().clone_ref(py)
    }

    #[getter]
    fn get_event_manager(&self, py: Python) -> PyObject {
        self.event_manager.as_ref().unwrap().clone_ref(py)
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

    #[getter]
    fn get_shards(&self) -> HashMap<u64, Py<Shard>> {
        self.shards.try_read().map(|map| map.clone()).unwrap_or_default()
    }

    #[getter]
    fn get_voice(&self, py: Python) -> PyObject {
        self.voice.as_ref().unwrap().clone_ref(py)
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
    shard_config: RefCell<Option<ShardScheme>>,
    shards: Arc<RwLock<HashMap<u64, Py<Shard>>>>,
    token: String,
}

impl Bot {
    fn get_shard(&self, guild: &PyAny) -> PyResult<Py<crate::shard::Shard>> {
        if !self.get_is_alive() {
            return Err(PyErr::new::<ComponentStateConflictError, _>(("Bot isn't running",)));
        }

        let py = guild.py();
        let guild = guild.extract::<u64>()?;
        let shard_id = (guild >> 22) % self.shard_config.borrow().as_ref().unwrap().total();

        self.shards
            .try_read()
            .map_err(|_| PyErr::new::<ComponentStateConflictError, _>(("Bot isn't running",)))?
            .get(&shard_id)
            .map(|value| value.clone_ref(py))
            .ok_or_else(|| PyValueError::new_err(("Shard not found",)))
    }

    fn run_async(&self, py: Python) -> PyResult<impl Future<Output = PyResult<()>>> {
        if self.get_is_alive() {
            return Err(PyErr::new::<ComponentStateConflictError, _>(
                ("Bot is already running",),
            ));
        }

        let cluster_arc = self.cluster.clone();
        let event_manager = self.get_event_manager(py);
        let consume_raw_event = event_manager.getattr(py, "consume_raw_event")?;

        // the dispatch method only returns a gathering Future (not a
        // coroutine) and needs to be called in the current thread.
        let globals_ = [("callback", event_manager.getattr(py, "dispatch")?)].into_py_dict(py);
        py.run(
            "async def dispatch(event):\n  await callback(event)",
            Some(globals_),
            None,
        )
        .unwrap();
        let dispatch = globals_.get_item("dispatch").unwrap().to_object(py);

        let gateway_url = self.gateway_url.clone();
        let intents = self.intents;
        let notify = self.notify.clone();
        // TODO: error handling
        let shard_config = self
            .shard_config
            .borrow_mut()
            .get_or_insert_with(|| crate::fetch_shards_info(&self.token).unwrap())
            .clone();
        let shard_state = ShardState::new();
        let shards = self.shards.clone();
        let shards_ = shard_config
            .iter()
            .map(|id| {
                let shard = Py::new(
                    py,
                    Shard::new(
                        cluster_arc.clone(),
                        shard_state.clone(),
                        shard_config.total(),
                        id,
                        intents,
                    ),
                )?;
                Ok((id, shard))
            })
            .collect::<PyResult<HashMap<u64, Py<Shard>>>>()?;
        let token = self.token.clone();

        let event_factory = self.get_event_factory(py);
        let start_rest = self.get_rest(py).getattr(py, "start")?;
        let start_voice = self.get_voice(py).getattr(py, "start")?;
        let starting_event = event_factory.call_method0(py, "deserialize_starting_event")?;
        let started_event = event_factory.call_method0(py, "deserialize_started_event")?;
        let stopping_event = event_factory.call_method0(py, "deserialize_stopping_event")?;
        let stopped_event = event_factory.call_method0(py, "deserialize_stopping_event")?;

        Ok(async move {
            *shards.write().await = shards_;
            let task_locals = Python::with_gil(|py| pyo3_anyio::tokio::get_locals_py(py).unwrap());
            Python::with_gil(|py| call_in_loop(task_locals.clone_py(py), start_rest))?.await?;
            Python::with_gil(|py| call_in_loop(task_locals.clone_py(py), start_voice))?.await?;
            dispatch_lifetime(&dispatch, starting_event).await?;

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

            let handle_event = make_event_handler(task_locals.clone(), shards.clone(), consume_raw_event).await?;
            tokio::spawn(pyo3_anyio::tokio::scope(task_locals, async move {
                let _ = dispatch_lifetime(&dispatch, started_event).await.ok();

                events.for_each_concurrent(None, handle_event).await;
                notify.notify_waiters();
                *cluster_arc.write().await = None;
                shards.write().await.clear();

                let _ = dispatch_lifetime(&dispatch, stopping_event).await.ok();
                let _ = dispatch_lifetime(&dispatch, stopped_event).await.ok();
            }));

            cluster.up().await;
            Ok(())
        })
    }
}

#[pyclass]
struct CallInLoop {
    callback: PyObject,
    send: Option<tokio::sync::oneshot::Sender<PyResult<()>>>,
}

#[pymethods]
impl CallInLoop {
    fn __call__(&mut self, py: Python) {
        match self.callback.call0(py) {
            Ok(_) => self.send.take().unwrap().send(Ok(())),
            Err(err) => self.send.take().unwrap().send(Err(err)),
        }
        .unwrap();
    }
}

fn call_in_loop(
    locals: pyo3_anyio::any::TaskLocals,
    callback: PyObject,
) -> PyResult<impl Future<Output = PyResult<()>>> {
    let (send, recv) = tokio::sync::oneshot::channel::<PyResult<()>>();

    Python::with_gil(|py| {
        locals.call_soon0(
            CallInLoop {
                callback,
                send: Some(send),
            }
            .into_py(py)
            .as_ref(py),
        )
    })?;
    Ok(async move { recv.await.unwrap() })
}

async fn dispatch_lifetime(dispatch: &PyObject, event: PyObject) -> PyResult<PyObject> {
    Python::with_gil(|py| await_py1(dispatch.as_ref(py), &[event.as_ref(py)]))?.await
}


#[pymethods]
impl Bot {
    #[getter]
    fn get_entity_factory(&self, py: Python) -> PyObject {
        self.refs.borrow(py).get_entity_factory(py)
    }

    #[getter]
    fn get_event_factory(&self, py: Python) -> PyObject {
        self.refs.borrow(py).get_event_factory(py)
    }

    #[getter]
    fn get_event_manager(&self, py: Python) -> PyObject {
        self.refs.borrow(py).get_event_manager(py)
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
    fn get_intents<'p>(&'p self, py: Python<'p>) -> &'p PyAny {
        self.intents_py.as_ref(py)
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
        self.refs.borrow(py).get_shards()
    }

    #[getter]
    fn get_voice(&self, py: Python) -> PyObject {
        return self.refs.borrow(py).get_voice(py);
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
        pyo3::prepare_freethreaded_python(); // TODO: is this necessary or is this poorly working around a bug?
        let intents = crate::to_intents(intents)?;

        let hikari_impl = py.import("hikari.impl")?;
        let http_settings = http_settings
            .map::<PyResult<PyObject>, _>(Ok)
            .unwrap_or_else(|| Ok(hikari_impl.call_method0("HTTPSettings")?.to_object(py)))?;
        let proxy_settings = proxy_settings
            .map::<PyResult<PyObject>, _>(Ok)
            .unwrap_or_else(|| Ok(hikari_impl.call_method0("ProxySettings")?.to_object(py)))?;

        let shard_map = Arc::new(RwLock::new(HashMap::new()));

        let refs = Py::new(py, _BotRefs {
            entity_factory: None,
            event_factory: None,
            event_manager: None,
            rest: None,
            shards: shard_map.clone(),
            voice: None,
        })?;

        let entity_factory = hikari_impl
            .call_method1("EntityFactoryImpl", (refs.as_ref(py),))?
            .to_object(py);

        // TODO: EventFactoryImpl needs to be exported at hikari.impl.__all__
        let event_factory = py
            .import("hikari.impl.event_factory")?
            .call_method1("EventFactoryImpl", (refs.as_ref(py),))?
            .to_object(py);

        let event_manager = hikari_impl
            .call_method1(
                "EventManagerImpl",
                (
                    entity_factory.as_ref(py),
                    event_factory.as_ref(py),
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
        let rest = hikari_impl
            .getattr("RESTClientImpl")?
            .call((), Some(rest_kwargs))?
            .to_object(py);

        let voice = hikari_impl
            .call_method1("VoiceComponentImpl", (refs.as_ref(py),))?
            .to_object(py);

        let mut refs_mut = refs.borrow_mut(py);
        refs_mut.entity_factory = Some(entity_factory);
        refs_mut.event_factory = Some(event_factory);
        refs_mut.event_manager = Some(event_manager);
        refs_mut.rest = Some(rest);
        refs_mut.voice = Some(voice);
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
            shards: shard_map,
            shard_config: RefCell::new(shards.map(|(from, to, total)| ShardScheme::Range { from, to, total })),
            token,
            gateway_url: gateway_url.to_owned(),
        })
    }

    fn close<'p>(&self, py: Python<'p>) -> PyResult<&'p PyAny> {
        if !self.get_is_alive() {
            return Err(PyErr::new::<ComponentStateConflictError, _>(("Bot isn't running",)));
        }

        let cluster = self.cluster.clone();
        let notify = self.notify.clone();
        fut_into_coro(py, async move {
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
        fut_into_coro(py, async move {
            notify.notified().await;
            Ok(())
        })
    }

    #[args("*", backend = "\"asyncio\"")]
    fn run(&self, py: Python, backend: &str) -> PyResult<PyResult<()>> {
        if self.get_is_alive() {
            return Err(PyErr::new::<ComponentStateConflictError, _>(
                ("Bot is already running",),
            ));
        }

        let fut = self.run_async(py)?;
        let notify = self.notify.clone();
        Ok(pyo3_anyio::tokio::run(
            py,
            async move {
                fut.await?;
                notify.notified().await;
                Ok(())
            },
            backend,
        ))
    }

    fn start<'p>(&self, py: Python<'p>) -> PyResult<&'p PyAny> {
        fut_into_coro(py, self.run_async(py)?)
    }

    fn get_me(&self, py: Python) -> PyObject {
        py.None()
    }

    #[args("*", status = "None", idle_since = "None", activity = "None", afk = "None")]
    fn update_presence<'p>(
        &self,
        py: Python<'p>,
        status: Option<PyObject>,
        idle_since: Option<PyObject>,
        activity: Option<PyObject>,
        afk: Option<PyObject>,
    ) -> PyResult<&'p PyAny> {
        let shards = self.shards.clone();
        let coros = shards
            .try_read()
            .map_err(|_| PyErr::new::<ComponentStateConflictError, _>(("Bot isn't running",)))?
            .values()
            .map(|shard| shard.call_method1(py, "update_presence", (&status, &idle_since, &activity, &afk)))
            .filter_map(Result::ok)
            .collect::<Vec<_>>();

        gather(py)?.call1(pyo3::types::PyTuple::new(py, coros))
    }

    #[args(guild, channel, "*", self_mute = "None", self_deaf = "None")]
    fn update_voice_state<'p>(
        &self,
        py: Python<'p>,
        guild: &'p PyAny,
        channel: Option<&'p PyAny>,
        self_mute: Option<&'p PyAny>,
        self_deaf: Option<&'p PyAny>,
    ) -> PyResult<&'p PyAny> {
        self.get_shard(guild)?
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
        guild: &PyAny,
        include_presences: Option<&PyAny>,
        query: String,
        limit: u64,
        users: Option<&PyAny>,
        nonce: Option<&PyAny>,
    ) -> PyResult<&'p PyAny> {
        self.get_shard(guild)?.borrow(py).request_guild_members(
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

#[pyclass]
struct ConsumeRawEvent {
    callback: PyObject,
    context: Option<PyObject>,
}

#[pymethods]
impl ConsumeRawEvent {
    #[args(args = "*")]
    fn __call__(&self, py: Python, args: &PyTuple) -> PyResult<()> {
        let result = if let Some(ref context) = self.context {
            context.call_method1(
                py,
                "run",
                PyTuple::new(py, [&[self.callback.as_ref(py)], args.as_slice()].concat()),
            )
        } else {
            self.callback.call1(py, args)
        };

        // TODO: switch to `if let Err(err) = result && !err.is_instance_of` when
        // https://github.com/rust-lang/rust/issues/53667 is stabalised.
        if let Err(err) = result {
            if !err.is_instance_of::<PyKeyError>(py) {
                return Err(err);
            }
        };

        Ok(())
    }
}

async fn make_event_handler(
    task_locals: pyo3_anyio::any::TaskLocals,
    shards: Arc<RwLock<HashMap<u64, Py<Shard>>>>,
    consume_raw_event: PyObject,
) -> PyResult<impl FnMut((u64, Event)) -> Ready<()>> {
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
                return ready(());
            }
        };

        let (data, name) = match (parsed.get("d"), parsed.get("t").map(Value::as_str)) {
            (Some(data), Some(Some(name))) => (data, name),
            _ => {
                debug!("Failed to parse event data; this is likely a control event like heartbeat ACK");
                return ready(());
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
            let name = name.to_object(py);

            // TODO: error handling
            let result = task_locals.call_soon1(consume_raw_event.as_ref(py), &[
                name.as_ref(py),
                shard.as_ref(py),
                data.as_ref(py),
            ]);
            if let Err(err) = result {
                warn!(err = as_error!(err); "Failed to call call_soon_threadsafe");
            }
        });

        ready(())
    })
}
