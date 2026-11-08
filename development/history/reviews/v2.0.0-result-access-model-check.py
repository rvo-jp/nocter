#!/usr/bin/env python3
"""Finite post-invocation design exploration, not a Nocter checker or language oracle.

Three already-created resource tokens: outside input, local, and error information.
One constructed computation, one selected input mode, at most two output handles.
No parsing, Copy payloads, projections/ranges, reborrowing, regions, user destructors,
threads, execution effects, or proof of general type safety.

Historical pre-S12 exploration; its assumptions do not validate the current
value-binding source model. See README.md#model-explorations for classification
and the production conformance entry point.
"""

from collections import Counter, deque
from dataclasses import dataclass, replace
import json


@dataclass(frozen=True)
class Output:
    mode: str
    token: int
    dependencies: tuple = ()
    tag: str = "success"


@dataclass(frozen=True)
class State:
    input_mode: str
    owners: tuple
    retained: str | None
    phase: str = "pending"
    outputs: tuple = (None, None)


@dataclass(frozen=True)
class Edge:
    state: State
    events: tuple = ()


def initial(mode):
    return State(mode, ("callee" if mode == "X" else "caller", "callee", "callee"),
                 mode if mode != "X" else None)


def permits(authority, demand):
    return demand == "I" or authority == "X" or authority == demand


def invariant(state):
    assert state.input_mode in ("I", "U", "X")
    assert state.phase in ("pending", "success", "absent", "failure", "cancelled")
    assert len(state.owners) == 3 and len(state.outputs) == 2
    assert all(owner in ("caller", "callee", "out0", "out1", "dead") for owner in state.owners)
    owned_outputs, dependencies = [], []
    for slot, output in enumerate(state.outputs):
        if output is None:
            continue
        assert output.mode in ("I", "U", "X") and output.tag == state.phase, "branch tag preserved"
        assert 0 <= output.token < 3
        if output.mode == "X":
            owned_outputs.append(output.token)
            assert state.owners[output.token] == f"out{slot}", "output ownership location"
        else:
            assert (output.token, output.mode) in output.dependencies, "access retains its referent"
        if output.tag == "failure":
            assert output.mode == "X" and output.token == 2, "failure owns error information"
        for root, mode in output.dependencies:
            assert root == 0 and mode in ("I", "U"), "only outside roots escape this abstraction"
            assert state.owners[root] == "caller", "dependency has live outside owner"
            assert state.input_mode != "X" and permits(state.input_mode, mode), "no stronger access"
            dependencies.append((root, mode))
    assert len(owned_outputs) == len(set(owned_outputs)), "one output owner per token"
    for token, owner in enumerate(state.owners):
        if owner.startswith("out"):
            output = state.outputs[int(owner[-1])]
            assert output is not None and output.mode == "X" and output.token == token, "owner accounted for"
    for index, (root, mode) in enumerate(dependencies):
        for other_root, other_mode in dependencies[index + 1:]:
            assert root != other_root or mode == other_mode == "I", "exclusive update output"
    if state.phase == "pending":
        assert not any(state.outputs), "pending computation has no published output"
        assert state.owners[1:] == ("callee", "callee"), "constructed local tokens remain frame-owned"
        expected = None if state.input_mode == "X" else state.input_mode
        assert state.retained == expected, "construction retains input until completion/cancellation"
        if state.retained:
            assert state.owners[0] == "caller", "pending input owner alive"
    else:
        assert state.retained is None and "callee" not in state.owners, "ordinary callee cleanup finished"
    if state.phase in ("absent", "cancelled"):
        assert not any(state.outputs), "no inactive success payload"


