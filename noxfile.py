# -*- coding: utf-8 -*-
# cython: language_level=3
# BSD 3-Clause License
#
# Copyright (c) 2020-2022, Faster Speeding
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
from __future__ import annotations

import itertools
import pathlib
from collections import abc as collections

import nox

nox.options.sessions = ["reformat", "spell-check", "type-check", "verify-types"]
GENERAL_TARGETS = ["./noxfile.py", "./rukari.pyi"]
_DEV_DEP_DIR = pathlib.Path("./dev-requirements")


def _dev_dep(*values: str) -> collections.Iterator[str]:
    return itertools.chain.from_iterable(("-r", str(_DEV_DEP_DIR / f"{value}.txt")) for value in values)


def install_requirements(session: nox.Session, *requirements: str, first_call: bool = True) -> None:
    # --no-install --no-venv leads to it trying to install in the global venv
    # as --no-install only skips "reused" venvs and global is not considered reused.
    if not _try_find_option(session, "--skip-install", when_empty="True"):
        if first_call:
            session.install("--upgrade", "wheel")

        session.install("--upgrade", *map(str, requirements))

    elif "." in requirements:
        session.install("--upgrade", "--force-reinstall", "--no-dependencies", ".")


def _try_find_option(session: nox.Session, name: str, *other_names: str, when_empty: str | None = None) -> str | None:
    args_iter = iter(session.posargs)
    names = {name, *other_names}

    for arg in args_iter:
        if arg in names:
            return next(args_iter, when_empty)


@nox.session(venv_backend="none")
def cleanup(session: nox.Session) -> None:
    """Cleanup any temporary files made in this project by its nox tasks."""
    import shutil

    # Remove directories
    for raw_path in ["./.nox"]:
        path = pathlib.Path(raw_path)
        try:
            shutil.rmtree(str(path.absolute()))

        except Exception as exc:
            session.warn(f"[ FAIL ] Failed to remove '{raw_path}': {exc!s}")

        else:
            session.log(f"[  OK  ] Removed '{raw_path}'")


def _pip_compile(session: nox.Session, /, *args: str) -> None:
    install_requirements(session, *_dev_dep("freeze_deps"))
    for path in pathlib.Path("./dev-requirements/").glob("*.in"):
        session.run(
            "pip-compile",
            str(path),
            "--output-file",
            str(path.with_name(path.name.removesuffix(".in") + ".txt")),
            *args
            # "--generate-hashes",
        )


@nox.session(name="freeze-dev-deps", reuse_venv=True)
def freeze_dev_deps(session: nox.Session) -> None:
    """Freeze the dev dependencies."""
    _pip_compile(session)


@nox.session(name="upgrade-dev-deps", reuse_venv=True)
def upgrade_dev_deps(session: nox.Session) -> None:
    """Upgrade the dev dependencies."""
    _pip_compile(session, "--upgrade")


@nox.session(reuse_venv=True, name="spell-check")
def spell_check(session: nox.Session) -> None:
    """Check this project's text-like files for common spelling mistakes."""
    install_requirements(session, *_dev_dep("lint"))
    session.run(
        "codespell",
        *GENERAL_TARGETS,
        ".gitattributes",
        ".gitignore",
        "LICENSE",
        "pyproject.toml",
        "README.md",
        "./github",
        ".pre-commit-config.yaml",
    )


@nox.session(reuse_venv=True)
def reformat(session: nox.Session) -> None:
    """Reformat this project's modules to fit the standard style."""
    install_requirements(session, *_dev_dep("reformat"))
    session.run("black", *GENERAL_TARGETS, "--extend-exclude", "^/tanjun/_internal/vendor/.*$")
    session.run("isort", *GENERAL_TARGETS)
    session.run("sort-all", *map(str, pathlib.Path("./tanjun/").glob("**/*.py")), success_codes=[0, 1])


def _run_pyright(session: nox.Session, *args: str) -> None:
    if _try_find_option(session, "--force-env", when_empty="True"):
        session.env["PYRIGHT_PYTHON_GLOBAL_NODE"] = "off"

    if version := _try_find_option(session, "--pyright-version"):
        session.env["PYRIGHT_PYTHON_FORCE_VERSION"] = version

    session.run("python", "-m", "pyright", "--version")
    session.run("python", "-m", "pyright", *args)


@nox.session(name="type-check", reuse_venv=True)
def type_check(session: nox.Session) -> None:
    """Statically analyse and veirfy this project using Pyright."""
    # TODO: https://github.com/pypa/pip/issues/11440
    install_requirements(session, "hikari", *_dev_dep("nox", "type_check"))
    _run_pyright(session)
    session.run("python", "-m", "mypy", "rukari.pyi", "--show-error-codes")


@nox.session(name="verify-types", reuse_venv=True)
def verify_types(session: nox.Session) -> None:
    """Verify the "type completeness" of types exported by the library using Pyright."""
    install_requirements(session, ".", *_dev_dep("type_check"))
    _run_pyright(session, "--verifytypes", "rukari", "--ignoreexternal")
