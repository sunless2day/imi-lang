//! ignore this, just tests like all others

#[test]
fn a_valid_program_runs() {
    assert!(imi_core::run("var x = 1;\nx += 1;").is_ok());
}

#[test]
fn division_by_zero_points_at_the_zero() {
    let err = imi_core::run("let x = 10;\nlet y = x / 0;").unwrap_err();
    assert_eq!((err.line, err.col), (2, 13));
}

#[test]
fn errors_come_with_a_suggestion_when_there_is_one() {
    let err = imi_core::run("let x = 1 ! 2;").unwrap_err();
    assert_eq!(err.suggestion.as_deref(), Some("Did you mean '!='?"));
}
