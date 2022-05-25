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
use std::time::Duration;

use pyo3::conversion::ToPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::types::{IntoPyDict, PyDict, PyModule, PyType};
use pyo3::{pyclass, pymethods, pymodule, AsPyPointer, Py, PyAny, PyObject, PyResult, Python};
use twilight_gateway::cluster::{Cluster, ShardScheme};
use twilight_gateway::shard::Shard;
use twilight_model::gateway::payload::outgoing::identify::IdentifyProperties;
use twilight_model::gateway::Intents;
// use tokio::stream::Stream;


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
        .map(Intents::from_bits) // Intents::MESSAGE_CONTENT |
        .unwrap_or_else(|| Some(Intents::all() & !(Intents::GUILD_MEMBERS | Intents::GUILD_PRESENCES)))
        .ok_or_else(|| PyValueError::new_err("Invalid intent(s) passed"))
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
    cluster: Option<Cluster>,
    intents: Intents,
    intents_py: PyObject,
    refs: Py<_BotRefs>,
    shards: Option<(u64, u64, u64)>,
    token: String,
}

// #[inline]
// fn _get_latency(shard: &Shard) -> (u64, f64) {
//     let latency = shard
//         .info()
//         .ok()
//         .and_then(|info| info.latency().recent().back().map(|duration|
// duration.as_secs_f64()))         .unwrap_or(f64::NAN);

//     return (shard.config().shard()[0], latency);
// }

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
        if self.cluster.is_none() {
            return HashMap::new();
        }

        self.cluster
            .as_ref()
            .unwrap()
            .shards()
            .map(|shard| {
                let latency = shard
                    .info()
                    .ok()
                    .and_then(|info| info.latency().recent().back().map(|duration| duration.as_secs_f64()))
                    .unwrap_or(f64::NAN);

                (shard.config().shard()[0], latency)
            })
            .collect()
    }

    // #[getter(heartbeat_latencies)]
    // fn get_heartbeat_latencies(&self) -> HashMap<u64, f64> {
    //     self.cluster
    //         .as_ref()
    //         .map(|cluster| cluster.shards().map(_get_latency).collect())
    //         .unwrap_or_else(HashMap::new)
    // }

    #[getter(heartbeat_latency)]
    fn get_heartbeat_latency(&self) -> f64 {
        self.cluster
            .as_ref()
            .and_then(|cluster| {
                let latencies = cluster
                    .shards()
                    .filter_map(|shard| shard.info().ok())
                    .filter_map(|info| info.latency().recent().back().map(Duration::as_secs_f64))
                    .collect::<Vec<f64>>();

                let len = latencies.len();
                if len == 0 {
                    None
                } else {
                    Some(latencies.into_iter().sum::<f64>() / len as f64)
                }
            })
            .unwrap_or(f64::NAN)
    }

    #[getter(http_settings)]
    fn get_http_settings(&self, py: Python) -> PyResult<PyObject> {
        self.refs.borrow(py).get_http_settings(py)
    }

    #[getter(is_alive)]
    fn get_is_alive(&self) -> bool {
        self.cluster.is_some()
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
        self.cluster
            .as_ref()
            .and_then(|cluster| cluster.shards().next().map(|shard| shard.config().shard()[1]))
            .unwrap_or(0)
    }

    #[getter(shards)]
    fn get_shards(&self, py: Python) -> PyObject {
        PyDict::new(py).to_object(py)
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
            cluster: None,
            refs,
            intents,
            intents_py,
            shards,
            token,
        })
    }

    fn close(&mut self) {
    }

    fn join(&mut self) {
    }

    fn run(&mut self) {
    }

    fn start(&mut self) -> PyResult<()> {
        let mut cluster = Cluster::builder(self.token.clone(), self.intents)
            .event_types(twilight_gateway::EventTypeFlags::SHARD_PAYLOAD)
            .identify_properties(IdentifyProperties::new("hikari", "hikari", std::env::consts::OS));

        if let Some((from, to, total)) = self.shards {
            cluster = cluster.shard_scheme(ShardScheme::Range { from, to, total });
        }

        // let (cluster, events) = cluster.build().await?;
        Ok(())
    }

    fn get_me(&self, py: Python) -> PyObject {
        py.None()
    }

    fn update_presence(&self) {
    }

    fn update_voice_state(&self) {
    }

    fn request_guild_members(&self) {
    }
}

// #[pyo3_asyncio::tokio::main(flavor = "current_thread")]

#[pymodule]
fn rukari(python: Python<'_>, module: &PyModule) -> PyResult<()> {
    let _ = pyo3_log::try_init();

    module.add_class::<BotManager>()?;
    module.add_class::<Bot>()?;
    let bot_type = module.getattr("Bot")?.cast_as::<PyType>()?;
    let traits = python.import("hikari.traits")?;

    assert!(
        bot_type.is_subclass(traits.getattr("RESTAware")?.cast_as::<PyType>()?)?,
        "Bot isn't RESTAware"
    );
    assert!(
        bot_type.is_subclass(traits.getattr("Runnable")?.cast_as::<PyType>()?)?,
        "Bot isn't Runnable"
    );
    // This would require being voice aware which probably isn't going to happen
    // right now.
    // assert!(
    //     bot_type.is_subclass(traits.getattr("ShardAware")?.cast_as::<PyType>()?)?
    // ,     "Bot isn't ShardAware"
    // );
    assert!(
        bot_type.is_subclass(traits.getattr("EventFactoryAware")?.cast_as::<PyType>()?)?,
        "Bot isn't EventFactoryAware"
    );

    Ok(())
}
