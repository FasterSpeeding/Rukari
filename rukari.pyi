# -*- coding: utf-8 -*-
# cython: language_level=3
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
from collections import abc as _collections
import datetime
import typing

import hikari

__author__: typing.Final[str]
__ci__: typing.Final[str]
__copyright__: typing.Final[str]
__coverage__: typing.Final[str]
__docs__: typing.Final[str]
__email__: typing.Final[str]
__issue_tracker__: typing.Final[str]
__license__: typing.Final[str]
__url__: typing.Final[str]
__version__: typing.Final[str]

class Bot(
    hikari.EventFactoryAware,
    hikari.EventManagerAware,
    hikari.IntentsAware,
    hikari.RESTAware,
    hikari.Runnable,
    hikari.EventFactoryAware,
):
    __slots__: _collections.Iterable[str]

    def __init__(
        token: str,
        /,
        *,
        http_settings: hikari.impl.HTTPSettings | None = None,
        intents: hikari.Intents | int | None = None,
        proxy_settings: hikari.impl.ProxySettings | None = None,
        max_rate_limit: float = 300.0,
        max_retries: int = 3,
        shards: tuple[int, int, int] | None = None,
    ) -> None: ...
    @property
    def heartbeat_latencies(self) -> _collections.Mapping[int, float]: ...
    @property
    def heartbeat_latency(self) -> float: ...
    @property
    def shards(self) -> typing.Mapping[int, hikari.api.GatewayShard]: ...
    @property
    def shard_count(self) -> int: ...
    def get_me(self) -> typing.Optional[hikari.OwnUser]: ...
    async def update_presence(
        self,
        *,
        status: hikari.Status | hikari.UndefinedType = hikari.UNDEFINED,
        idle_since: datetime.datetime | hikari.UndefinedType | None = hikari.UNDEFINED,
        activity: hikari.Activity | hikari.UndefinedType | None = hikari.UNDEFINED,
        afk: bool | hikari.UndefinedType = hikari.UNDEFINED,
    ) -> None: ...
    async def update_voice_state(
        self,
        guild: hikari.Snowflakeish | hikari.PartialGuild,
        channel: hikari.Snowflakeish | hikari.GuildVoiceChannel | None,
        *,
        self_mute: bool | hikari.UndefinedType = hikari.UNDEFINED,
        self_deaf: bool | hikari.UndefinedType = hikari.UNDEFINED,
    ) -> None: ...
    async def request_guild_members(
        self,
        guild: hikari.Snowflakeish | hikari.PartialGuild,
        *,
        include_presences: bool | hikari.UndefinedType = hikari.UNDEFINED,
        query: str = "",
        limit: int = 0,
        users: hikari.SnowflakeishSequence[hikari.User]
        | hikari.UndefinedType = hikari.UNDEFINED,
        nonce: str | hikari.UndefinedType = hikari.UNDEFINED,
    ) -> None: ...
