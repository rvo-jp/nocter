#!/usr/bin/env python3
"""Finite whole-place access/init exploration, not a Nocter compiler or safety proof.

Two owning roots and two access identities; explicit end events stand for already
computed last-use/cleanup facts. No source liveness inference, path projections,
payload/storage dependencies, user destructors, or call reservations are modeled.

Historical pre-S12 exploration; its assumptions do not validate the current
value-binding source model. See README.md#model-explorations for classification
and the production conformance entry point.
"""
from collections import deque
from dataclasses import dataclass, replace
from itertools import combinations_with_replacement, product
import json

ROOTS = ("r0", "r1")
ACCESS = ("a", "b")


@dataclass(frozen=True, order=True)
class Lease:
    name: str
    parent: str
    root: str
    mode: str


@dataclass(frozen=True)
class State:
    initialized: tuple = (True, True)
    leases: tuple = ()

    def lease(self, name):
        return next((item for item in self.leases if item.name == name), None)

    def children(self, name):
        return tuple(item for item in self.leases if item.parent == name)

    def root(self, name):
        return name if name in ROOTS else self.lease(name).root

    def mode(self, name):
        return "X" if name in ROOTS else self.lease(name).mode


def validate(state):
    assert len(state.initialized) == len(ROOTS)
    assert len({item.name for item in state.leases}) == len(state.leases)
    ancestors = {}
    for item in state.leases:
        assert item.name in ACCESS and item.mode in ("I", "U")
        assert item.root in ROOTS and state.initialized[ROOTS.index(item.root)]
        seen = {item.name}
        parent = item.parent
        while parent not in ROOTS:
            assert parent not in seen, "delegation is acyclic"
            seen.add(parent)
            owner = state.lease(parent)
            assert owner is not None, "delegation parent survives"
            assert owner.root == item.root
            parent = owner.parent
        assert parent == item.root
        ancestors[item.name] = seen - {item.name}
        if item.mode == "U" and item.parent not in ROOTS:
            assert state.lease(item.parent).mode == "U", "I cannot grant U"
    for left, right in combinations_with_replacement(state.leases, 2):
        if left == right or left.root != right.root:
            continue
        related = left.name in ancestors[right.name] or right.name in ancestors[left.name]
        assert related or left.mode == right.mode == "I", "overlapping exclusive siblings conflict"


# Actions are (operation, target, new_access_name). All roots denote whole places.
def perform(state, action):
    operation, target, name = action
    if target not in ROOTS and state.lease(target) is None:
        return None
    if operation == "end":
        if target in ROOTS or state.children(target):
            return None
        return replace(state, leases=tuple(item for item in state.leases if item.name != target))
    root = state.root(target)
    initialized = state.initialized[ROOTS.index(root)]
    children = state.children(target)
    readable = initialized and not any(child.mode == "U" for child in children)
    writable = state.mode(target) in ("X", "U") and not children
    if operation == "read":
        return state if readable else None
    if operation == "write":
        if not writable or target not in ROOTS and not initialized:
            return None
        flags = list(state.initialized)
        flags[ROOTS.index(root)] = True
        return replace(state, initialized=tuple(flags))
    if operation == "move":
        if target not in ROOTS or not initialized or children:
            return None
        flags = list(state.initialized)
        flags[ROOTS.index(root)] = False
        return replace(state, initialized=tuple(flags))
    assert operation in ("delegateI", "delegateU")
    if name not in ACCESS or state.lease(name) is not None:
        return None
    if not readable or operation == "delegateU" and not writable:
        return None
    child = Lease(name, target, root, operation[-1])
    return replace(state, leases=tuple(sorted((*state.leases, child))))


def transition_invariant(before, action, after):
    validate(before)
    validate(after)
    operation, target, name = action
    root = before.root(target)
    index = ROOTS.index(root)
    descendants = set()
    pending = [target]
    while pending:
        parent = pending.pop()
        children = before.children(parent)
        descendants.update(child.name for child in children)
        pending.extend(child.name for child in children)
    if operation in ("read", "delegateI"):
        assert before.initialized[index]
        assert not any(item.name in descendants and item.mode == "U" for item in before.leases)
    elif operation in ("write", "move", "delegateU"):
        assert before.mode(target) in ("X", "U") and not descendants
        if operation != "write" or target not in ROOTS:
            assert before.initialized[index]
    if operation in ("read", "write", "move"):
        assert before.leases == after.leases
        expected = list(before.initialized)
        if operation in ("write", "move"):
            expected[index] = operation == "write"
        if operation == "move":
            assert target in ROOTS
        assert after.initialized == tuple(expected)
    elif operation.startswith("delegate"):
        assert before.initialized == after.initialized
        assert before.lease(name) is None
        assert set(after.leases) - set(before.leases) == {Lease(name, target, root, operation[-1])}
        assert set(before.leases) <= set(after.leases)
    else:
        assert operation == "end" and target not in ROOTS and not descendants
        assert before.initialized == after.initialized
        assert set(after.leases) == set(before.leases) - {before.lease(target)}


def actions():
    for target in ROOTS + ACCESS:
        for operation in ("read", "write", "move", "end"):
            yield operation, target, ""
        for mode, name in product(("I", "U"), ACCESS):
            yield "delegate" + mode, target, name


