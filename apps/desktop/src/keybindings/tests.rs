use super::{
    Bindings, CHORD_CONFLICT, FILE, Preferences, Target, check_conflicts, load, save, validate_chord,
};
use gpui_kit::{KeyBinding, Keymap, Keystroke, OperatingSystem};
use std::{convert::identity, fs};

#[test]
fn older_custom_shortcut_keeps_its_chord_when_current_project_action_is_added() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join(FILE);
    let os = OperatingSystem::Mac;
    fs::write(&file, r#"{"version":1,"bindings":{"search":"cmd-k","find":"cmd-f","preferences":"cmd-,","new_session":"cmd-shift-n"}}"#).unwrap();
    let loaded = load(root.path(), os, identity).unwrap();
    assert_eq!(loaded.get(Target::NewSession).as_deref(), Some("cmd-shift-n"));
    assert_eq!(loaded.get(Target::NewSessionCurrentProject), &None);

    fs::write(&file, r#"{"version":1,"bindings":{"search":"cmd-k","find":"cmd-f","preferences":"cmd-,","new_session_current_project":null}}"#).unwrap();
    let loaded = load(root.path(), os, identity).unwrap();
    assert_eq!(loaded.get(Target::NewSession).as_deref(), Some("cmd-n"));
    assert_eq!(loaded.get(Target::NewSessionCurrentProject), &None);
}

#[test]
fn validates_single_modified_chords_and_detects_collisions() {
    for os in [OperatingSystem::Mac, OperatingSystem::Windows] {
        let defaults = Bindings::defaults(os);
        assert!(defaults.normalized(os).is_ok());
        let chord = if os == OperatingSystem::Mac {
            "cmd-shift-j"
        } else {
            "ctrl-shift-j"
        };
        assert!(validate_chord(chord, os).is_ok());
        for key in [
            "a",
            "escape",
            "ctrl-c",
            "cmd-q",
            "ctrl-v",
            "ctrl-enter",
            "ctrl-tab",
            "ctrl-\0",
            "ctrl-k ctrl-f",
            "ctrl-k->x",
            "shift-j",
            "fn-j",
        ] {
            assert!(validate_chord(key, os).is_err(), "{os:?}: {key:?}");
        }
        let mut conflict = defaults.clone();
        conflict.set(Target::Search, defaults.get(Target::Find).clone());
        assert!(conflict.normalized(os).is_err());
    }
    assert!(validate_chord("cmd-shift-3", OperatingSystem::Mac).is_err());
    assert!(validate_chord("ctrl-alt-j", OperatingSystem::Windows).is_err());
    assert!(validate_chord("win-j", OperatingSystem::Windows).is_err());
}
#[test]
fn atomic_settings_keep_active_bindings_until_restart_and_protect_old_file() {
    let root = tempfile::tempdir().unwrap();
    let os = OperatingSystem::Mac;
    let defaults = Bindings::defaults(os);
    let mut state = Preferences::new(os);
    state.root = Some(root.path().to_owned());
    state
        .draft
        .set(Target::Search, Some("cmd-shift-j".to_owned()));
    state.draft.set(Target::Find, None);
    state.persist_pending(os);
    let bytes = fs::read(root.path().join(FILE)).unwrap();
    assert_eq!(state.active, defaults);
    assert_eq!(state.saved, state.draft);
    assert_eq!(load(root.path(), os, identity).unwrap(), state.saved);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(root.path().join(FILE))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let saved = state.saved.clone();
    state.draft.set(Target::Search, Some("cmd-c".to_owned()));
    state.persist_pending(os);
    assert_eq!(fs::read(root.path().join(FILE)).unwrap(), bytes);
    assert_eq!(state.saved, saved);
    assert_eq!(state.active, defaults);
    // A real replacement error leaves the existing target untouched and no
    // partial temporary file behind; the last published settings stay saved.
    fs::rename(root.path().join(FILE), root.path().join("original.json")).unwrap();
    fs::create_dir(root.path().join(FILE)).unwrap();
    state
        .draft
        .set(Target::Search, Some("cmd-shift-l".to_owned()));
    state.persist_pending(os);
    assert!(root.path().join(FILE).is_dir());
    assert_eq!(fs::read(root.path().join("original.json")).unwrap(), bytes);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    assert_eq!(state.saved, saved);
    assert_eq!(state.active, defaults);
}
#[test]
fn unknown_or_partial_files_are_not_migrated_or_overwritten() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(FILE);
    for value in [
        r#"{"version":2,"bindings":{"search":null,"find":null,"preferences":null}}"#,
        r#"{"version":1,"bindings":{"search":null}}"#,
        r#"{"version":1,"bindings":{"search":null,"find":null,"preferences":null},"commands":[]}"#,
        "not json",
    ] {
        fs::write(&path, value).unwrap();
        assert!(load(root.path(), OperatingSystem::Mac, identity).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), value);
    }
    fs::remove_file(&path).unwrap();
    assert_eq!(
        load(root.path(), OperatingSystem::Mac, identity).unwrap(),
        Bindings::defaults(OperatingSystem::Mac)
    );
    assert!(!path.exists());
    let defaults = Bindings::defaults(OperatingSystem::Windows);
    save(root.path(), &defaults, OperatingSystem::Windows).unwrap();
    assert_eq!(
        load(root.path(), OperatingSystem::Windows, identity).unwrap(),
        defaults
    );
}
#[test]
fn shifted_punctuation_finishes_when_primary_modifier_is_released() {
    use super::primary_released;
    use gpui_kit::Modifiers;
    let candidate = Keystroke::parse("cmd-?").unwrap();
    let keyup_after_shift_release = Keystroke::parse("cmd-/").unwrap();
    assert_ne!(candidate.key, keyup_after_shift_release.key);
    assert!(!primary_released(
        &candidate,
        keyup_after_shift_release.modifiers
    ));
    assert!(primary_released(&candidate, Modifiers::none()));
    let windows = Keystroke::parse("ctrl-shift-j").unwrap();
    assert!(!primary_released(
        &windows,
        Keystroke::parse("ctrl-j").unwrap().modifiers
    ));
    assert!(primary_released(&windows, Modifiers::none()));
    for normalized_screenshot in ["cmd-#", "cmd-$", "cmd-%", "cmd-^"] {
        assert!(validate_chord(normalized_screenshot, OperatingSystem::Mac).is_err());
    }
}

