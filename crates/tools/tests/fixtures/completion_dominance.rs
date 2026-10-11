//! Shared, engine-legal capture fixture. Executes the real Local evaluator.
use agent::{SeatView, Table};
use engine::{ActionType, Card, CardSet, GameAction, GamePhase};
use heuristic::sequence_concepts::{Actions, Guard, Information, Node, Program, SequenceError, Stage};
use heuristic::sequence_evaluation::{
    self, Candidate, Context, LeafBudget, LeafFailure, LeafResponse, Limits, SelectionProof,
};
use heuristic::value_concepts::{CohortMethod, Provenance};
use heuristic::{BeliefScope, Params, PlanMode, PlanningAssessment, PlanningContext, SequenceTrace};

fn leaf(_: &SeatView<'_>, legal: &[GameAction], _: LeafBudget) -> Result<LeafResponse, LeafFailure> {
    legal
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .copied()
        .map(LeafResponse::unit)
        .ok_or_else(|| LeafFailure::unit(SequenceError::WrongContext))
}

pub fn fixture(conditional: bool) -> (SequenceTrace, Option<PlanningAssessment>) {
    let hands = std::array::from_fn(|seat| (seat * 9..seat * 9 + 9).map(|i| Card::from_index(i as u8)).collect());
    let mut table = Table::new(&hands);
    while table.game.state.all_tricks.len() < 7 {
        let (_, legal) = table.to_act().unwrap();
        let action = match table.game.state.phase {
            GamePhase::WaitingForStart => legal[0],
            GamePhase::Bidding => *legal.iter().find(|a| a.action_type == ActionType::StopBidding).unwrap(),
            _ => *legal
                .iter()
                .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                .min_by_key(|a| match a.action_type {
                    ActionType::CardPlayed(c) => c.index(),
                    _ => unreachable!(),
                })
                .unwrap(),
        };
        assert!(table.game.is_legal(&action));
        table.apply(action);
    }
    assert_eq!(table.to_act().unwrap().0, 0);
    let cards: Vec<_> = table
        .to_act()
        .unwrap()
        .1
        .iter()
        .filter_map(|a| matches!(a.action_type, ActionType::CardPlayed(_)).then_some(a.action_type))
        .collect();
    assert_eq!(cards.len(), 2);
    let initial = Information::of(&table.view(0));
    let context = Context {
        params: Params::default(),
        seed: 0,
        posterior_revision: 0,
        provenance: Provenance {
            belief_model: "capture-local-fixture",
            response_model: "first-legal-card-fixture",
            cohort: CohortMethod::Enumerated,
        },
    };
    let (cohort, nodes, start) = if conditional {
        let mut hands = std::array::from_fn(|seat| table.hand(seat as u8));
        let a = CardSet::single(hands[1].cards()[0].index());
        let b = CardSet::single(hands[3].cards()[0].index());
        hands[1] = (hands[1] - a) | b;
        hands[3] = (hands[3] - b) | a;
        let other = Table::from_view(&table.view(0), hands).unwrap();
        let cohort: Vec<_> = (0..8)
            .map(|origin| {
                (
                    if [2, 7].contains(&origin) {
                        table.clone()
                    } else {
                        other.clone()
                    },
                    (origin + 1) as f64 / 36.0,
                )
            })
            .collect();
        (
            cohort,
            vec![
                Node::Tail,
                Node::Action {
                    actions: Actions::Exact(cards[1]),
                    next: Stage(0),
                },
                Node::Choice(vec![Stage(1), Stage(1)]),
                Node::Guard {
                    predicate: Guard::OwnLead,
                    yes: Stage(2),
                    no: Stage(0),
                    unknown: Stage(0),
                },
                Node::Action {
                    actions: Actions::Exact(cards[0]),
                    next: Stage(3),
                },
            ],
            Stage(4),
        )
    } else {
        (
            vec![(table.clone(), 0.25), (table.clone(), 0.75)],
            vec![
                Node::Tail,
                Node::Action {
                    actions: Actions::Exact(cards[0]),
                    next: Stage(0),
                },
                Node::Action {
                    actions: Actions::Exact(cards[1]),
                    next: Stage(0),
                },
                Node::Choice(vec![Stage(1), Stage(2), Stage(1)]),
                Node::Guard {
                    predicate: Guard::OwnLead,
                    yes: Stage(3),
                    no: Stage(0),
                    unknown: Stage(0),
                },
            ],
            Stage(4),
        )
    };
    let candidate = Candidate::new(Program {
        name: "capture-completion-dominance",
        version: 1,
        start,
        nodes,
    });
    let compared = sequence_evaluation::evaluate(
        &[candidate],
        &cohort,
        0,
        context.provenance,
        &context,
        Limits {
            memo_entries: 0,
            memo_bytes: 0,
            reuse_suffixes: false,
            ..Limits::default()
        },
        &mut leaf,
    )
    .unwrap();
    let information = if conditional {
        compared
            .selection
            .records()
            .iter()
            .find(|r| {
                matches!(&r.proof, Some(SelectionProof::CompletionDominance(_)))
                    && r.support.iter().map(|(i, _)| *i).eq([2, 7])
            })
            .unwrap()
            .information
            .clone()
    } else {
        initial.clone()
    };
    let choices: Vec<_> = compared
        .selection
        .records()
        .iter()
        .filter(|r| r.information == information)
        .cloned()
        .collect();
    let program = if conditional {
        choices
            .iter()
            .find(|r| matches!(&r.proof, Some(SelectionProof::CompletionDominance(_))))
            .map(|r| r.semantic_suffix.clone())
    } else {
        compared.receipt.program.suffix(compared.receipt.stage).ok()
    };
    let trace = SequenceTrace {
        authorization: Some(compared.selection.authorization(&information)),
        mode: if conditional {
            PlanMode::Prepared
        } else {
            PlanMode::Formation
        },
        scope: if conditional {
            BeliefScope::PreparedConditional {
                origin: initial.clone(),
            }
        } else {
            BeliefScope::CurrentInformation
        },
        information,
        program,
        choices,
        terminals: std::sync::Arc::new(if conditional {
            vec![]
        } else {
            compared.executions[0].terminals.clone()
        }),
        work: if conditional { Default::default() } else { compared.work },
        failure: None,
        deferred_limit: None,
        total_candidates: Some(compared.executions.len()),
    };
    let assessment = (!conditional).then(|| PlanningAssessment {
        actions: compared.executions.iter().map(|e| e.first.action).collect(),
        values: compared.values,
        context: PlanningContext {
            observer: 0,
            belief_scope: BeliefScope::CurrentInformation,
            knowledge: initial.knowledge,
            events: initial.events,
            kinds: initial.kinds,
            series: initial.series,
            response_params: context.params,
            seed: context.seed,
            possible_allocations: cohort.len() as u64,
            requested_worlds: cohort.len(),
            proposal_draws: 0,
        },
    });
    (trace, assessment)
}
