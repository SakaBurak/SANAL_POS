use payos_domain::PaymentStatus::{self, *};

const ALLOWED: &[(PaymentStatus, PaymentStatus)] = &[
    (Created, RequiresAction),
    (Created, Authorized),
    (Created, Failed),
    (Created, Unknown),
    (RequiresAction, Authorized),
    (RequiresAction, Failed),
    (RequiresAction, Unknown),
    (Authorized, Captured),
    (Authorized, Voided),
    (Authorized, Unknown),
    (Captured, PartiallyRefunded),
    (Captured, Refunded),
    (Captured, Disputed),
    (Captured, Unknown),
    (PartiallyRefunded, PartiallyRefunded),
    (PartiallyRefunded, Refunded),
    (PartiallyRefunded, Disputed),
    (PartiallyRefunded, Unknown),
    (Unknown, Authorized),
    (Unknown, Failed),
    (Unknown, Captured),
    (Unknown, Voided),
    (Unknown, PartiallyRefunded),
    (Unknown, Refunded),
];

#[test]
fn transition_table_matches_specification_for_every_pair() {
    for from in PaymentStatus::ALL {
        for to in PaymentStatus::ALL {
            assert_eq!(
                from.can_transition_to(to),
                ALLOWED.contains(&(from, to)),
                "{from} -> {to}"
            );
        }
    }
}

#[test]
fn backward_transitions_are_rejected() {
    assert!(!Captured.can_transition_to(Authorized));
    assert!(!Authorized.can_transition_to(Created));
    assert!(!Refunded.can_transition_to(Captured));
    assert!(!PartiallyRefunded.can_transition_to(Captured));
}

#[test]
fn terminal_states_have_no_outgoing_transitions() {
    for from in PaymentStatus::ALL.into_iter().filter(|s| s.is_terminal()) {
        for to in PaymentStatus::ALL {
            assert!(!from.can_transition_to(to), "{from} -> {to}");
        }
    }
}

#[test]
fn terminal_states_are_exactly_failed_voided_refunded_disputed() {
    let terminal: Vec<_> = PaymentStatus::ALL
        .into_iter()
        .filter(|s| s.is_terminal())
        .collect();

    assert_eq!(terminal, vec![Failed, Voided, Refunded, Disputed]);
}

#[test]
fn unknown_cannot_re_enter_unknown() {
    assert!(!Unknown.can_transition_to(Unknown));
    assert!(!Unknown.can_transition_to(Created));
    assert!(!Unknown.can_transition_to(Disputed));
}

#[test]
fn status_strings_are_snake_case() {
    let expected = [
        "created",
        "requires_action",
        "authorized",
        "captured",
        "failed",
        "voided",
        "partially_refunded",
        "refunded",
        "unknown",
        "disputed",
    ];

    for (status, text) in PaymentStatus::ALL.into_iter().zip(expected) {
        assert_eq!(status.as_str(), text);
        assert_eq!(status.to_string(), text);
    }
}