#[test]
fn first_authoritative_load_preserves_only_explicit_offline_edits() {
    let baseline = Bindings::defaults(OperatingSystem::Mac);
    let mut pending = baseline.clone();
    pending.set(Target::Search, Some("cmd-shift-j".to_owned()));
    let mut loaded = baseline.clone();
    loaded.set(Target::Find, Some("cmd-shift-t".to_owned()));
    let merged = loaded.preserve_pending(&baseline, &pending);
    assert_eq!(merged.get(Target::Search).as_deref(), Some("cmd-shift-j"));
    assert_eq!(merged.get(Target::Find).as_deref(), Some("cmd-shift-t"));
    assert_eq!(
        merged.get(Target::Preferences),
        loaded.get(Target::Preferences)
    );
}
#[test]
fn component_shortcuts_are_protected_from_new_global_chords() {
    use super::protected_binding;
    assert!(protected_binding(&KeyBinding::new(
        "cmd-]",
        gpui_kit::base::input::Indent,
        Some("Input")
    )));
    assert!(protected_binding(&KeyBinding::new(
        "cmd-q",
        crate::Quit,
        None
    )));
    assert!(!protected_binding(&KeyBinding::new(
        "cmd-k",
        crate::OpenSessionSearch,
        None
    )));
    assert!(protected_binding(&KeyBinding::new(
        "cmd-k",
        gpui_kit::Unbind("hive_desktop::OpenSessionSearch".into()),
        None
    )));
}

#[test]
fn prompt_stash_only_dispatches_in_composer_and_can_be_unbound() {
    use gpui_kit::{Action, KeyContext};
    let shortcut = super::PromptStash;
    let original = KeyBinding::new("ctrl-s", shortcut, Some("HiveComposer"));
    let mut keymap = Keymap::new(vec![original]);
    let key = [Keystroke::parse("ctrl-s").unwrap()];
    let composer = [KeyContext::parse("HiveComposer").unwrap(), KeyContext::parse("Input").unwrap()];
    let terminal = [KeyContext::parse("HiveTerminal").unwrap(), KeyContext::parse("Input").unwrap()];
    assert_eq!(Target::PromptStash.context().unwrap().depth_of(&composer), Some(1));
    assert_eq!(keymap.bindings_for_input(&key, &composer).0.len(), 1);
    assert!(keymap.bindings_for_input(&key, &terminal).0.is_empty());
    assert!(keymap.bindings_for_input(&key, &[]).0.is_empty());
    super::replace_bindings(&mut keymap, vec![(super::PromptStash.name(), None)]);
    assert!(keymap.bindings_for_input(&key, &composer).0.is_empty());
}

