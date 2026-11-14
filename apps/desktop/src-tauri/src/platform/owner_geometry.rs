use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ReadVersion {
    epoch: u64,
    revision: u64,
    user_generation: u64,
    live: bool,
    closed: bool,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct FitOperation {
    pub(super) sequence: u64,
    pub(super) before: OwnerGeometryStamp,
    pub(super) target: PhysicalGeometry,
}
pub(super) struct OwnerLifecycle {
    pub(super) stamp: OwnerGeometryStamp,
    pub(super) in_flight: Option<FitOperation>,
    // Completed sequence numbers are bounded diagnostics, never a whitelist
    // of historical dimensions. Causality uses only current actual/in-flight.
    pub(super) settled: VecDeque<u64>,
    pub(super) live_resize: bool,
    pub(super) closed: bool,
}
#[derive(Default)]
pub(super) struct GeometryLifecycle {
    pub(super) owners: BTreeMap<String, OwnerLifecycle>,
    pub(super) sequence: u64,
}
impl GeometryLifecycle {
    pub(super) fn read_version(&self, owner: &str) -> Option<ReadVersion> {
        self.owners.get(owner).map(|state| ReadVersion {
            epoch: state.stamp.epoch,
            revision: state.stamp.revision,
            user_generation: state.stamp.user_generation,
            live: state.live_resize,
            closed: state.closed,
        })
    }
    pub(super) fn accept_read(
        &mut self,
        owner: &str,
        version: Option<ReadVersion>,
        actual: PhysicalGeometry,
    ) -> AppResult<OwnerGeometryDecision> {
        if self.read_version(owner) != version {
            return Err(AppError::new(
                "geometry_snapshot_changed",
                "desktop",
                "窗口几何已更新，请重新读取",
            ));
        }
        self.observe(owner, actual)?;
        Ok(self.classify(owner, Some(actual), None))
    }
    pub(super) fn native_edge(
        &mut self,
        epoch: u64,
        edge: i32,
    ) -> Option<(String, Option<OwnerInvalidation>)> {
        let owner = self
            .owners
            .iter()
            .find(|(_, state)| state.stamp.epoch == epoch)
            .map(|(owner, _)| owner.clone())?;
        let invalidation = if edge == 1 || edge == 3 {
            self.invalidate(
                &owner,
                if edge == 1 {
                    "user_live_resize"
                } else {
                    "native_owner_closed"
                },
            )
        } else {
            None
        };
        if let Some(state) = self.owners.get_mut(&owner) {
            state.live_resize = edge == 1;
            if edge == 3 {
                state.closed = true;
            }
        }
        Some((owner, invalidation))
    }
    pub(super) fn observe(
        &mut self,
        owner: &str,
        actual: PhysicalGeometry,
    ) -> AppResult<OwnerGeometryStamp> {
        if !actual.valid() {
            return Err(error());
        }
        let state = self
            .owners
            .entry(owner.into())
            .or_insert_with(|| OwnerLifecycle {
                stamp: OwnerGeometryStamp {
                    epoch: actual.epoch,
                    user_generation: 0,
                    revision: 0,
                    actual,
                },
                in_flight: None,
                settled: VecDeque::new(),
                live_resize: actual.live_resize != 0,
                closed: false,
            });
        if actual.epoch < state.stamp.epoch {
            return Err(AppError::new(
                "stale_owner_window",
                "desktop",
                "原文窗口实例已更换",
            ));
        }
        if state.stamp.epoch != actual.epoch {
            *state = OwnerLifecycle {
                stamp: OwnerGeometryStamp {
                    epoch: actual.epoch,
                    user_generation: 0,
                    revision: 0,
                    actual,
                },
                in_flight: None,
                settled: VecDeque::new(),
                live_resize: actual.live_resize != 0,
                closed: false,
            };
        }
        if state.closed {
            return Err(error());
        }
        Ok(state.stamp)
    }
    pub(super) fn invalidate(
        &mut self,
        owner: &str,
        reason: &'static str,
    ) -> Option<OwnerInvalidation> {
        let state = self.owners.get_mut(owner)?;
        let boundary = OwnerInvalidation {
            owner: owner.into(),
            epoch: state.stamp.epoch,
            through_generation: state.stamp.user_generation,
            reason,
        };
        state.stamp.user_generation = state.stamp.user_generation.saturating_add(1);
        state.stamp.revision = state.stamp.revision.saturating_add(1);
        state.in_flight = None;
        state.settled.clear();
        Some(boundary)
    }
    pub(super) fn reserve(
        &mut self,
        owner: &str,
        before: PhysicalGeometry,
        height: u32,
    ) -> AppResult<Option<FitOperation>> {
        self.observe(owner, before)?;
        let state = self.owners.get_mut(owner).ok_or_else(error)?;
        if state.closed
            || state.live_resize
            || before.live_resize != 0
            || state.in_flight.is_some()
            || !state.stamp.actual.same_bounds(before)
        {
            return Err(error());
        }
        if before.height == height {
            return Ok(None);
        }
        self.sequence = self.sequence.checked_add(1).ok_or_else(error)?;
        let operation = FitOperation {
            sequence: self.sequence,
            before: state.stamp,
            target: PhysicalGeometry { height, ..before },
        };
        state.in_flight = Some(operation);
        Ok(Some(operation))
    }
    pub(super) fn settle(
        &mut self,
        owner: &str,
        operation: FitOperation,
        after: Option<PhysicalGeometry>,
    ) -> bool {
        let Some(state) = self.owners.get_mut(owner) else {
            return false;
        };
        let Some(after) = after else {
            return false;
        };
        let valid = !state.closed
            && !state.live_resize
            && state.stamp.epoch == operation.before.epoch
            && state.stamp.user_generation == operation.before.user_generation
            && state
                .in_flight
                .is_some_and(|pending| pending.sequence == operation.sequence)
            && after.live_resize == 0
            && after.same_bounds(operation.target);
        if !valid {
            return false;
        }
        state.in_flight = None;
        state.stamp.actual = after;
        state.stamp.revision = state.stamp.revision.saturating_add(1);
        if state.settled.len() == 32 {
            state.settled.pop_front();
        }
        state.settled.push_back(operation.sequence);
        true
    }
    pub(super) fn classify(
        &mut self,
        owner: &str,
        actual: Option<PhysicalGeometry>,
        forced: Option<&'static str>,
    ) -> OwnerGeometryDecision {
        if let (Some(state), Some(actual)) = (self.owners.get(owner), actual)
            && actual.epoch != state.stamp.epoch
        {
            return OwnerGeometryDecision {
                invalidation: None,
                classification: "known_stale_window",
            };
        }
        if let Some(reason) = forced {
            return OwnerGeometryDecision {
                invalidation: self.invalidate(owner, reason),
                classification: reason,
            };
        }
        let Some(state) = self.owners.get(owner) else {
            return OwnerGeometryDecision {
                invalidation: None,
                classification: "untracked_owner",
            };
        };
        if !state.live_resize && actual.is_some_and(|actual| actual.live_resize != 0) {
            let invalidation = self.invalidate(owner, "untracked_user_live_resize");
            if let Some(state) = self.owners.get_mut(owner) {
                state.live_resize = true;
                if let Some(actual) = actual {
                    state.stamp.actual = actual;
                }
            }
            return OwnerGeometryDecision {
                invalidation,
                classification: "untracked_user_live_resize",
            };
        }
        if state.live_resize {
            // One real AppKit gesture has already revoked the old card at
            // its begin edge. Intermediate resize events do not invent new
            // gestures and invalidate the latest deferred content extent.
            if let Some(actual) = actual.filter(|actual| actual.valid())
                && let Some(state) = self.owners.get_mut(owner)
            {
                if !state.stamp.actual.same_bounds(actual) {
                    state.stamp.revision = state.stamp.revision.saturating_add(1);
                }
                state.stamp.actual = actual;
            }
            return OwnerGeometryDecision {
                invalidation: None,
                classification: "user_live_resize_in_progress",
            };
        }
        let classification = actual.filter(|v| v.valid()).and_then(|actual| {
            if state.closed
                || state.live_resize
                || actual.live_resize != 0
                || actual.epoch != state.stamp.epoch
            {
                return None;
            }
            if actual.same_bounds(state.stamp.actual) {
                return Some("known_no_effect");
            }
            if state.in_flight.is_some_and(|operation| {
                operation.before.user_generation == state.stamp.user_generation
                    && actual.same_bounds(operation.target)
            }) {
                return Some("recognized_content_fit");
            }
            None
        });
        if let Some(classification) = classification {
            return OwnerGeometryDecision {
                invalidation: None,
                classification,
            };
        }
        let invalidation = self.invalidate(owner, "unknown_owner_geometry");
        if let (Some(state), Some(actual)) =
            (self.owners.get_mut(owner), actual.filter(|v| v.valid()))
            && state.stamp.epoch == actual.epoch
        {
            state.stamp.actual = actual;
        }
        OwnerGeometryDecision {
            invalidation,
            classification: "unknown_owner_geometry",
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn geometry(height: u32) -> PhysicalGeometry {
        PhysicalGeometry {
            epoch: 7,
            x: 120.,
            y: 900.,
            width: 1120,
            height,
            scale: 2.,
            live_resize: 0,
        }
    }
    fn state() -> GeometryLifecycle {
        let mut state = GeometryLifecycle::default();
        state.observe("hud", geometry(560)).unwrap();
        state
    }
    fn finish(state: &mut GeometryLifecycle, height: u32) -> FitOperation {
        let before = state.owners["hud"].stamp.actual;
        let operation = state.reserve("hud", before, height).unwrap().unwrap();
        assert!(state.settle("hud", operation, Some(operation.target)));
        operation
    }
    #[test]
    fn fit_abc_delayed_merged_reordered_and_duplicate_events_use_current_actual() {
        let mut state = state();
        let a = finish(&mut state, 600);
        let b = finish(&mut state, 700);
        let c = finish(&mut state, 500);
        // Native event payloads can be old or merged. They are not authority
        // to replace the current OS readback with a historical target.
        for _payload in [b.target, a.target, c.target, c.target, a.target] {
            let actual = state.owners["hud"].stamp.actual;
            let result = state.classify("hud", Some(actual), None);
            assert!(result.invalidation.is_none());
            assert_eq!(result.classification, "known_no_effect");
        }
        assert_eq!(state.owners["hud"].stamp.user_generation, 0);
        // Physically returning to an old target is a new, unknown change.
        assert!(
            state
                .classify("hud", Some(a.target), None)
                .invalidation
                .is_some()
        );
    }
    #[test]
    fn reentrant_target_event_is_recognized_before_settle_without_locking_native() {
        let shared = Mutex::new(state());
        let operation = {
            let mut state = shared.lock().unwrap();
            state.reserve("hud", geometry(560), 640).unwrap().unwrap()
        };
        // The production native effect is outside this lock. Reentry can
        // acquire it and classify the in-flight target, rather than hang.
        let mut callback = shared
            .try_lock()
            .expect("native call must not hold lifecycle mutex");
        assert_eq!(
            callback
                .classify("hud", Some(operation.target), None)
                .classification,
            "recognized_content_fit"
        );
        assert!(callback.settle("hud", operation, Some(operation.target)));
    }
    #[test]
    fn no_op_registers_no_allowance_and_no_event_is_needed_for_completion() {
        let mut state = state();
        assert!(state.reserve("hud", geometry(560), 560).unwrap().is_none());
        assert!(state.owners["hud"].in_flight.is_none());
        let operation = state.reserve("hud", geometry(560), 641).unwrap().unwrap();
        assert!(state.settle("hud", operation, Some(operation.target)));
        assert!(state.owners["hud"].in_flight.is_none());
        assert!(
            state
                .classify("hud", Some(geometry(643)), None)
                .invalidation
                .is_some()
        );
    }
    #[test]
    fn failed_readback_partial_application_and_changed_width_or_scale_never_settle() {
        for actual in [
            None,
            Some(geometry(561)),
            Some(PhysicalGeometry {
                width: 1100,
                ..geometry(640)
            }),
            Some(PhysicalGeometry {
                scale: 1.,
                ..geometry(640)
            }),
            Some(PhysicalGeometry {
                x: 121.,
                ..geometry(640)
            }),
        ] {
            let mut state = state();
            let operation = state.reserve("hud", geometry(560), 640).unwrap().unwrap();
            assert!(!state.settle("hud", operation, actual));
            let invalidation = state
                .invalidate("hud", "fit_apply_or_readback_failed")
                .unwrap();
            assert_eq!(invalidation.through_generation, 0);
            assert!(state.owners["hud"].in_flight.is_none());
            assert_eq!(state.owners["hud"].stamp.user_generation, 1);
        }
    }
    #[test]
    fn live_resize_invalidates_even_when_user_hits_current_or_historical_fit_size() {
        let mut state = state();
        let old = finish(&mut state, 640);
        let pending = state.reserve("hud", old.target, 700).unwrap().unwrap();
        let (_, boundary) = state.native_edge(7, 1).unwrap();
        assert_eq!(boundary.unwrap().through_generation, 0);
        assert!(state.owners["hud"].in_flight.is_none());
        assert!(!state.settle("hud", pending, Some(pending.target)));
        assert!(state.reserve("hud", old.target, 700).is_err());
        state.native_edge(7, 2).unwrap();
        assert!(
            state
                .classify("hud", Some(old.target), None)
                .invalidation
                .is_none()
        );
        assert_eq!(state.owners["hud"].stamp.user_generation, 1);
    }
    #[test]
    fn same_label_new_instance_rejects_old_completion_edge_and_close_event() {
        let mut state = state();
        let operation = state.reserve("hud", geometry(560), 640).unwrap().unwrap();
        let replacement = PhysicalGeometry {
            epoch: 8,
            ..geometry(560)
        };
        state.observe("hud", replacement).unwrap();
        assert!(!state.settle("hud", operation, Some(operation.target)));
        assert!(state.native_edge(7, 3).is_none());
        assert!(
            state
                .classify("hud", Some(operation.target), Some("owner_destroyed"))
                .invalidation
                .is_none()
        );
        assert_eq!(state.owners["hud"].stamp.epoch, 8);
        assert!(!state.owners["hud"].closed);
        assert!(state.observe("hud", geometry(560)).is_err());
        assert_eq!(state.owners["hud"].stamp.epoch, 8);
    }
    #[test]
    fn one_live_gesture_preserves_latest_fit_generation_until_its_end() {
        let mut state = state();
        state.native_edge(7, 1).unwrap();
        let requested_generation = state.owners["hud"].stamp.user_generation;
        for height in [610, 620, 650, 630] {
            let actual = PhysicalGeometry {
                live_resize: 1,
                ..geometry(height)
            };
            assert_eq!(
                state.classify("hud", Some(actual), None).classification,
                "user_live_resize_in_progress"
            );
            assert_eq!(
                state.owners["hud"].stamp.user_generation,
                requested_generation
            );
        }
        state.native_edge(7, 2).unwrap();
        let before = PhysicalGeometry {
            live_resize: 0,
            ..geometry(630)
        };
        assert!(
            state
                .classify("hud", Some(before), None)
                .invalidation
                .is_none()
        );
        assert_eq!(
            state.owners["hud"].stamp.user_generation,
            requested_generation
        );
        let latest = state.reserve("hud", before, 710).unwrap().unwrap();
        assert!(state.settle("hud", latest, Some(latest.target)));
        state.native_edge(7, 1).unwrap();
        assert_ne!(
            state.owners["hud"].stamp.user_generation,
            requested_generation
        );
    }
    #[test]
    fn an_untracked_native_live_flag_revokes_once_before_treating_intermediate_events_as_known() {
        let mut state = state();
        let actual = PhysicalGeometry {
            live_resize: 1,
            ..geometry(620)
        };
        let first = state.classify("hud", Some(actual), None);
        assert_eq!(first.classification, "untracked_user_live_resize");
        assert_eq!(first.invalidation.unwrap().through_generation, 0);
        assert_eq!(state.owners["hud"].stamp.user_generation, 1);
        assert_eq!(
            state
                .classify(
                    "hud",
                    Some(PhysicalGeometry {
                        height: 640,
                        ..actual
                    }),
                    None
                )
                .classification,
            "user_live_resize_in_progress"
        );
        assert_eq!(state.owners["hud"].stamp.user_generation, 1);
    }
    #[test]
    fn explicit_user_actions_revoke_pending_fit_without_granting_old_targets() {
        for reason in [
            "begin_user_drag",
            "reset_position",
            "restore_position",
            "width_or_compact_changed",
            "owner_scale_changed",
        ] {
            let mut state = state();
            let operation = state.reserve("hud", geometry(560), 640).unwrap().unwrap();
            assert!(
                state
                    .classify("hud", Some(geometry(560)), Some(reason))
                    .invalidation
                    .is_some()
            );
            assert!(!state.settle("hud", operation, Some(operation.target)));
            assert!(
                state
                    .classify("hud", Some(operation.target), None)
                    .invalidation
                    .is_some()
            );
        }
    }
    #[test]
    fn reservation_capacity_is_one_and_terminal_history_is_bounded_not_a_size_whitelist() {
        let mut state = state();
        state.reserve("hud", geometry(560), 640).unwrap();
        assert!(state.reserve("hud", geometry(560), 700).is_err());
        state.invalidate("hud", "fit_reservation_failed").unwrap();
        for height in 600..750 {
            finish(&mut state, height);
        }
        assert_eq!(state.owners["hud"].settled.len(), 32);
        assert!(state.owners["hud"].in_flight.is_none());
        assert!(
            state
                .classify("hud", Some(geometry(600)), None)
                .invalidation
                .is_some()
        );
    }
    #[test]
    fn backing_pixel_rounding_and_unknown_reads_are_not_logical_height_tolerances() {
        let mut state = state();
        let operation = state.reserve("hud", geometry(560), 641).unwrap().unwrap();
        assert!(!state.settle("hud", operation, Some(geometry(640))));
        assert!(state.settle("hud", operation, Some(geometry(641))));
        assert!(state.reserve("hud", geometry(641), 641).unwrap().is_none());
        assert!(state.classify("hud", None, None).invalidation.is_some());
    }
    #[test]
    fn owner_close_terminates_operation_and_never_reopens_the_same_native_instance() {
        let mut state = state();
        let operation = state.reserve("hud", geometry(560), 640).unwrap().unwrap();
        state.native_edge(7, 3).unwrap();
        assert!(!state.settle("hud", operation, Some(operation.target)));
        assert!(state.observe("hud", geometry(560)).is_err());
    }
    #[test]
    fn stale_physical_read_after_a_same_epoch_fit_cannot_invalidate_the_confirmed_card() {
        let mut state = state();
        let before_read = state.read_version("hud");
        let observed_a = geometry(560);
        let b = finish(&mut state, 700);
        let error = state
            .accept_read("hud", before_read, observed_a)
            .err()
            .unwrap();
        assert_eq!(error.code, "geometry_snapshot_changed");
        assert_eq!(state.owners["hud"].stamp.user_generation, 0);
        assert!(state.owners["hud"].stamp.actual.same_bounds(b.target));
        let fresh = state.read_version("hud");
        assert!(
            state
                .accept_read("hud", fresh, b.target)
                .unwrap()
                .invalidation
                .is_none()
        );
    }
    #[test]
    fn stale_read_during_a_user_edge_or_replacement_never_rewinds_owner_state() {
        let mut state = state();
        let before = state.read_version("hud");
        state.native_edge(7, 1).unwrap();
        assert!(state.accept_read("hud", before, geometry(560)).is_err());
        assert_eq!(state.owners["hud"].stamp.user_generation, 1);
        let before = state.read_version("hud");
        state
            .observe(
                "hud",
                PhysicalGeometry {
                    epoch: 8,
                    ..geometry(560)
                },
            )
            .unwrap();
        assert!(state.accept_read("hud", before, geometry(560)).is_err());
        assert_eq!(state.owners["hud"].stamp.epoch, 8);
    }
}
