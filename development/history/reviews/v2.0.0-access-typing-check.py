#!/usr/bin/env python3
"""Finite typing-boundary design check; not a Nocter parser, compiler, or proof.

Checks Copy evidence, source availability, referent versus payload/region dependencies,
and binding/call/return boundaries. It does not model general CFGs or type constructors.
"""

from dataclasses import dataclass, replace
from itertools import product
import json


@dataclass(frozen=True)
class Source:
    kind: str
    copy_proof: bool = False
    concrete_copy: bool = False
    location: str | None = None
    payload: frozenset = frozenset()
    storage: frozenset = frozenset()
    initialized: bool = True

    @property
    def referent(self):
        return frozenset((self.location,)) if self.kind in ("read", "update") else frozenset()


@dataclass(frozen=True)
class Result:
    mode: str
    referent: frozenset
    payload: frozenset
    storage: frozenset
    copied: bool = False
    transferred: bool = False
    source_available: bool = True


OPERATIONS = ("bind", "argX", "returnI", "returnU", "returnX", "transfer")


def result(source, mode, *, copied=False, transferred=False):
    return Result(mode, frozenset() if copied or transferred else source.referent,
                  source.payload, source.storage, copied, transferred, not transferred)


def check_boundary(source, operation):
    assert operation in OPERATIONS
    assert source.kind in ("owner", "fresh", "read", "update", "value")
    assert not source.copy_proof or source.concrete_copy
    if not source.initialized:
        return None
    answer = None
    if operation == "transfer":
        if source.kind == "owner":
            answer = result(source, "X", transferred=True)
    elif operation == "argX":
        if source.kind in ("owner", "fresh"):
            answer = result(source, "X", transferred=True)
        elif source.copy_proof:
            answer = result(source, "X", copied=True)
    elif operation == "bind":
        if source.kind == "fresh":
            answer = result(source, "X", transferred=True)
        elif source.kind == "update":
            answer = result(source, "U")
        elif source.copy_proof:
            answer = result(source, "X", copied=True)
        elif source.kind == "read":
            answer = result(source, "I")
    elif operation == "returnX":
        if source.kind == "fresh":
            answer = result(source, "X", transferred=True)
        elif source.copy_proof:
            answer = result(source, "X", copied=True)
    elif operation == "returnI":
        if source.copy_proof:
            answer = result(source, "I", copied=True)
        elif source.kind in ("read", "update"):
            answer = result(source, "I")
    elif operation == "returnU" and source.kind == "update":
        answer = result(source, "U")
    if answer is not None and operation.startswith("return"):
        if "local" in answer.referent | answer.payload | answer.storage:
            return None
    return answer


def invariant(source, operation, answer):
    """Inspect accepted evidence independently of the boundary selector's branches."""
    assert source.initialized, "read/transfer needs definite initialization"
    assert answer.payload == source.payload, "payload dependencies never disappear in Copy or transfer"
    assert answer.storage == source.storage, "storage requirements survive materialization"
    assert answer.mode in ("I", "U", "X")
    assert not answer.copied or source.copy_proof, "Copy requires source-context evidence"
    assert not (answer.copied and answer.transferred), "copying and transfer have distinct source effects"
    if answer.copied:
        assert not answer.referent and answer.source_available, "Copy detaches only its own referent access"
    if answer.transferred:
        assert source.kind in ("owner", "fresh"), "only an owner supplies transferred ownership"
        assert answer.mode == "X" and not answer.source_available and not answer.referent
    else:
        assert answer.source_available, "nontransfer operation leaves source availability intact"
    if answer.mode == "X":
        assert answer.copied or answer.transferred, "ownership cannot appear by permission strengthening"
    if answer.mode == "U":
        assert source.kind == "update" and answer.referent == source.referent, "update keeps its target"
    if answer.mode == "I" and not answer.copied:
        assert source.kind in ("read", "update") and answer.referent == source.referent
    if operation == "bind" and source.kind == "update":
        assert answer.mode == "U" and not answer.copied, "binding U does not become a snapshot"
    if operation == "argX":
        if source.kind in ("owner", "fresh"):
            assert answer.transferred and not answer.copied, "existing Copy owner still transfers into X"
        else:
            assert answer.copied and answer.source_available, "non-owning X input needs an independent Copy"
    if operation == "transfer":
        assert source.kind == "owner" and answer.transferred, "standalone transfer targets existing owner"
    if operation.startswith("return"):
        assert answer.mode == operation[-1]
        assert "local" not in answer.referent | answer.payload | answer.storage, "result survives ordinary cleanup"


