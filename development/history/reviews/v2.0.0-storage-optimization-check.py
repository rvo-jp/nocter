#!/usr/bin/env python3
"""Finite storage-plan/trace exploration, not a Nocter compiler or general proof.

Assumes checked storage classifications, finite scalar inputs, and explicit source cleanup.
Does not infer real library summaries, model arbitrary callbacks, or measure performance.

Research-only assumed-certificate experiment, not implemented compiler analysis.
See README.md#model-explorations for its scope and production-test entry points.
"""
from dataclasses import dataclass, replace
from itertools import product
import json


@dataclass(frozen=True)
class Storage:
    implementation_only: bool
    exact_failure: bool
    observer: bool
    budget: bool
    concrete_boundary: bool
    origin: str


def storage_trace(case, invalid, fail, *, materialize=True, keep_validation=True,
                  keep_drop=True, keep_origin=True):
    events = [("before",)]
    if invalid and keep_validation:
        return tuple(events + [("error", "invalid_layout")])
    if materialize:
        if not case.implementation_only:
            events.append(("resource_request",))
        if case.observer:
            events.append(("allocator_callback",))
        if fail:
            outcome = "abort" if case.implementation_only else "error"
            return tuple(events + [(outcome, "out_of_memory")])
    if case.budget:
        events.append(("remaining_budget", 1 if materialize else 2))
    if case.concrete_boundary:
        events.append(("foreign_storage", "valid" if materialize else "missing"))
    if keep_drop:
        events.append(("user_drop",))
    # Origin is proof metadata, not a source-level introspection operation.
    events.append(("result", 42, case.origin if keep_origin else "erased"))
    return tuple(events)


def source_observations(case, invalid, fail):
    observed = {storage_trace(case, invalid, fail)}
    # Only unexposed implementation backing may vary with the chosen representation.
    # Exact requests, callbacks, and budgets are tied to this environment's outcome.
    if case.implementation_only and not (case.exact_failure or case.observer or case.budget):
        observed.update(storage_trace(case, invalid, choice) for choice in (False, True))
    return observed


def elide_eligible(case):
    return case.implementation_only and not any((case.exact_failure, case.observer,
                                                case.budget, case.concrete_boundary))


def storage_checks():
    classes = accepted = rejected = 0
    for flags in product((False, True), repeat=5):
        for origin in ("outside_A", "outside_B"):
            case = Storage(*flags, origin)
            classes += 1
            counterexample = False
            for invalid, fail in product((False, True), repeat=2):
                candidate = storage_trace(case, invalid, False, materialize=False)
                allowed = source_observations(case, invalid, fail)
                counterexample |= candidate not in allowed
                if elide_eligible(case):
                    assert candidate in allowed, "storage elision changed a source observation or origin"
                    accepted += 1
            if not elide_eligible(case):
                assert counterexample, "this finite profile needs a rejection witness"
                rejected += 1
    base = Storage(True, False, False, False, False, "outside_A")
    mutants = [
        (replace(base, implementation_only=False), False, False, {}),
        (replace(base, exact_failure=True), False, True, {}),
        (replace(base, observer=True), False, False, {}),
        (replace(base, budget=True), False, False, {}),
        (replace(base, concrete_boundary=True), False, False, {}),
        (base, False, False, {"keep_origin": False}),
        (base, False, False, {"keep_drop": False}),
        (base, True, False, {"keep_validation": False}),
    ]
    for case, invalid, fail, changes in mutants:
        candidate = storage_trace(case, invalid, False, materialize=False, **changes)
        assert candidate not in source_observations(case, invalid, fail), "invalid elision mutant survived"
    return classes, accepted, rejected, len(mutants)


@dataclass(frozen=True)
class Reuse:
    old_access_live: bool
    cleanup_reads_bytes: bool
    cleanup_done: bool
    same_domain: bool
    address_live: bool
    capacity: int
    requested: int
    alignment: int
    required_alignment: int


def reuse_eligible(case):
    return (not case.old_access_live and not case.address_live
            and (case.cleanup_done or not case.cleanup_reads_bytes)
            and case.same_domain and case.capacity >= case.requested
            and case.alignment % case.required_alignment == 0)


def reuse_trace(case, reuse):
    if reuse and case.capacity < case.requested:
        return (("invalid", "capacity"),)
    if reuse and case.alignment % case.required_alignment:
        return (("invalid", "alignment"),)
    old_bytes = 7 if reuse else 11
    events = [("new_value", 7)]
    if case.old_access_live:
        events.append(("old_read", old_bytes))
    if case.address_live:
        events.append(("foreign_read", old_bytes))
    if not case.cleanup_done:
        events.append(("old_drop", old_bytes if case.cleanup_reads_bytes else "no_byte_read"))
    events.append(("allocator_affinity", "new" if case.same_domain or not reuse else "old"))
    return tuple(events)


