#!/usr/bin/env python3
"""Bounded design exploration, not a Nocter checker or normative language oracle.

Two owned root places, two calls, at most two operands per call. No projections,
reborrows, Copy operations, reinitialization, suspension, or external effects.
See v2.0.0-value-access-semantics.md for the design and the limits of this check.

Historical pre-S12 exploration; its assumptions do not validate the current
value-binding source model. See README.md#model-explorations for classification
and the production conformance entry point.
"""

from collections import Counter, deque
from dataclasses import dataclass, replace
import json


@dataclass(frozen=True)
class Call:
    arguments: tuple = ()
    active: bool = False
    owned: tuple = ()


@dataclass(frozen=True)
class State:
    roots: tuple = ("a", "b")
    calls: tuple = (Call(), Call())
    destroyed: tuple = ()


def set_call(state, index, call):
    calls = list(state.calls)
    calls[index] = call
    return replace(state, calls=tuple(calls))


def accesses(state):
    return [(index, mode, root) for index, call in enumerate(state.calls)
            if call.active for mode, root in call.arguments if mode != "X"]


def reservations(state):
    return [(index, mode, root) for index, call in enumerate(state.calls)
            if not call.active for mode, root in call.arguments]


def invariant(state):
    owners = [token for token in state.roots if token is not None]
    owners += [token for call in state.calls for token in call.owned]
    assert Counter(owners + list(state.destroyed)) == Counter(("a", "b")), "linear ownership"
    for call in state.calls:
        assert call.active or not call.owned, "preparation does not own reserved sources"
    live = accesses(state)
    pending = reservations(state)
    for _, _, root in live + pending:
        assert state.roots[root] is not None, "access/reservation needs initialized owner"
    for index, (_, mode, root) in enumerate(live):
        for _, other_mode, other_root in live[index + 1:]:
            assert root != other_root or mode == other_mode == "I", "exclusive update"
        if mode == "U":
            assert all(other_root != root for _, _, other_root in pending), "reservation protection"


def reserve(state, index, mode, root):
    call = state.calls[index]
    if call.active or len(call.arguments) == 2 or state.roots[root] is None:
        return None
    if any(other_mode == "U" and other_root == root for _, other_mode, other_root in accesses(state)):
        return None
    return set_call(state, index, replace(call, arguments=call.arguments + ((mode, root),)))


def activate(state, index):
    call = state.calls[index]
    if call.active:
        return None
    for position, (mode, root) in enumerate(call.arguments):
        if state.roots[root] is None:
            return None
        for other_mode, other_root in call.arguments[position + 1:]:
            if root == other_root and not mode == other_mode == "I":
                return None
        for _, other_mode, other_root in accesses(state):
            if root == other_root and not mode == other_mode == "I":
                return None
        if mode != "I" and any(other != index and other_root == root
                               for other, _, other_root in reservations(state)):
            return None
    roots, owned = list(state.roots), []
    for mode, root in call.arguments:
        if mode == "X":
            owned.append(roots[root])
            roots[root] = None
    committed = set_call(state, index, replace(call, active=True, owned=tuple(owned)))
    return replace(committed, roots=tuple(roots))


def exit_call(state, index):
    """Preparation cancellation or invoked completion/failure/cancellation cleanup."""
    call = state.calls[index]
    result = set_call(state, index, Call())
    return replace(result, destroyed=tuple(sorted(state.destroyed + call.owned)))


def destroy_root(state, root):
    token = state.roots[root]
    if token is None or any(other_root == root for _, _, other_root
                            in accesses(state) + reservations(state)):
        return None
    roots = list(state.roots)
    roots[root] = None
    return replace(state, roots=tuple(roots), destroyed=tuple(sorted(state.destroyed + (token,))))


def successors(state):
    for index in range(2):
        for mode in ("I", "U", "X"):
            for root in range(2):
                yield reserve(state, index, mode, root)
        yield activate(state, index)
        yield exit_call(state, index)
    for root in range(2):
        yield destroy_root(state, root)


def scenarios():
    initial = State()
    pending = reserve(initial, 0, "X", 0)
    assert exit_call(pending, 0) == initial, "later preparation failure retains source"
    invoked = activate(pending, 0)
    assert invoked.roots[0] is None and invoked.calls[0].owned == ("a",)
    failed = exit_call(invoked, 0)
    assert failed.roots[0] is None and failed.destroyed == ("a",), "callee failure does not return ownership"
    assert activate(reserve(pending, 0, "X", 0), 0) is None, "duplicate transfer rejected"
    assert destroy_root(pending, 0) is None, "reserved source cannot be destroyed"

    outer = reserve(initial, 0, "U", 0)
    reader = activate(reserve(outer, 1, "I", 0), 1)
    assert reader is not None, "nested read during reservation allowed"
    assert activate(reader, 0) is None, "retained read blocks update activation"
    assert activate(exit_call(reader, 1), 0) is not None, "completed read permits outer activation"
    assert activate(reserve(outer, 1, "U", 0), 1) is None, "nested update blocked by reservation"
    assert activate(reserve(reserve(initial, 0, "U", 0), 0, "U", 1), 0) is not None
    assert activate(reserve(reserve(initial, 0, "U", 0), 0, "I", 0), 0) is None

    invalid_states = (
        replace(initial, roots=("a", "a")),
        replace(initial, calls=(Call((("U", 0), ("I", 0)), True), Call())),
        replace(initial, roots=(None, "b"), destroyed=("a",),
                calls=(Call((("I", 0),), True), Call())),
        replace(initial, roots=(None, "b"), destroyed=("a",),
                calls=(Call((("X", 0),)), Call())),
    )
    for state in invalid_states:
        try:
            invariant(state)
        except AssertionError:
            continue
        raise AssertionError("invariant failed to reject intentional invalid state")
    return 4


def explore():
    initial = State()
    visited, queue, transitions = {initial}, deque((initial,)), 0
    while queue:
        state = queue.popleft()
        invariant(state)
        for target in successors(state):
            if target is None:
                continue
            invariant(target)
            transitions += 1
            if target not in visited:
                visited.add(target)
                queue.append(target)
    return len(visited), transitions


if __name__ == "__main__":
    if not __debug__:
        raise SystemExit("Run without -O: this design check requires assertions")
    mutants = scenarios()
    states, edges = explore()
    print(json.dumps({"reachable_states": states, "allowed_transition_checks": edges,
                      "invalid_state_mutants_rejected": mutants, "result": "pass"}, sort_keys=True))
