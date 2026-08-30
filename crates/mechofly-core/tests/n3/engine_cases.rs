use super::*;

struct ReferenceEngine {
    graph: Arc<ModelGraph>,
    state: ModelState,
    ledger: BehaviorTelemetryLedger,
    intent: BehaviorIntentSnapshot,
}

impl ReferenceEngine {
    fn from_state(graph: Arc<ModelGraph>, state: ModelState) -> Self {
        let spikes = state.spikes.iter().map(|value| *value as usize).sum();
        Self {
            graph,
            intent: frozen::intent(&state, spikes),
            ledger: BehaviorTelemetryLedger::new(state.frame),
            state,
        }
    }

    fn step(&mut self, stimulus: &[i32]) -> FrameSummary {
        // Independent scalar traversal checks orchestration against the
        // production parallel backend; update_neuron itself is frozen in N3.
        let next: Vec<(i32, u8)> = (0..self.state.activation.len())
            .map(|target| {
                update_neuron(
                    target,
                    self.state.frame + 1,
                    self.state.seed,
                    &self.state.activation,
                    &self.graph.incoming_offsets,
                    &self.graph.incoming_sources,
                    &self.graph.modeled_weights,
                    stimulus[target],
                )
            })
            .collect();
        let (activation, spikes): (Vec<i32>, Vec<u8>) = next.into_iter().unzip();
        self.accept(activation, spikes)
    }

    fn accept(&mut self, activation: Vec<i32>, spikes: Vec<u8>) -> FrameSummary {
        self.state.activation = activation;
        self.state.spikes = spikes;
        self.state.frame += 1;
        let spike_count = self.state.spikes.iter().map(|value| *value as usize).sum();
        let mean = if self.state.activation.is_empty() {
            0
        } else {
            (self
                .state
                .activation
                .iter()
                .map(|value| *value as i64)
                .sum::<i64>()
                / self.state.activation.len() as i64) as i32
        };
        self.intent = frozen::intent(&self.state, spike_count);
        let behavior = frozen::behavior(&self.state, spike_count);
        if behavior == self.state.behavior {
            self.state.behavior_age_frames += 1;
        } else {
            let from = self.state.behavior;
            let elapsed = self.state.behavior_age_frames.saturating_add(1);
            let before = self.state.digest();
            let reason = frozen::reason(from, behavior, &self.intent);
            self.state.behavior = behavior;
            self.state.behavior_age_frames = 0;
            self.ledger.record(BehaviorTransitionEvent::new(
                self.state.frame,
                from,
                behavior,
                elapsed,
                reason,
                self.intent,
                before,
                self.state.digest(),
            ));
        }
        FrameSummary {
            frame: self.state.frame,
            spike_count,
            mean_activation_q15: mean,
            behavior: self.state.behavior,
            state_digest: self.state.digest(),
        }
    }
}

fn assert_engine_equal(candidate: &ModelEngine, reference: &ReferenceEngine) {
    assert_eq!(candidate.state, reference.state);
    assert_eq!(candidate.last_behavior_intent, reference.intent);
    assert_eq!(
        candidate.behavior_telemetry_stream_sha256(),
        reference.ledger.event_stream_sha256()
    );
    assert_eq!(
        candidate.latest_behavior_transition(),
        reference.ledger.latest()
    );
}

#[test]
fn per_frame_neural_state_intent_events_and_checkpoint_parity() {
    // Full Demo4096 graphs; no reduced or fake dynamics are substituted.
    for seed in [0_u64, 19] {
        let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, seed));
        for scenario in 0..5 {
            let mut candidate = ModelEngine::new(Arc::clone(&graph), 0x7E1E_0000 ^ seed);
            let mut reference =
                ReferenceEngine::from_state(Arc::clone(&graph), candidate.state.clone());
            for frame in 0..900_u64 {
                let mut stimulus = vec![0; graph.neuron_ids.len()];
                let offset = match scenario {
                    0 => None,
                    1 => Some(WALK_POPULATION_OFFSET),
                    2 => Some(GROOM_POPULATION_OFFSET),
                    3 if frame % 300 < 12 => Some(LOOM_POPULATION_OFFSET),
                    4 => match (frame / 120) % 6 {
                        1 => Some(WALK_POPULATION_OFFSET),
                        2 => Some(GROOM_POPULATION_OFFSET),
                        3 => Some(ALERT_POPULATION_OFFSET),
                        4 => Some(REVERSE_POPULATION_OFFSET),
                        5 if frame % 120 < 12 => Some(LOOM_POPULATION_OFFSET),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(offset) = offset {
                    for value in stimulus
                        .iter_mut()
                        .skip(offset)
                        .step_by(FUNCTIONAL_POPULATION_COUNT)
                    {
                        *value = 8_192;
                    }
                }
                let actual = candidate.step_cpu(StepInput {
                    stimulus_q15: &stimulus,
                });
                let expected = reference.step(&stimulus);
                assert_eq!(
                    actual, expected,
                    "seed={seed} scenario={scenario} frame={frame}"
                );
                assert_eq!(
                    serde_json::to_vec(&actual).unwrap(),
                    serde_json::to_vec(&expected).unwrap()
                );
                assert_engine_equal(&candidate, &reference);
                if frame == 449 {
                    let serialized = serde_json::to_vec(&candidate.state).unwrap();
                    let restored: ModelState = serde_json::from_slice(&serialized).unwrap();
                    candidate =
                        ModelEngine::from_state(Arc::clone(&graph), restored.clone()).unwrap();
                    reference = ReferenceEngine::from_state(Arc::clone(&graph), restored);
                    assert_engine_equal(&candidate, &reference);
                }
            }
            assert_eq!(
                candidate.behavior_telemetry_snapshot(),
                reference.ledger.snapshot(reference.intent)
            );
        }
    }
    println!("N3_FRAMEWISE_FULL_GRAPH_CHECKS=9000");
}

#[test]
fn accepted_backend_output_parity_survives_ring_eviction_and_disabled_logging() {
    let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 9));
    let mut candidate = ModelEngine::new(Arc::clone(&graph), 27);
    let mut disabled = candidate.clone();
    disabled.set_behavior_telemetry_enabled(false);
    let mut reference = ReferenceEngine::from_state(graph, candidate.state.clone());
    for frame in 0..1_400 {
        let mut activation = vec![ACTIVATION_MIN; candidate.state.activation.len()];
        let offset = if frame % 2 == 0 {
            ALERT_POPULATION_OFFSET
        } else {
            WALK_POPULATION_OFFSET
        };
        activation[offset] = 4_600;
        let spikes = vec![0; activation.len()];
        let expected = reference.accept(activation.clone(), spikes.clone());
        let actual = candidate.accept_backend_step(activation.clone(), spikes.clone());
        assert_eq!(actual, expected);
        assert_eq!(disabled.accept_backend_step(activation, spikes), expected);
        assert_engine_equal(&candidate, &reference);
    }
    let telemetry = candidate.behavior_telemetry_snapshot();
    assert_eq!(telemetry.total_event_count, 1_400);
    assert_eq!(telemetry.retained_event_count, 512);
    assert_eq!(telemetry.dropped_event_count, 888);
    assert_eq!(telemetry, reference.ledger.snapshot(reference.intent));
    assert_eq!(disabled.behavior_telemetry_total_event_count(), 0);
    assert_eq!(
        disabled.last_behavior_intent,
        candidate.last_behavior_intent
    );
}
