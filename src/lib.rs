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
#![feature(once_cell)]
#![allow(clippy::too_many_arguments)] // This isn't compatible with the Python functions we're implementing.
#![allow(clippy::borrow_deref_ref)] // Leads to a ton of false positives around args of py types.
use pyo3::types::PyType;
use pyo3::PyResult;
use twilight_gateway::cluster::ShardScheme;
use twilight_model::gateway::Intents;

mod bot;
mod shard;

pub(crate) fn to_intents(intents: Option<u64>) -> PyResult<Intents> {
    intents
        .map(Intents::from_bits)
        .unwrap_or_else(|| {
            Some(Intents::all() & !(Intents::GUILD_MEMBERS | Intents::GUILD_PRESENCES | Intents::MESSAGE_CONTENT))
        })
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Invalid intent(s) passed"))
}


pub(crate) fn fetch_shards_info(token: &str) -> Result<ShardScheme, Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            // TOOD: handle back off
            twilight_http::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .token(token.to_string())
                .build()
                .gateway()
                .authed()
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


#[pyo3::pymodule]
fn rukari(py: pyo3::Python<'_>, module: &pyo3::types::PyModule) -> PyResult<()> {
    let _ = pyo3_log::try_init();

    let bot_type = PyType::new::<crate::bot::Bot>(py);
    let hikari = py.import("hikari")?;
    let traits = hikari.getattr("traits")?;
    hikari
        .getattr("api")?
        .getattr("GatewayShard")?
        .call_method1("register", (PyType::new::<crate::shard::Shard>(py),))?;

    module.add_class::<crate::bot::Bot>()?;

    assert!(
        bot_type.is_subclass(traits.getattr("EventFactoryAware")?.cast_as::<PyType>()?)?,
        "Bot isn't EventFactoryAware"
    );
    assert!(
        bot_type.is_subclass(traits.getattr("EventManagerAware")?.cast_as::<PyType>()?)?,
        "Bot isn't EventManagerAware"
    );
    assert!(
        bot_type.is_subclass(traits.getattr("RESTAware")?.cast_as::<PyType>()?)?,
        "Bot isn't RESTAware"
    );
    assert!(
        bot_type.is_subclass(traits.getattr("Runnable")?.cast_as::<PyType>()?)?,
        "Bot isn't Runnable"
    );
    assert!(
        bot_type.is_subclass(traits.getattr("ShardAware")?.cast_as::<PyType>()?)?,
        "Bot isn't ShardAware"
    );

    Ok(())
}