#[test]
fn send_and_start_draft_has_its_own_rebindable_composer_chord() {
    use gpui_kit::{Action, KeyContext};
    let target = Target::SendAndStartDraft;
    for os in [OperatingSystem::Mac, OperatingSystem::Windows] {
        let chord = target.default(os).unwrap();
        assert!(target.validate(chord, os).is_ok());
        assert!(Target::NewSession.validate(chord, os).is_err());
        let map = Keymap::new(vec![KeyBinding::new(
            chord, super::SendAndStartDraft, Some("HiveComposer"),
        )]);
        let key = [Keystroke::parse_with_os(chord, os).unwrap()];
        let ordinary_send = [Keystroke::parse_with_os(
            if os == OperatingSystem::Mac { "cmd-enter" } else { "ctrl-enter" }, os,
        ).unwrap()];
        let composer = [KeyContext::parse("HiveComposer").unwrap(), KeyContext::parse("Input").unwrap()];
        let terminal = [KeyContext::parse("HiveTerminal").unwrap(), KeyContext::parse("Input").unwrap()];
        assert_eq!(target.context().unwrap().depth_of(&composer), Some(1));
        assert_eq!(map.bindings_for_input(&key, &composer).0[0].action().name(), super::SendAndStartDraft.name());
        assert!(map.bindings_for_input(&key, &terminal).0.is_empty());
        assert!(map.bindings_for_input(&ordinary_send, &composer).0.is_empty());
    }
}

#[test]
fn effort_shortcuts_can_be_assigned_rebound_and_reset_without_a_default() {
    use gpui_kit::{Action, KeyContext};

    let root = tempfile::tempdir().unwrap();
    let os = OperatingSystem::Mac;
    let increase = Target::IncreaseReasoningEffort;
    let decrease = Target::DecreaseReasoningEffort;
    let old = r#"{"version":1,"bindings":{"search":"cmd-k","find":"cmd-f","preferences":"cmd-,"}}"#;
    fs::write(root.path().join(FILE), old).unwrap();
    let mut bindings = load(root.path(), os, identity).unwrap();
    assert_eq!(bindings.get(increase), &None);
    assert_eq!(bindings.get(decrease), &None);

    bindings.set(increase, Some("cmd-shift-j".into()));
    bindings.set(decrease, Some("cmd-shift-j".into()));
    assert_eq!(bindings.normalized(os), Err(CHORD_CONFLICT));
    bindings.set(decrease, Some("cmd-shift-k".into()));
    save(root.path(), &bindings, os).unwrap();
    assert_eq!(load(root.path(), os, identity).unwrap(), bindings);

    let mut keymap = Keymap::new(vec![
        KeyBinding::new("cmd-shift-j", super::IncreaseReasoningEffort, Some("HiveComposer")),
        KeyBinding::new("cmd-shift-k", super::DecreaseReasoningEffort, Some("HiveComposer")),
    ]);
    let composer = [KeyContext::parse("HiveComposer").unwrap(), KeyContext::parse("Input").unwrap()];
    let terminal = [KeyContext::parse("HiveTerminal").unwrap(), KeyContext::parse("Input").unwrap()];
    for (target, action) in [
        (increase, super::IncreaseReasoningEffort.name()),
        (decrease, super::DecreaseReasoningEffort.name()),
    ] {
        let chord = bindings.get(target).as_deref().unwrap();
        let keys = [target.validate(chord, os).unwrap()];
        assert_eq!(target.context().unwrap().depth_of(&composer), Some(1));
        assert_eq!(keymap.bindings_for_input(&keys, &composer).0[0].action().name(), action);
        assert!(keymap.bindings_for_input(&keys, &terminal).0.is_empty());
        super::replace_bindings(&mut keymap, vec![(action, None)]);
        assert!(keymap.bindings_for_input(&keys, &composer).0.is_empty());
        bindings.set(target, target.default(os).map(str::to_owned));
    }
    save(root.path(), &bindings, os).unwrap();
    let loaded = load(root.path(), os, identity).unwrap();
    assert_eq!(loaded.get(increase), &None);
    assert_eq!(loaded.get(decrease), &None);
}