def checked_edge(before, after, events):
    """Validate accepted transitions as well as state invariants."""
    invariant(after)
    newly_dead = [root for root, owner in enumerate(after.owners)
                  if owner == "dead" and before.owners[root] != "dead"]
    destroyed = [event[1] for event in events if event[0] == "destroy"]
    assert Counter(destroyed) == Counter(newly_dead), "exactly-once destruction events"
    for root, owner in enumerate(before.owners):
        assert owner != "dead" or after.owners[root] == "dead", "no resource resurrection"
    for event in events:
        if event[0] in ("forward", "duplicate-read"):
            source, destination = event[1:3]
            old_output = before.outputs[source]
            assert old_output == after.outputs[destination], "forwarding preserves mode, tag, and dependencies"
            if event[0] == "duplicate-read":
                assert old_output.mode == "I" and after.outputs[source] == old_output, "only read access duplicates"
            else:
                assert after.outputs[source] is None, "forwarding moves the handle"
    # A dependent owned output must be destroyed before releasing its dependency.
    for slot, output in enumerate(before.outputs):
        if output is None or output.mode != "X" or after.owners[output.token] != "dead":
            continue
        destroy_at = events.index(("destroy", output.token))
        for root, mode in output.dependencies:
            assert destroy_at < events.index(("release", slot, root, mode)), "output cleanup before release"
    return Edge(after, tuple(events))


def settle(state, phase, output=None):
    if state.phase != "pending":
        return None
    owners, events = list(state.owners), []
    if output and output.mode == "X":
        if owners[output.token] != "callee":
            return None
        owners[output.token] = "out0"
    if output:
        for root, mode in output.dependencies:
            if (root != 0 or state.retained is None or not permits(state.retained, mode)
                    or owners[root] != "caller"):
                return None
    for root in reversed(range(3)):
        if owners[root] == "callee":
            owners[root] = "dead"
            events.append(("destroy", root))
    if state.retained:
        # Handoff is atomic; it does not release/reacquire an escaping access.
        operation = "handoff" if output and output.dependencies else "release-input"
        events.append((operation, 0, state.retained))
    after = replace(state, owners=tuple(owners), retained=None, phase=phase,
                    outputs=(output, None))
    return checked_edge(state, after, events)


def success(state, mode, root, dependency=None):
    if state.phase != "pending":
        return None
    if mode == "X":
        dependencies = () if dependency is None else ((0, dependency),)
        return settle(state, "success", Output(mode, root, dependencies))
    # A local or consumed-input referent is not eligible after ordinary exit cleanup.
    if root != 0 or state.owners[root] != "caller" or not permits(state.input_mode, mode):
        return None
    return settle(state, "success", Output(mode, root, ((root, mode),)))


def forward(state, source, destination):
    output = state.outputs[source]
    if output is None or state.outputs[destination] is not None:
        return None
    outputs, owners = list(state.outputs), list(state.owners)
    outputs[source], outputs[destination] = None, output
    if output.mode == "X":
        owners[output.token] = f"out{destination}"
    return checked_edge(state, replace(state, outputs=tuple(outputs), owners=tuple(owners)),
                        (("forward", source, destination, output.tag),))


def duplicate_read(state, source, destination):
    output = state.outputs[source]
    if output is None or output.mode != "I" or state.outputs[destination] is not None:
        return None
    outputs = list(state.outputs)
    outputs[destination] = output
    return checked_edge(state, replace(state, outputs=tuple(outputs)), (("duplicate-read", source, destination),))


def drop_output(state, slot):
    output = state.outputs[slot]
    if output is None:
        return None
    outputs, owners, events = list(state.outputs), list(state.owners), []
    outputs[slot] = None
    if output.mode == "X":
        owners[output.token] = "dead"
        events.append(("destroy", output.token))
    events.extend(("release", slot, root, mode) for root, mode in output.dependencies)
    return checked_edge(state, replace(state, outputs=tuple(outputs), owners=tuple(owners)), events)


def destroy_outside(state):
    if state.owners[0] != "caller" or state.retained is not None:
        return None
    if any(output and any(root == 0 for root, _ in output.dependencies) for output in state.outputs):
        return None
    return checked_edge(state, replace(state, owners=("dead",) + state.owners[1:]), (("destroy", 0),))


def successors(state):
    if state.phase == "pending":
        for mode in ("I", "U", "X"):
            for root in range(3):
                yield success(state, mode, root)
                if mode == "X":
                    for dependency in ("I", "U"):
                        yield success(state, mode, root, dependency)
        yield settle(state, "failure", Output("X", 2, tag="failure"))
        yield settle(state, "absent")
        yield settle(state, "cancelled")
    for source, destination in ((0, 1), (1, 0)):
        yield forward(state, source, destination)
        yield duplicate_read(state, source, destination)
        yield drop_output(state, source)
    yield destroy_outside(state)


