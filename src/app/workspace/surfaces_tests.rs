use super::*;

#[test]
fn cycling_the_workspace_walks_the_surfaces_and_leaves_a_diff() {
    for (surface, forward, expected) in [
        (AppSurface::Chat, true, Some(AppSurface::Editor)),
        (AppSurface::Chat, false, Some(AppSurface::Terminal)),
        (AppSurface::Editor, true, Some(AppSurface::Terminal)),
        (AppSurface::Editor, false, Some(AppSurface::Chat)),
        (AppSurface::Terminal, true, Some(AppSurface::Chat)),
        (AppSurface::Terminal, false, Some(AppSurface::Editor)),
        (AppSurface::Diff, true, Some(AppSurface::Chat)),
        (AppSurface::Diff, false, Some(AppSurface::Chat)),
    ] {
        assert_eq!(
            cycle_target(surface, forward),
            expected,
            "{surface:?} forward={forward}"
        );
    }
}

#[test]
fn arriving_requests_only_focus_when_replacing_the_composer_slot() {
    assert!(arriving_request_takes_focus(true));
    assert!(!arriving_request_takes_focus(false));
}

#[test]
fn activating_a_sheet_never_stacks_it_with_an_existing_sheet() {
    for sheet in [
        AppSheet::Sessions,
        AppSheet::Run,
        AppSheet::Keybindings,
        AppSheet::Settings,
        AppSheet::ProjectTrust,
    ] {
        let flags = sheet_flags(Some(sheet));
        assert_eq!(
            [
                flags.sessions,
                flags.run,
                flags.keybindings,
                flags.settings,
                flags.project_trust,
            ]
            .into_iter()
            .filter(|active| *active)
            .count(),
            1
        );
    }
    assert!(!sheet_flags(None).any());
}

#[test]
fn an_existing_sheet_prevents_recapturing_the_return_focus() {
    assert!(should_capture_return_focus(sheet_flags(None)));
    assert!(!should_capture_return_focus(sheet_flags(Some(
        AppSheet::Sessions
    ))));
    assert!(!should_capture_return_focus(sheet_flags(Some(
        AppSheet::Run
    ))));
}