def explore():
    seen = {State()}
    pending = deque(seen)
    transitions = rejected = 0
    while pending:
        before = pending.popleft()
        validate(before)
        for action in actions():
            after = perform(before, action)
            if after is None:
                rejected += 1
                continue
            transition_invariant(before, action, after)
            transitions += 1
            if after not in seen:
                seen.add(after)
                pending.append(after)
    return seen, transitions, rejected


def sequence(state, *actions_to_apply):
    for action in actions_to_apply:
        after = perform(state, action)
        if after is None:
            return None
        transition_invariant(state, action, after)
        state = after
    return state


def loop_fixed_point(seed, body):
    """One nondeterministically repeated continuing path, including the zero-iteration seed."""
    heads = {seed}
    pending = deque(heads)
    while pending:
        before = pending.popleft()
        after = sequence(before, *body)
        if after is None:
            return heads, False
        if after not in heads:
            heads.add(after)
            pending.append(after)
    return heads, True


def scenarios(states):
    start = State()
    update = perform(start, ("delegateU", "r0", "a"))
    assert perform(update, ("read", "r0", "")) is None
    child = perform(update, ("delegateI", "a", "b"))
    assert perform(child, ("read", "a", "")) == child
    assert perform(child, ("write", "a", "")) is None
    assert perform(child, ("end", "a", "")) is None
    restored = perform(child, ("end", "b", ""))
    assert perform(restored, ("write", "a", "")) == restored
    assert perform(restored, ("move", "a", "")) is None
    root_restored = perform(restored, ("end", "a", ""))
    assert perform(root_restored, ("move", "r0", "")) is not None
    read = perform(start, ("delegateI", "r0", "a"))
    siblings = perform(read, ("delegateI", "r0", "b"))
    assert siblings is not None
    assert perform(read, ("delegateU", "a", "b")) is None
    assert perform(read, ("delegateU", "r0", "b")) is None
    assert perform(read, ("move", "r0", "")) is None
    assert perform(read, ("write", "r1", "")) == read

    # A join is universally checked. Mutually exclusive leases remain alternative worlds.
    join_checks = 0
    for left, right in combinations_with_replacement(states, 2):
        for target, operation in product(ROOTS, ("read", "write", "move")):
            outcomes = [perform(state, (operation, target, "")) for state in (left, right)]
            if all(after is not None for after in outcomes):
                for before, after in zip((left, right), outcomes):
                    transition_invariant(before, (operation, target, ""), after)
                join_checks += 1
    moved = perform(start, ("move", "r0", ""))
    assert perform(moved, ("read", "r0", "")) is None  # live predecessor cannot repair this one
    assert all(perform(state, ("write", "r0", "")) is not None for state in (start, moved))
    other_update = perform(start, ("delegateU", "r1", "a"))
    assert all(perform(state, ("write", "a", "")) is not None for state in (update, other_update))
    assert perform(other_update, ("read", "r0", "")) is not None
    assert perform(update, ("read", "r0", "")) is None

    heads, safe = loop_fixed_point(start, (("move", "r0", ""), ("write", "r0", "")))
    assert safe and heads == {start}
    heads, safe = loop_fixed_point(start, (("read", "r0", ""), ("move", "r0", "")))
    assert not safe and moved in heads  # first iteration alone would incorrectly pass
    heads, safe = loop_fixed_point(moved, (("write", "r0", ""),))
    assert safe and moved in heads and start in heads  # zero iterations still matter at exit
    # Reinitialization only on a break edge is not a continuing-path repair.
    assert perform(moved, ("write", "r0", "")) == start
    assert not loop_fixed_point(start, (("move", "r0", ""),))[1]

    invalid_states = (
        State((False, True), (Lease("a", "r0", "r0", "I"),)),
        State(leases=(Lease("a", "b", "r0", "I"),)),
        State(leases=(Lease("a", "r0", "r0", "I"), Lease("b", "a", "r0", "U"))),
        State(leases=(Lease("a", "r0", "r0", "U"), Lease("b", "r0", "r0", "I"))),
        State(leases=(Lease("a", "b", "r0", "U"), Lease("b", "a", "r0", "U"))),
    )
    for state in invalid_states:
        try:
            validate(state)
        except AssertionError:
            continue
        raise AssertionError("accepted an invalid access-graph mutant")
    invalid_transitions = (
        (read, ("write", "r0", ""), read),
        (update, ("read", "r0", ""), update),
        (moved, ("move", "r0", ""), moved),
        (start, ("move", "r0", ""), start),
        (child, ("end", "a", ""), State(leases=(child.lease("b"),))),
    )
    for before, action, after in invalid_transitions:
        try:
            transition_invariant(before, action, after)
        except AssertionError:
            continue
        raise AssertionError("accepted an invalid transition mutant")
    return join_checks, len(invalid_states), len(invalid_transitions)


if __name__ == "__main__":
    if not __debug__:
        raise SystemExit("Run without -O: this design check requires assertions")
    states, transitions, rejected = explore()
    joins, state_mutants, transition_mutants = scenarios(states)
    print(json.dumps({"states": len(states), "transitions": transitions,
                      "rejected_action_attempts": rejected, "accepted_join_operations": joins,
                      "invalid_state_mutants_rejected": state_mutants,
                      "invalid_transition_mutants_rejected": transition_mutants,
                      "loop_scenarios": 4, "result": "pass"}, sort_keys=True))
