//! Generation-fenced ownership for cross-machine interactive handoffs.
//!
//! The runtime performs transport work, but this state machine is the sole
//! authority for when endpoint-bound messages may be routed. A candidate never
//! becomes routable before commit, and reconnect metadata can only invalidate a
//! pending handoff; it cannot change focus.

use super::federation::{Generation, MachineId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AttachmentKey {
    pub machine: MachineId,
    pub generation: Generation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    Connecting,
    ValidatingView,
    ApplyingGeometry,
    Revalidating,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Active(AttachmentKey),
    Preparing {
        source: AttachmentKey,
        target: AttachmentKey,
        phase: Phase,
        source_coherent: bool,
    },
    Frozen {
        source: AttachmentKey,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Coordinator {
    state: State,
}

impl Coordinator {
    pub(super) fn new(active: AttachmentKey) -> Self {
        Self {
            state: State::Active(active),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn active(&self) -> AttachmentKey {
        match self.state {
            State::Active(active)
            | State::Preparing { source: active, .. }
            | State::Frozen { source: active } => active,
        }
    }

    /// Input, resize, selection, close, copy mode, and acknowledgements all use
    /// this same gate. Preparing and frozen states route nothing.
    pub(super) fn permits(&self, key: AttachmentKey) -> bool {
        matches!(self.state, State::Active(active) if active == key)
    }

    pub(super) fn begin(&mut self, target: AttachmentKey) -> bool {
        let State::Active(source) = self.state else {
            return false;
        };
        if source.machine == target.machine || target.generation.connection == 0 {
            return false;
        }
        self.state = State::Preparing {
            source,
            target,
            phase: Phase::Connecting,
            source_coherent: true,
        };
        true
    }

    pub(super) fn advance(&mut self, expected: Phase, next: Phase) -> bool {
        let State::Preparing { phase, .. } = &mut self.state else {
            return false;
        };
        if *phase != expected {
            return false;
        }
        *phase = next;
        true
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn source_disconnected(&mut self) {
        if let State::Preparing {
            source_coherent, ..
        } = &mut self.state
        {
            *source_coherent = false;
        }
    }

    pub(super) fn fail(&mut self) {
        let State::Preparing {
            source,
            source_coherent,
            ..
        } = self.state
        else {
            return;
        };
        self.state = if source_coherent {
            State::Active(source)
        } else {
            State::Frozen { source }
        };
    }

    pub(super) fn commit(&mut self, current: AttachmentKey) -> bool {
        let State::Preparing {
            target,
            phase: Phase::Revalidating,
            ..
        } = self.state
        else {
            return false;
        };
        if target != current {
            self.fail();
            return false;
        }
        self.state = State::Active(target);
        true
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn key(machine: MachineId, connection: u64) -> AttachmentKey {
        AttachmentKey {
            machine,
            generation: Generation {
                supervisor: 1,
                connection,
            },
        }
    }

    #[test]
    fn every_precommit_phase_freezes_all_routing_and_failure_restores_source() {
        let source = key(MachineId::Local, 1);
        let target = key(MachineId::Ssh(Uuid::new_v4()), 2);
        for failed_phase in [
            Phase::Connecting,
            Phase::ValidatingView,
            Phase::ApplyingGeometry,
            Phase::Revalidating,
        ] {
            let mut handoff = Coordinator::new(source);
            assert!(handoff.begin(target));
            for (from, to) in [
                (Phase::Connecting, Phase::ValidatingView),
                (Phase::ValidatingView, Phase::ApplyingGeometry),
                (Phase::ApplyingGeometry, Phase::Revalidating),
            ] {
                if from == failed_phase {
                    break;
                }
                assert!(handoff.advance(from, to));
            }
            assert!(!handoff.permits(source));
            assert!(!handoff.permits(target));
            handoff.fail();
            assert!(handoff.permits(source));
            assert!(!handoff.permits(target));
        }
    }

    #[test]
    fn commit_is_atomic_and_rejects_reconnected_candidate_generation() {
        let source = key(MachineId::Local, 1);
        let target = key(MachineId::Ssh(Uuid::new_v4()), 2);
        let mut handoff = Coordinator::new(source);
        assert!(handoff.begin(target));
        assert!(handoff.advance(Phase::Connecting, Phase::ValidatingView));
        assert!(handoff.advance(Phase::ValidatingView, Phase::ApplyingGeometry));
        assert!(handoff.advance(Phase::ApplyingGeometry, Phase::Revalidating));
        assert!(!handoff.commit(key(target.machine, 3)));
        assert!(handoff.permits(source));

        assert!(handoff.begin(target));
        assert!(handoff.advance(Phase::Connecting, Phase::ValidatingView));
        assert!(handoff.advance(Phase::ValidatingView, Phase::ApplyingGeometry));
        assert!(handoff.advance(Phase::ApplyingGeometry, Phase::Revalidating));
        assert!(handoff.commit(target));
        assert!(!handoff.permits(source));
        assert!(handoff.permits(target));
    }

    #[test]
    fn losing_source_and_candidate_remains_visibly_frozen() {
        let source = key(MachineId::Local, 1);
        let target = key(MachineId::Ssh(Uuid::new_v4()), 2);
        let mut handoff = Coordinator::new(source);
        assert!(handoff.begin(target));
        handoff.source_disconnected();
        handoff.fail();
        assert_eq!(handoff.active(), source);
        assert!(!handoff.permits(source));
        assert!(!handoff.permits(target));
    }
}
