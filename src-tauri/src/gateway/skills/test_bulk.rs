use super::*;

#[test]
fn plan_align_action_matrix() {
    assert_eq!(plan_align_action(true, false), AlignAction::Enable);
    assert_eq!(plan_align_action(false, true), AlignAction::Disable);
    assert_eq!(plan_align_action(true, true), AlignAction::Keep);
    assert_eq!(plan_align_action(false, false), AlignAction::Keep);
}
