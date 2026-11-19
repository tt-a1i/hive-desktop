use super::{
    Bindings, CHORD_CONFLICT, FILE, Preferences, Target, check_conflicts, load, save, validate_chord,
};
use gpui_kit::{KeyBinding, Keymap, Keystroke, OperatingSystem};
use std::{convert::identity, fs};

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
        Some(Target::NewSession.default(OperatingSystem::Mac))
    );
    assert_eq!(
        migrated.get(Target::ToggleTerminal).as_deref(),
        Some(Target::ToggleTerminal.default(OperatingSystem::Mac))
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
        let search = Target::PreviousSession.default(os);
        let find = Target::OpenGitWorkspace.default(os);
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
            Some(Target::NextSession.default(os))
        );

        migrated.set(Target::Search, Some(Target::Search.default(os).to_owned()));
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