def reuse_checks():
    cases = accepted = rejected = 0
    for flags in product((False, True), repeat=5):
        for capacity, requested, alignment, required in product((1, 2), (1, 2, 3), (1, 2), (1, 2)):
            case = Reuse(*flags, capacity, requested, alignment, required)
            cases += 1
            equal = reuse_trace(case, False) == reuse_trace(case, True)
            if reuse_eligible(case):
                assert equal, "reuse changed an access, cleanup, origin, or storage requirement"
                accepted += 1
            else:
                assert not equal, "the rejected simple-overwrite plan needs a finite witness"
                rejected += 1
    return cases, accepted, rejected


MODES = ("pure", "log", "fail")


def produce(events, mode, index, value):
    if mode == "log":
        events.append(("producer", index))
    if mode == "fail" and value < 0:
        return None
    return 2 * value + 1


def consume(events, mode, index, value):
    if mode == "log":
        events.append(("consumer", index))
    return not (mode == "fail" and value == 1)


def cleanup(events, indices, resource):
    if resource:
        events.extend(("drop", index) for index in indices)


def pipeline(values, producer, consumer, resource, *, fused):
    events = []
    staged = []
    total = 0
    for index, value in enumerate(values):
        answer = produce(events, producer, index, value)
        if answer is None:
            cleanup(events, (i for i, _ in reversed(staged)), resource)
            return tuple(events + [("error", "producer", index)])
        if fused:
            if not consume(events, consumer, index, answer):
                cleanup(events, (index,), resource)
                return tuple(events + [("error", "consumer", index)])
            total += answer
            cleanup(events, (index,), resource)
        else:
            staged.append((index, answer))
    if not fused:
        for index, answer in staged:
            if not consume(events, consumer, index, answer):
                cleanup(events, (i for i, _ in reversed(staged)), resource)
                return tuple(events + [("error", "consumer", index)])
            total += answer
        # This source program explicitly schedules reverse logical cleanup.
        # It is not an assertion about Nocter's implicit collection drop order.
        cleanup(events, (i for i, _ in reversed(staged)), resource)
    return tuple(events + [("result", total)])


def fusion_eligible(producer, consumer, resource):
    # Sufficient certificate for this simple streaming plan, not every possible fusion.
    return not resource and (producer == "pure" or consumer == "pure")


def fusion_checks():
    inputs = [values for length in range(4) for values in product((-1, 0, 2), repeat=length)]
    cases = accepted = rejected_schemas = 0
    for producer, consumer, resource in product(MODES, MODES, (False, True)):
        witness = None
        for values in inputs:
            cases += 1
            before = pipeline(values, producer, consumer, resource, fused=False)
            after = pipeline(values, producer, consumer, resource, fused=True)
            if before != after:
                witness = values
            if fusion_eligible(producer, consumer, resource):
                assert before == after, "fusion changed callbacks, first failure, cleanup, or result"
                accepted += 1
        if not fusion_eligible(producer, consumer, resource):
            assert witness is not None, "unconditional fusion needs a rejection witness"
            rejected_schemas += 1
    # Both callbacks have no I/O here, yet failure order makes fusion invalid.
    assert pipeline((0, -1), "fail", "fail", False, fused=False)[-1] == ("error", "producer", 1)
    assert pipeline((0, -1), "fail", "fail", False, fused=True)[-1] == ("error", "consumer", 0)
    return cases, accepted, rejected_schemas


if __name__ == "__main__":
    if not __debug__:
        raise SystemExit("Run without -O: this exploration requires assertions")
    storage, elisions, elision_rejections, mutants = storage_checks()
    reuse, reuses, reuse_rejections = reuse_checks()
    fusion, fusions, fusion_rejections = fusion_checks()
    print(json.dumps({
        "storage_classes": storage, "accepted_elision_observations": elisions,
        "unsafe_elision_schemas_rejected": elision_rejections,
        "invalid_elision_mutants_rejected": mutants,
        "reuse_cases": reuse, "accepted_reuse_cases": reuses,
        "unsafe_reuse_cases_rejected": reuse_rejections,
        "fusion_trace_pairs": fusion, "accepted_fusion_pairs": fusions,
        "unsafe_fusion_schemas_rejected": fusion_rejections,
        "result": "pass",
    }, sort_keys=True))
