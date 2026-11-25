use std::collections::{BTreeMap, btree_map::Entry};

use super::{MarkIssue, Offset};

pub(super) enum Lifecycle {
    Open,
    Draining(Offset),
    Closed,
}

impl Lifecycle {
    fn accepts(&self, offset: Offset) -> bool {
        match self {
            Self::Open => true,
            Self::Draining(cutoff) => offset <= *cutoff,
            Self::Closed => false,
        }
    }
}

pub(super) struct State {
    pub(super) lifecycle: Lifecycle,
    groups: BTreeMap<u64, Option<Group>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            lifecycle: Lifecycle::Open,
            groups: BTreeMap::new(),
        }
    }
}

pub(super) enum Input {
    Begin(Offset),
    Submit(Offset, Vec<u8>),
    Miss(String),
    Undo,
}

pub(super) enum Operation {
    Write(Vec<u8>),
    Remove,
}

struct Group {
    offset: Offset,
    phase: Phase,
}

enum Phase {
    Capturing,
    Queued(Vec<u8>),
    Writing,
    Committed,
    RemoveQueued,
    Removing,
    Undone,
    Failed(String),
}

enum Event {
    Submit(Vec<u8>),
    Miss(String),
    Undo,
    Finish(Result<(), String>),
}

impl Group {
    fn transition(&mut self, event: Event) {
        self.phase = match (&self.phase, event) {
            (Phase::Capturing, Event::Submit(png)) => Phase::Queued(png),
            (Phase::Capturing, Event::Miss(issue)) => Phase::Failed(issue),
            (Phase::Writing | Phase::Committed | Phase::Failed(_), Event::Undo) => {
                Phase::RemoveQueued
            }
            (Phase::Capturing | Phase::Queued(_), Event::Undo) => Phase::Undone,
            (Phase::Writing, Event::Finish(Ok(()))) => Phase::Committed,
            (Phase::Removing, Event::Finish(Ok(()))) => Phase::Undone,
            (Phase::Writing | Phase::Removing, Event::Finish(Err(issue))) => Phase::Failed(issue),
            _ => return,
        };
    }

    fn take_operation(&mut self, closed: bool) -> Option<Operation> {
        match &mut self.phase {
            Phase::Queued(png) if !closed => {
                let operation = Operation::Write(std::mem::take(png));
                self.phase = Phase::Writing;
                Some(operation)
            }
            Phase::RemoveQueued => {
                self.phase = Phase::Removing;
                Some(Operation::Remove)
            }
            _ => None,
        }
    }

    fn pending(&self) -> bool {
        matches!(
            self.phase,
            Phase::Capturing
                | Phase::Queued(_)
                | Phase::Writing
                | Phase::RemoveQueued
                | Phase::Removing
        )
    }
}

impl State {
    pub(super) fn input(&mut self, id: u64, input: Input) -> bool {
        if matches!(self.lifecycle, Lifecycle::Closed) {
            return false;
        }
        match (self.groups.entry(id), input) {
            (Entry::Vacant(entry), Input::Begin(offset)) if self.lifecycle.accepts(offset) => {
                entry.insert(Some(Group {
                    offset,
                    phase: Phase::Capturing,
                }));
            }
            (Entry::Vacant(entry), Input::Submit(offset, png))
                if self.lifecycle.accepts(offset) =>
            {
                entry.insert(Some(Group {
                    offset,
                    phase: Phase::Queued(png),
                }));
            }
            (Entry::Vacant(entry), Input::Undo) => {
                entry.insert(None);
            }
            (Entry::Occupied(mut entry), input) => {
                let Some(group) = entry.get_mut() else {
                    return false;
                };
                match input {
                    Input::Submit(offset, png)
                        if offset == group.offset && matches!(group.phase, Phase::Capturing) =>
                    {
                        group.transition(Event::Submit(png));
                    }
                    Input::Miss(issue) if matches!(group.phase, Phase::Capturing) => {
                        group.transition(Event::Miss(issue));
                    }
                    Input::Undo => group.transition(Event::Undo),
                    _ => return false,
                }
            }
            _ => return false,
        }
        true
    }

    pub(super) fn drain(&mut self, cutoff: Offset) {
        self.lifecycle = Lifecycle::Draining(cutoff);
        for group in self.groups.values_mut().flatten() {
            if group.offset > cutoff {
                group.transition(Event::Undo);
            }
        }
    }

    pub(super) fn pending(&self) -> bool {
        self.groups.values().flatten().any(Group::pending)
    }

    pub(super) fn close(&mut self) -> Vec<MarkIssue> {
        let issues = self
            .groups
            .iter()
            .filter_map(|(&id, group)| {
                let group = group.as_ref()?;
                let issue = match &group.phase {
                    Phase::Failed(issue) => issue.clone(),
                    _ if group.pending() => "mark drain timed out".into(),
                    _ => return None,
                };
                Some(MarkIssue { id, issue })
            })
            .collect();
        self.stop();
        issues
    }

    pub(super) fn stop(&mut self) {
        self.lifecycle = Lifecycle::Closed;
        for group in self.groups.values_mut().flatten() {
            if matches!(group.phase, Phase::Queued(_)) {
                group.transition(Event::Undo);
            }
        }
    }

    pub(super) fn take_operation(&mut self) -> Option<(u64, Offset, Operation)> {
        let closed = matches!(self.lifecycle, Lifecycle::Closed);
        self.groups.iter_mut().find_map(|(&id, group)| {
            let group = group.as_mut()?;
            Some((id, group.offset, group.take_operation(closed)?))
        })
    }

    pub(super) fn can_commit(&self, id: u64, offset: Offset) -> bool {
        self.lifecycle.accepts(offset)
            && self
                .groups
                .get(&id)
                .and_then(Option::as_ref)
                .is_some_and(|group| matches!(group.phase, Phase::Writing))
    }

    pub(super) fn finish(&mut self, id: u64, result: Result<(), String>) {
        if let Some(Some(group)) = self.groups.get_mut(&id) {
            group.transition(Event::Finish(result));
        }
    }
}
