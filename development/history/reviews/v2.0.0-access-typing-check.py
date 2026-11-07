#!/usr/bin/env python3
"""Finite typing-boundary design check; not a Nocter parser, compiler, or proof.

Checks Copy evidence, source availability, referent versus payload/region dependencies,
and binding/call/return boundaries, including S9 operation-directed local inference.
It does not parse Nocter or model general CFGs, delegation graphs, or type constructors.
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
    access_result: bool = False

    @property
    def referent(self):
        if self.kind == "owner":
            return frozenset(("local",))
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


OPERATIONS = ("infer", "bindI", "bindU", "bindX", "argX", "returnI", "returnU", "returnX", "transfer")


def inferred_binding(source):
    if source.kind == "fresh":
        return "bindX"
    if source.kind == "update" and source.access_result:
        return "bindU"
    return "bindI"


def result(source, mode, *, copied=False, transferred=False):
    return Result(mode, frozenset() if copied or transferred else source.referent,
                  source.payload, source.storage, copied, transferred, not transferred)


def check_boundary(source, operation):
    assert operation in OPERATIONS
    assert source.kind in ("owner", "fresh", "read", "update", "value")
    assert not source.copy_proof or source.concrete_copy
    if not source.initialized:
        return None
    if operation == "infer":
        return check_boundary(source, inferred_binding(source))
    answer = None
    if operation == "transfer":
        if source.kind == "owner":
            answer = result(source, "X", transferred=True)
    elif operation == "argX":
        if source.kind in ("owner", "fresh"):
            answer = result(source, "X", transferred=True)
        elif source.copy_proof:
            answer = result(source, "X", copied=True)
    elif operation == "bindI":
        if source.kind in ("owner", "read", "update", "value"):
            answer = result(source, "I")
        elif source.kind == "fresh" and source.copy_proof:
            answer = result(source, "I", copied=True)
    elif operation == "bindU":
        if source.kind in ("owner", "update"):
            answer = result(source, "U")
    elif operation == "bindX":
        if source.kind == "fresh":
            answer = result(source, "X", transferred=True)
        elif source.copy_proof:
            answer = result(source, "X", copied=True)
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
        assert source.kind in ("owner", "update") and answer.referent == source.referent, "update keeps its target"
    if answer.mode == "I" and not answer.copied:
        assert source.kind in ("owner", "read", "update", "value") and answer.referent == source.referent
    if operation.startswith("bind"):
        assert answer.mode == operation[-1], "the written local mode is authoritative"
        if operation in ("bindI", "bindU"):
            if source.kind == "fresh":
                assert operation == "bindI" and answer.copied, "fresh I needs independent Copy, never a hidden owner"
            else:
                assert not answer.copied and not answer.transferred, "named-place I/U binding keeps its access"
        if operation == "bindX" and source.kind == "owner":
            assert answer.copied and not answer.transferred, "owning destination alone is not an explicit source move"
    if operation == "infer":
        expected = "X" if source.kind == "fresh" else "U" if source.kind == "update" and source.access_result else "I"
        assert answer.mode == expected, "infer from the operation, not the named source's stored permission"
        assert not answer.copied, "inference does not silently materialize a Copy"
        assert answer.transferred == (source.kind == "fresh"), "a bare owner read never infers a move"
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
            forms = (False, True) if kind in ("read", "update") else (False,)
            for location, payload, storage, initialized, access_result in product(
                    locations, dependency_sets, dependency_sets, (False, True), forms):
                yield Source(kind, proof, concrete, location, payload, storage, initialized, access_result)


def scenarios():
    scalar = Source("owner", True, True)
    assert check_boundary(scalar, "bindI").mode == "I"
    assert check_boundary(scalar, "bindU").mode == "U"
    assert check_boundary(scalar, "bindX").source_available
    assert not check_boundary(scalar, "argX").source_available
    assert not check_boundary(scalar, "transfer").source_available
    assert check_boundary(scalar, "returnX").copied
    assert check_boundary(Source("owner"), "bindX") is None
    assert check_boundary(Source("owner"), "bindI").mode == "I"
    assert check_boundary(Source("fresh", True, True), "bindI").copied
    assert check_boundary(Source("fresh"), "bindI") is None
    assert check_boundary(Source("fresh"), "bindU") is None
    assert check_boundary(Source("fresh"), "bindX").mode == "X"
    assert check_boundary(Source("fresh"), "transfer") is None

    # One source operation cannot change meaning because of Copy or a later use.
    assert check_boundary(scalar, "infer").mode == "I"
    assert check_boundary(scalar, "infer").source_available
    assert check_boundary(Source("fresh"), "infer").mode == "X"
    # Literal/arithmetic construction supplies a fresh Copy value; no annotation is needed to own it.
    literal = Source("fresh", True, True)
    assert check_boundary(literal, "infer").mode == "X"
    assert check_boundary(literal, "bindI").copied
    assert check_boundary(Source("value", True, True), "infer").mode == "I"
    generic_fresh = Source("fresh", False, True)
    assert check_boundary(generic_fresh, "infer").mode == "X"
    assert check_boundary(generic_fresh, "bindI") is None
    dependent_fresh = Source("fresh", True, True, payload=frozenset(("local",)), storage=frozenset(("outside",)))
    read_answer = check_boundary(dependent_fresh, "bindI")
    assert read_answer.copied and read_answer.payload == dependent_fresh.payload
    assert read_answer.storage == dependent_fresh.storage
    retained = Source("value", True, True, payload=read_answer.payload, storage=read_answer.storage)
    assert check_boundary(retained, "returnI") is None
    assert check_boundary(Source("update", location="outside"), "infer").mode == "I"
    assert check_boundary(Source("update", location="outside", access_result=True), "infer").mode == "U"
    assert check_boundary(Source("read", location="outside", access_result=True), "infer").mode == "I"

    # Model first -> ^first -> fresh result -> next -> inspect, preserving carried dependencies.
    first = Source("owner", payload=frozenset(("outside",)))
    moved = check_boundary(first, "transfer")
    assert not moved.source_available
    supplied = Source("fresh", payload=moved.payload, storage=moved.storage)
    next_owner = check_boundary(supplied, "infer")
    assert next_owner.mode == "X" and next_owner.payload == first.payload
    assert check_boundary(replace(first, initialized=False), "infer") is None
    inspect = check_boundary(first, "infer")
    assert inspect.mode == "I" and inspect.source_available and inspect.referent == first.referent

    header = Source("read", True, True, "outside", frozenset(("outside",)))
    copied = check_boundary(header, "bindX")
    assert copied.referent == frozenset() and copied.payload == header.payload
    assert copied.source_available and header.referent == frozenset(("outside",))
    assert check_boundary(header, "argX").copied and check_boundary(header, "argX").source_available
    assert check_boundary(header, "bindI").referent == header.referent
    assert check_boundary(header, "bindU") is None
    assert check_boundary(replace(header, kind="update"), "bindU").mode == "U"
    local_scalar = Source("read", True, True, "local")
    assert check_boundary(local_scalar, "returnI").referent == frozenset()
    assert check_boundary(replace(local_scalar, payload=frozenset(("local",))), "returnI") is None
    assert check_boundary(replace(local_scalar, storage=frozenset(("local",))), "returnX") is None
    assert check_boundary(Source("read", location="local"), "returnI") is None

    generic = Source("read", False, True, "outside")
    assert check_boundary(generic, "bindI").mode == "I"
    assert check_boundary(generic, "bindX") is None
    assert check_boundary(generic, "argX") is None
    assert check_boundary(replace(generic, copy_proof=True), "bindI").mode == "I"
    assert check_boundary(replace(generic, copy_proof=True), "bindX").mode == "X"

    update = replace(header, kind="update")
    region_copy = replace(header, storage=frozenset(("outside",)))
    local_dependency = replace(header, payload=frozenset(("local",)))
    invalid_results = (
        (header, "bindX", replace(copied, payload=frozenset())),
        (region_copy, "bindX", replace(check_boundary(region_copy, "bindX"), storage=frozenset())),
        (generic, "bindX", result(generic, "X", copied=True)),
        (header, "bindI", copied),
        (update, "bindU", copied),
        (scalar, "argX", check_boundary(scalar, "bindX")),
        (header, "argX", result(header, "X", transferred=True)),
        (local_dependency, "returnX", result(local_dependency, "X", copied=True)),
        (header, "returnU", result(header, "U")),
        (header, "bindX", replace(copied, source_available=False)),
        (Source("fresh", True, True), "bindI", Result("I", frozenset(), frozenset(), frozenset())),
        (scalar, "infer", result(scalar, "X", transferred=True)),
        (Source("update", location="outside"), "infer", result(Source("update", location="outside"), "U")),
        (Source("update", location="outside", access_result=True), "infer", result(Source("update", location="outside"), "I")),
        (Source("fresh"), "infer", Result("I", frozenset(), frozenset(), frozenset())),
    )
    for source, operation, answer in invalid_results:
        try:
            invariant(source, operation, answer)
        except AssertionError:
            continue
        raise AssertionError("invariant accepted an intentionally invalid typing result")
    return len(invalid_results)


def explore():
    attempts = accepted = stable_specializations = stable_bindings = 0
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
            if operation in ("infer", "bindI", "bindU") and source.kind != "value":
                if operation == "bindI" and source.kind == "fresh":
                    continue  # Copy evidence controls independent I materialization, never inferred mode.
                other_source = replace(source, copy_proof=not source.copy_proof, concrete_copy=True)
                assert check_boundary(other_source, operation) == answer, "Copy cannot change inferred or I/U binding permission"
                stable_bindings += 1
    return attempts, accepted, stable_specializations, stable_bindings


@dataclass(frozen=True)
class Slot:
    mode: str
    initialized: bool


@dataclass(frozen=True)
class Assignment:
    slot: Slot
    destroyed_old: bool = False
    installed_new: bool = False


def assign(slot, succeeded):
    if slot.mode == "I" or slot.mode == "U" and not slot.initialized:
        return None
    if not succeeded:
        return Assignment(slot)
    return Assignment(replace(slot, initialized=True), slot.initialized, True)


def assignment_invariant(before, succeeded, after):
    assert before.mode in ("U", "X"), "only update/ownership grants assignment"
    assert before.mode != "U" or before.initialized, "U cannot expose an extraction hole"
    assert after.slot.mode == before.mode, "assignment does not change binding permission"
    if succeeded:
        assert after.slot.initialized and after.installed_new
        assert after.destroyed_old == before.initialized, "reinitialization cannot destroy a nonexistent old value"
    else:
        assert after.slot == before and not after.installed_new and not after.destroyed_old, "failure performs no replacement; before is the post-RHS-side-effect checkpoint"


def transfer_slot(slot):
    if slot.mode != "X" or not slot.initialized:
        return None
    return replace(slot, initialized=False)


def assignment_checks():
    accepted = transfers = 0
    for mode, initialized in product(("I", "U", "X"), (False, True)):
        before = Slot(mode, initialized)
        for succeeded in (False, True):
            after = assign(before, succeeded)
            if after is not None:
                assignment_invariant(before, succeeded, after)
                accepted += 1
        moved = transfer_slot(before)
        if moved is not None:
            assert mode == "X" and initialized and not moved.initialized
            assert assign(moved, False).slot == moved
            assert assign(moved, True).slot == before
            transfers += 1
    mutants = (
        (Slot("I", True), True, Assignment(Slot("I", True), True, True)),
        (Slot("X", False), False, Assignment(Slot("X", True), False, True)),
        (Slot("X", False), True, Assignment(Slot("X", True), True, True)),
        (Slot("U", True), False, Assignment(Slot("U", False))),
    )
    for before, succeeded, after in mutants:
        try:
            assignment_invariant(before, succeeded, after)
        except AssertionError:
            continue
        raise AssertionError("invariant accepted an intentionally invalid assignment")
    return accepted, transfers, len(mutants)


if __name__ == "__main__":
    if not __debug__:
        raise SystemExit("Run without -O: this design check requires assertions")
    mutants = scenarios()
    attempts, accepted, stable, stable_bindings = explore()
    assignments, transfers, assignment_mutants = assignment_checks()
    print(json.dumps({"boundary_attempts": attempts, "accepted_boundary_checks": accepted,
                      "specialization_stability_checks": stable,
                      "binding_permission_stability_checks": stable_bindings,
                      "invalid_result_mutants_rejected": mutants,
                      "accepted_assignment_checks": assignments, "accepted_slot_transfers": transfers,
                      "invalid_assignment_mutants_rejected": assignment_mutants,
                      "result": "pass"}, sort_keys=True))