def scenarios():
    reader = initial("I")
    assert destroy_outside(reader) is None, "dependency starts at constructed-computation boundary"
    result = success(reader, "I", 0).state
    forwarded = forward(result, 0, 1).state
    copied = duplicate_read(forwarded, 1, 0).state
    assert destroy_outside(drop_output(copied, 0).state) is None, "remaining read copy retains owner"
    released = drop_output(drop_output(copied, 0).state, 1).state
    assert destroy_outside(released) is not None
    assert success(reader, "U", 0) is None, "I cannot manufacture U"
    assert success(reader, "X", 0) is None, "I cannot take outside ownership"
    assert success(reader, "I", 1) is None, "local access cannot escape"

    updater = initial("U")
    update = success(updater, "U", 0).state
    assert duplicate_read(update, 0, 1) is None, "U cannot be duplicated"
    assert destroy_outside(update) is None
    failed = settle(updater, "failure", Output("X", 2, tag="failure")).state
    assert failed.owners == ("caller", "dead", "out0") and not failed.outputs[0].dependencies
    absent = settle(updater, "absent").state
    assert destroy_outside(absent) is not None, "known absence releases success dependency"

    dependent = success(reader, "X", 1, "I").state
    discarded = drop_output(dependent, 0)
    assert discarded.events.index(("destroy", 1)) < discarded.events.index(("release", 0, 0, "I"))
    assert destroy_outside(dependent) is None, "owned output can still depend on outside input"
    cancelled = settle(reader, "cancelled").state
    assert destroy_outside(cancelled) is not None
    consumed = initial("X")
    assert success(consumed, "I", 0) is None, "consumed input is not outside storage"
    owned = success(consumed, "X", 0).state
    assert owned.owners[0] == "out0" and drop_output(owned, 0).state.owners[0] == "dead"
    error_as_data = success(reader, "X", 2).state
    assert forward(error_as_data, 0, 1).state.outputs[1].tag == "success", "error token alone does not select failure"

    owned_local = success(reader, "X", 1).state
    mutants = (
        replace(reader, retained=None),
        replace(result, owners=("dead", "dead", "dead")),
        replace(update, outputs=(update.outputs[0], update.outputs[0])),
        replace(owned_local, outputs=(owned_local.outputs[0], owned_local.outputs[0])),
        replace(failed, outputs=(replace(failed.outputs[0], tag="success"), None)),
        replace(result, outputs=(Output("U", 0, ((0, "U"),)), None)),
    )
    for mutant in mutants:
        try:
            invariant(mutant)
        except AssertionError:
            continue
        raise AssertionError("invariant accepted an intentionally invalid state")
    forwarded_dependent = forward(dependent, 0, 1)
    laundered = replace(forwarded_dependent.state,
                        outputs=(None, replace(forwarded_dependent.state.outputs[1], dependencies=())))
    invalid_edges = (
        (dependent, discarded.state, tuple(reversed(discarded.events))),
        (dependent, laundered, forwarded_dependent.events),
        (dependent, discarded.state, ()),
    )
    for before, after, events in invalid_edges:
        try:
            checked_edge(before, after, events)
        except AssertionError:
            continue
        raise AssertionError("transition checker accepted an intentionally invalid edge")
    return len(mutants), len(invalid_edges)


def explore():
    starts = tuple(initial(mode) for mode in ("I", "U", "X"))
    visited, queue, transitions = set(starts), deque(starts), 0
    while queue:
        state = queue.popleft()
        invariant(state)
        for edge in successors(state):
            if edge is None:
                continue
            transitions += 1
            if edge.state not in visited:
                visited.add(edge.state)
                queue.append(edge.state)
    return len(visited), transitions


if __name__ == "__main__":
    if not __debug__:
        raise SystemExit("Run without -O: this design check requires assertions")
    invalid_states, invalid_transitions = scenarios()
    states, transitions = explore()
    print(json.dumps({"reachable_states": states, "allowed_transition_checks": transitions,
                      "invalid_state_mutants_rejected": invalid_states,
                      "invalid_transition_mutants_rejected": invalid_transitions,
                      "result": "pass"}, sort_keys=True))