def sources():
    dependency_sets = tuple(frozenset(root for root, included in zip(("outside", "local"), flags) if included)
                            for flags in product((False, True), repeat=2))
    for kind in ("owner", "fresh", "read", "update", "value"):
        locations = ("local", "outside") if kind in ("read", "update") else (None,)
        for proof, concrete in ((False, False), (False, True), (True, True)):
            if kind == "value" and not proof:
                continue
            for location, payload, storage, initialized in product(
                    locations, dependency_sets, dependency_sets, (False, True)):
                yield Source(kind, proof, concrete, location, payload, storage, initialized)


def scenarios():
    scalar = Source("owner", True, True)
    assert check_boundary(scalar, "bind").source_available
    assert not check_boundary(scalar, "argX").source_available
    assert not check_boundary(scalar, "transfer").source_available
    assert check_boundary(scalar, "returnX").copied
    assert check_boundary(Source("owner"), "bind") is None
    assert check_boundary(Source("fresh"), "transfer") is None

    header = Source("read", True, True, "outside", frozenset(("outside",)))
    copied = check_boundary(header, "bind")
    assert copied.referent == frozenset() and copied.payload == header.payload
    assert copied.source_available and header.referent == frozenset(("outside",))
    assert check_boundary(header, "argX").copied and check_boundary(header, "argX").source_available
    assert check_boundary(replace(header, kind="update"), "bind").mode == "U"
    local_scalar = Source("read", True, True, "local")
    assert check_boundary(local_scalar, "returnI").referent == frozenset()
    assert check_boundary(replace(local_scalar, payload=frozenset(("local",))), "returnI") is None
    assert check_boundary(replace(local_scalar, storage=frozenset(("local",))), "returnX") is None
    assert check_boundary(Source("read", location="local"), "returnI") is None

    generic = Source("read", False, True, "outside")
    assert check_boundary(generic, "bind").mode == "I"
    assert check_boundary(generic, "argX") is None
    assert check_boundary(replace(generic, copy_proof=True), "bind").mode == "X"

    update = replace(header, kind="update")
    region_copy = replace(header, storage=frozenset(("outside",)))
    local_dependency = replace(header, payload=frozenset(("local",)))
    invalid_results = (
        (header, "bind", replace(copied, payload=frozenset())),
        (region_copy, "bind", replace(check_boundary(region_copy, "bind"), storage=frozenset())),
        (generic, "bind", result(generic, "X", copied=True)),
        (update, "bind", copied),
        (scalar, "argX", check_boundary(scalar, "bind")),
        (header, "argX", result(header, "X", transferred=True)),
        (local_dependency, "returnX", result(local_dependency, "X", copied=True)),
        (header, "returnU", result(header, "U")),
        (header, "bind", replace(copied, source_available=False)),
    )
    for source, operation, answer in invalid_results:
        try:
            invariant(source, operation, answer)
        except AssertionError:
            continue
        raise AssertionError("invariant accepted an intentionally invalid typing result")
    return len(invalid_results)


def explore():
    attempts = accepted = stable_specializations = 0
    for source in sources():
        for operation in OPERATIONS:
            attempts += 1
            answer = check_boundary(source, operation)
            if answer is not None:
                invariant(source, operation, answer)
                accepted += 1
            if not source.copy_proof:
                other = check_boundary(replace(source, concrete_copy=not source.concrete_copy), operation)
                assert other == answer, "specialization cannot reselect the checked source operation"
                stable_specializations += 1
    return attempts, accepted, stable_specializations


if __name__ == "__main__":
    if not __debug__:
        raise SystemExit("Run without -O: this design check requires assertions")
    mutants = scenarios()
    attempts, accepted, stable = explore()
    print(json.dumps({"boundary_attempts": attempts, "accepted_boundary_checks": accepted,
                      "specialization_stability_checks": stable,
                      "invalid_result_mutants_rejected": mutants, "result": "pass"}, sort_keys=True))
