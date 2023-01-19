# -*- coding: utf-8 -*-
# BSD 3-Clause License
#
# Copyright (c) 2022, Faster Speeding
# All rights reserved.
#
# Redistribution and use in source and binary forms, with or without
# modification, are permitted provided that the following conditions are met:
#
# * Redistributions of source code must retain the above copyright notice, this
#   list of conditions and the following disclaimer.
#
# * Redistributions in binary form must reproduce the above copyright notice,
#   this list of conditions and the following disclaimer in the documentation
#   and/or other materials provided with the distribution.
#
# * Neither the name of the copyright holder nor the names of its
#   contributors may be used to endorse or promote products derived from
#   this software without specific prior written permission.
#
# THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
# AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
# IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
# DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
# FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
# DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
# SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
# CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
# OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
# OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
import datetime
from collections import abc as collections
from concurrent import futures

import hikari

class Bot(
    hikari.EventFactoryAware,
    hikari.EventManagerAware,
    hikari.Runnable,
    hikari.RESTAware,
    hikari.ShardAware,
):
    __slots__: collections.Iterable[str]

    def __init__(
        self,
        token: str,
        /,
        *,
        http_settings: hikari.impl.HTTPSettings = ...,
        intents: hikari.Intents | int = ...,
        proxy_settings: hikari.impl.ProxySettings = ...,
        max_rate_limit: float = ...,
        max_retries: int = ...,
        shards: tuple[int, int, int] = ...,
        rest_url: str = ...,
        gateway_url: str = ...,
    ) -> None: ...
    @property
    def entity_factory(self) -> hikari.api.EntityFactory: ...
    @property
    def event_factory(self) -> hikari.api.EventFactory: ...
    @property
    def event_manager(self) -> hikari.api.EventManager: ...
    @property
    def executor(self) -> futures.Executor | None: ...
    @property
    def heartbeat_latencies(self) -> collections.Mapping[int, float]: ...
    @property
    def heartbeat_latency(self) -> float: ...
    @property
    def http_settings(self) -> hikari.api.HTTPSettings: ...
    @property
    def intents(self) -> hikari.Intents: ...
    @property
    def is_alive(self) -> bool: ...
    @property
    def proxy_settings(self) -> hikari.api.ProxySettings: ...
    @property
    def rest(self) -> hikari.api.RESTClient: ...
    @property
    def shard_count(self) -> int: ...
    @property
    def shards(self) -> collections.Mapping[int, hikari.api.GatewayShard]: ...
    @property
    def voice(self) -> hikari.api.VoiceComponent: ...
    async def close(self) -> None: ...
    def get_me(self) -> hikari.OwnUser | None: ...
    async def join(self) -> None: ...
    async def request_guild_members(
        self,
        guild: hikari.SnowflakeishOr[hikari.PartialGuild],
        *,
        include_presences: hikari.UndefinedOr[bool] = ...,
        query: str = ...,
        limit: int = ...,
        users: hikari.UndefinedOr[hikari.SnowflakeishSequence[hikari.User]] = ...,
        nonce: hikari.UndefinedOr[str] = ...,
    ) -> None: ...
    def run(self, *, backend: str = ...) -> None: ...
    async def start(self) -> None: ...
    async def update_presence(
        self,
        *,
        status: hikari.UndefinedOr[hikari.Status] = ...,
        idle_since: hikari.UndefinedNoneOr[datetime.datetime] = ...,
        activity: hikari.UndefinedNoneOr[hikari.Activity] = ...,
        afk: hikari.UndefinedOr[bool] = ...,
    ) -> None: ...
    async def update_voice_state(
        self,
        guild: hikari.SnowflakeishOr[hikari.PartialGuild],
        channel: hikari.SnowflakeishOr[hikari.GuildVoiceChannel] | None,
        *,
        self_mute: hikari.UndefinedOr[bool] = ...,
        self_deaf: hikari.UndefinedOr[bool] = ...,
    ) -> None: ...