#[test]
fn legacy_three_key_file_migrates_new_targets_and_preserves_explicit_null() {
    let root = tempfile::tempdir().unwrap();
    let legacy = r#"{"version":1,"bindings":{"search":"cmd-shift-j","find":null,"preferences":"cmd-,"}}"#;
    fs::write(root.path().join(FILE), legacy).unwrap();
    let migrated = load(root.path(), OperatingSystem::Mac, identity).unwrap();
    assert_eq!(migrated.get(Target::Search).as_deref(), Some("cmd-shift-j"));
    assert_eq!(migrated.get(Target::Find), &None);
    assert_eq!(migrated.get(Target::Preferences).as_deref(), Some("cmd-,"));
    assert_eq!(
        migrated.get(Target::NewSession).as_deref(),
        Target::NewSession.default(OperatingSystem::Mac)
    );
    assert_eq!(
        migrated.get(Target::ToggleTerminal).as_deref(),
        Target::ToggleTerminal.default(OperatingSystem::Mac)
    );

    let with_disabled_new = r#"{"version":1,"bindings":{"search":"cmd-shift-j","find":"cmd-f","preferences":"cmd-,","new_session":null}}"#;
    fs::write(root.path().join(FILE), with_disabled_new).unwrap();
    let loaded = load(root.path(), OperatingSystem::Mac, identity).unwrap();
    assert_eq!(loaded.get(Target::NewSession), &None);
}

#[test]
fn legacy_custom_shortcuts_take_priority_over_new_defaults() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(FILE);
    for os in [OperatingSystem::Mac, OperatingSystem::Windows] {
        let search = Target::PreviousSession.default(os).unwrap();
        let find = Target::OpenGitWorkspace.default(os).unwrap();
        let legacy = serde_json::json!({
            "version": 1,
            "bindings": {"search": search, "find": find, "preferences": null}
        });
        let legacy_bytes = serde_json::to_vec(&legacy).unwrap();
        fs::write(&path, &legacy_bytes).unwrap();

        let mut migrated = load(root.path(), os, identity).unwrap();
        assert_eq!(fs::read(&path).unwrap(), legacy_bytes);
        assert_eq!(migrated.get(Target::Search).as_deref(), Some(search));
        assert_eq!(migrated.get(Target::Find).as_deref(), Some(find));
        assert_eq!(migrated.get(Target::Preferences), &None);
        assert_eq!(migrated.get(Target::PreviousSession), &None);
        assert_eq!(migrated.get(Target::OpenGitWorkspace), &None);
        assert_eq!(
            migrated.get(Target::NextSession).as_deref(),
            Target::NextSession.default(os)
        );

        migrated.set(Target::Search, Target::Search.default(os).map(str::to_owned));
        save(root.path(), &migrated, os).unwrap();
        assert_eq!(load(root.path(), os, identity).unwrap(), migrated);
    }
}

#[test]
fn migration_rejects_duplicate_explicit_shortcuts() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(FILE);
    let config = br#"{"version":1,"bindings":{"search":"cmd-;","find":"cmd-f","preferences":null,"previous_session":"cmd-;"}}"#;
    fs::write(&path, config).unwrap();
    assert!(load(root.path(), OperatingSystem::Mac, identity).is_err());
    assert_eq!(fs::read(&path).unwrap(), config);
}

#[test]
fn shortcuts_that_dispatch_as_the_same_key_conflict() {
    let os = OperatingSystem::Windows;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(FILE);
    let old = br#"{"version":1,"bindings":{"search":"ctrl-shift-;","find":"ctrl-:","preferences":null}}"#;
    fs::write(&path, old).unwrap();

    // Model the Windows US keyboard mapper's output for this key. Production
    // asks the active platform mapper and works with its dispatch identity.
    let dispatch_key = |mut key: Keystroke| {
        if key.modifiers.shift && key.key == ";" {
            key.modifiers.shift = false;
            key.key = ":".to_owned();
        }
        key
    };
    let unmapped = load(root.path(), os, identity).unwrap();
    assert_eq!(
        check_conflicts(&unmapped, os, dispatch_key, |_| false),
        Err(CHORD_CONFLICT)
    );

    let loaded = load(root.path(), os, dispatch_key).unwrap();
    assert_eq!(loaded.get(Target::Find).as_deref(), Some("ctrl-:"));
    assert_eq!(loaded.get(Target::Search), &None);
    assert_eq!(loaded.get(Target::Preferences), &None);
    assert_eq!(
        loaded.get(Target::PreviousSession).as_deref(),
        Some("ctrl-;")
    );
    assert_eq!(fs::read(&path).unwrap(), old);
    assert_eq!(check_conflicts(&loaded, os, dispatch_key, |_| false), Ok(()));
    save(root.path(), &loaded, os).unwrap();
    assert_eq!(load(root.path(), os, dispatch_key).unwrap(), loaded);
}

#[test]
fn added_defaults_yield_to_saved_shortcuts_after_layout_mapping() {
    let os = OperatingSystem::Windows;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(FILE);
    let old = br#"{"version":1,"bindings":{"search":"ctrl-shift-,","find":"ctrl-f","preferences":null}}"#;
    fs::write(&path, old).unwrap();

    // On a layout where Shift+comma produces a semicolon, the old custom
    // shortcut takes priority over the new action's semicolon default.
    let loaded = load(root.path(), os, |mut key| {
        if key.modifiers.shift && key.key == "," {
            key.modifiers.shift = false;
            key.key = ";".to_owned();
        }
        key
    })
    .unwrap();
    assert_eq!(loaded.get(Target::Search).as_deref(), Some("ctrl-shift-,"));
    assert_eq!(loaded.get(Target::Find).as_deref(), Some("ctrl-f"));
    assert_eq!(loaded.get(Target::PreviousSession), &None);
    assert_eq!(loaded.get(Target::Preferences), &None);
    assert_eq!(fs::read(&path).unwrap(), old);
}

#[test]
fn repeated_live_rebinds_keep_the_keymap_bounded_and_preserve_other_bindings() {
    use super::replace_bindings;
    use gpui_kit::{Action, KeyContext, Unbind, base::input::Copy};

    let mut map = Keymap::new(vec![
        KeyBinding::new("cmd-k", crate::OpenSessionSearch, None),
        KeyBinding::new("cmd-`", crate::ToggleWorkspaceTerminal, None),
        KeyBinding::new("cmd-b", crate::ToggleSidebar, None),
    ]);
    map.add_bindings([
        KeyBinding::new("cmd-q", crate::Quit, None),
        KeyBinding::new("cmd-c", Copy, None),
        KeyBinding::new("cmd-shift-c", Copy, None),
        KeyBinding::new("cmd-shift-c", Unbind(Copy.name().into()), None),
        KeyBinding::new("cmd-shift-h", crate::OpenSessionSearch, Some("TestContext")),
    ]);
    let initial_count = map.bindings().len();
    let initial_version = map.version();
    let search = crate::OpenSessionSearch.name();
    let terminal = crate::ToggleWorkspaceTerminal.name();
    let matches = |map: &Keymap, chord| {
        map.bindings_for_input(&[Keystroke::parse(chord).unwrap()], &[]).0.len()
    };
    for _ in 0..200 {
        replace_bindings(&mut map, vec![
            (search, Some(KeyBinding::new("cmd-shift-j", crate::OpenSessionSearch, None))),
            (terminal, Some(KeyBinding::new("cmd-shift-t", crate::ToggleWorkspaceTerminal, None))),
        ]);
        assert_eq!(map.bindings().len(), initial_count);
        assert_eq!(matches(&map, "cmd-k"), 0);
        assert_eq!(matches(&map, "cmd-shift-j"), 1);
        assert_eq!(matches(&map, "cmd-shift-t"), 1);

        replace_bindings(&mut map, vec![(search, None), (terminal, None)]);
        assert_eq!(map.bindings().len(), initial_count - 2);
        assert_eq!(matches(&map, "cmd-shift-j"), 0);
        assert_eq!(matches(&map, "cmd-shift-t"), 0);
        let context = [KeyContext::parse("TestContext").unwrap()];
        assert_eq!(map.bindings_for_input(&[Keystroke::parse("cmd-shift-h").unwrap()], &context).0.len(), 1);

        replace_bindings(&mut map, vec![
            (search, Some(KeyBinding::new("cmd-k", crate::OpenSessionSearch, None))),
            (terminal, Some(KeyBinding::new("cmd-`", crate::ToggleWorkspaceTerminal, None))),
        ]);
        assert_eq!(map.bindings().len(), initial_count);
        assert_eq!(matches(&map, "cmd-k"), 1);
        assert_eq!(matches(&map, "cmd-`"), 1);
        assert_eq!(matches(&map, "cmd-q"), 1);
        assert_eq!(matches(&map, "cmd-c"), 1);
        assert_eq!(matches(&map, "cmd-shift-c"), 0);
    }
    assert!(map.version() != initial_version);
}

#[test]
fn layout_change_replaces_old_mapped_key_in_the_real_keymap() {
    use super::replace_bindings;
    use gpui_kit::Action;
    // Model a layout switch after the platform mapper registered ctrl-shift-;
    // as ctrl-: on layout A. Layout B emits ctrl-; for that saved chord.
    let mut map = Keymap::new(vec![KeyBinding::new("ctrl-:", crate::OpenSessionSearch, None)]);
    let typed = Keystroke::parse("ctrl-;").unwrap();
    assert_eq!(map.bindings_for_input(&[typed.clone()], &[]).0.len(), 0);
    replace_bindings(&mut map, vec![(crate::OpenSessionSearch.name(), Some(KeyBinding::new("ctrl-;", crate::OpenSessionSearch, None)))]);
    assert_eq!(map.bindings_for_input(&[typed], &[]).0.len(), 1);
}

#[test]
fn layout_failure_is_separate_from_unresolved_read_and_save_errors() {
    use super::Notice;
    let os = OperatingSystem::Windows;
    for read_error in [true, false] {
        let mut state = Preferences::new(os);
        let before = state.active.clone();
        state.notice = Some(if read_error { Notice::Read("read failure".into()) } else { Notice::Save("save failure".into()) });
        state.record_layout_result(Err("binding creation failed".into()));
        assert!(state.layout_blocked);
        assert_eq!(state.layout_error.as_deref(), Some("binding creation failed"));
        assert_eq!(state.active, before);
        match &state.notice {
            Some(Notice::Read(message)) if read_error => assert_eq!(message, "read failure"),
            Some(Notice::Save(message)) if !read_error => assert_eq!(message, "save failure"),
            _ => panic!("layout failure replaced an existing persistence error"),
        }
        state.record_layout_result(Ok(()));
        assert!(!state.layout_blocked);
        assert!(state.layout_error.is_none());
        assert!(matches!(&state.notice, Some(Notice::Read(_)) | Some(Notice::Save(_))));
    }
}

#[test]
fn workspace_focus_defaults_are_scoped_away_from_terminal_input() {
    use super::{FocusNextFromTerminal, FocusNextWorkspaceArea, FocusPreviousWorkspaceArea};
    use gpui_kit::KeyContext;

    let keymap = Keymap::new(vec![
        KeyBinding::new("f6", FocusNextWorkspaceArea, Some("!HiveTerminal")),
        KeyBinding::new("shift-f6", FocusPreviousWorkspaceArea, Some("!HiveTerminal")),
        KeyBinding::new("cmd-f6", FocusNextFromTerminal, Some("HiveTerminal")),
    ]);
    let workspace = KeyContext::parse("HiveWorkspace").unwrap();
    let sidebar = [workspace.clone()];
    let composer = [
        workspace.clone(),
        KeyContext::parse("HiveConversation").unwrap(),
        KeyContext::parse("HiveComposer").unwrap(),
        KeyContext::parse("Textarea").unwrap(),
    ];
    let terminal = [
        workspace,
        KeyContext::parse("HiveConversation").unwrap(),
        KeyContext::parse("HiveTerminal").unwrap(),
    ];
    let forward = Keystroke::parse("f6").unwrap();
    let backward = Keystroke::parse("shift-f6").unwrap();
    let modified = Keystroke::parse("cmd-f6").unwrap();

    for key in [forward, backward] {
        for inside_workspace in [&sidebar[..], &composer[..]] {
            assert_eq!(
                keymap
                    .bindings_for_input(std::slice::from_ref(&key), inside_workspace)
                    .0
                    .len(),
                1
            );
        }
        assert!(keymap.bindings_for_input(&[key], &terminal).0.is_empty());
    }
    assert_eq!(keymap.bindings_for_input(&[modified], &terminal).0.len(), 1);
}

#[test]
fn existing_shortcut_files_get_workspace_focus_defaults_and_keep_disabled_overrides() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(FILE);
    for os in [OperatingSystem::Mac, OperatingSystem::Windows] {
        fs::write(&path, r#"{"version":1,"bindings":{"search":null,"find":null,"preferences":null}}"#).unwrap();
        let mut bindings = load(root.path(), os, identity).unwrap();
        assert_eq!(bindings.get(Target::FocusNextWorkspaceArea).as_deref(), Some("f6"));
        assert_eq!(bindings.get(Target::FocusPreviousWorkspaceArea).as_deref(), Some("shift-f6"));
        assert!(Target::Search.validate("f6", os).is_err());
        bindings.set(Target::FocusNextWorkspaceArea, None);
        save(root.path(), &bindings, os).unwrap();
        assert_eq!(load(root.path(), os, identity).unwrap(), bindings);
    }
}
