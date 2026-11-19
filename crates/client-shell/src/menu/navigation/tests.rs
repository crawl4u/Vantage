use {
    super::super::MenuRuntime,
    launcher::menu::{MenuAction, MenuField, MenuScreen},
};

#[test]
fn chat_settings_back_returns_to_the_world_without_pause() {
    use launcher::menu::settings_sections::SettingSection;

    let mut menu = MenuRuntime::new(false, 2, "Player".into());
    menu.open_chat_settings();
    assert!(menu.is_visible());
    assert_eq!(menu.screen(), MenuScreen::Settings);
    assert_eq!(menu.settings_section(), Some(SettingSection::Chat));
    assert!(menu.over_world());
    assert!(!menu.uses_panorama());
    menu.activate(MenuAction::SearchSettings);
    menu.edit_text("chat");
    menu.activate(MenuAction::AddBack);
    assert!(!menu.is_visible());

    menu.open_pause();
    menu.activate(MenuAction::PauseSettings);
    assert!(menu.view().settings_query.is_empty());
    menu.activate(MenuAction::AddBack);
    assert!(menu.is_visible());
    assert_eq!(menu.screen(), MenuScreen::Pause);

    menu.show_home();
    menu.activate(MenuAction::Navigate(MenuScreen::Settings));
    assert!(menu.uses_panorama());
    menu.activate(MenuAction::AddBack);
    assert!(menu.is_visible());
    assert_eq!(menu.screen(), MenuScreen::Home);
}

#[test]
fn a_loaded_self_profile_can_open_dressing_room_from_keyboard_focus() {
    let mut menu = MenuRuntime::new(true, 2, "BugTest".into());
    menu.control_auth = Some(launcher::menu::auth::AuthState::Authenticated);
    menu.feeds.profile.loaded = true;
    menu.feeds.profile.avatar_loaded = true;
    menu.feeds.profile.featured_screenshot_loaded = true;
    menu.activate(MenuAction::Navigate(MenuScreen::Profile));
    let action = MenuAction::Navigate(MenuScreen::DressingRoom);
    menu.focus_pointer(action);
    assert_eq!(menu.view().focused_action, Some(action));
    menu.activate_focused();
    assert_eq!(menu.screen(), MenuScreen::DressingRoom);
    menu.activate(MenuAction::AddBack);
    assert_eq!(menu.screen(), MenuScreen::Profile);
}

fn join_from(screen: MenuScreen) -> MenuRuntime {
    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    menu.activate(MenuAction::Navigate(screen));
    menu.request_connect("fixture.invalid".into());
    assert!(menu.take_join_intent().is_some());
    menu.show_home();
    menu.show_connecting();
    menu.show_world();
    menu
}

#[test]
fn quitting_reveals_each_originating_play_page_with_a_way_back_home() {
    for origin in [MenuScreen::Play, MenuScreen::Social, MenuScreen::Servers] {
        let mut menu = join_from(origin);
        menu.open_pause();
        menu.activate(MenuAction::PauseSettings);
        menu.show_after_disconnect();
        assert_eq!(menu.screen(), origin);
        assert!(menu.is_visible());
        assert!(menu.uses_panorama());
        assert!(menu.view().message.is_none());
        menu.activate(MenuAction::AddBack);
        assert_eq!(menu.screen(), MenuScreen::Home);
    }
}

#[test]
fn a_replacement_join_retains_the_original_play_page() {
    let mut menu = join_from(MenuScreen::Servers);
    menu.request_connect("replacement.invalid".into());
    assert!(menu.take_join_intent().is_some());
    menu.show_home();
    menu.show_connecting();
    menu.show_world();
    menu.show_after_disconnect();
    assert_eq!(menu.screen(), MenuScreen::Servers);
}

#[test]
fn quitting_a_local_world_reveals_worlds() {
    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    menu.activate(MenuAction::Navigate(MenuScreen::Play));
    menu.request_local_world_join("Local fixture".into(), false);
    assert!(menu.take_join_intent().is_some());
    menu.show_world();
    menu.open_pause();
    menu.show_after_disconnect();
    assert_eq!(menu.screen(), MenuScreen::Play);
}

#[test]
fn joining_from_a_server_form_returns_to_the_page_that_opened_it() {
    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    menu.activate(MenuAction::Navigate(MenuScreen::Servers));
    menu.activate(MenuAction::PlayAddServer);
    menu.request_connect("fixture.invalid".into());
    assert!(menu.take_join_intent().is_some());
    menu.show_world();
    menu.show_after_disconnect();
    assert_eq!(menu.screen(), MenuScreen::Servers);
}

#[test]
fn cancellation_returns_to_the_origin_and_clears_the_queued_join() {
    for origin in [MenuScreen::Play, MenuScreen::Social, MenuScreen::Servers] {
        let mut menu = MenuRuntime::new(true, 2, "Player".into());
        menu.activate(MenuAction::Navigate(origin));
        menu.request_connect("fixture.invalid".into());
        menu.cancel_join();
        assert_eq!(menu.screen(), origin);
        assert!(!menu.is_connecting());
        assert!(menu.take_join_intent().is_none());
    }
}

#[test]
fn a_new_launcher_join_replaces_the_previous_origin() {
    let mut menu = join_from(MenuScreen::Servers);
    menu.show_after_disconnect();
    menu.activate(MenuAction::Navigate(MenuScreen::Social));
    menu.request_connect("fixture.invalid".into());
    assert!(menu.take_join_intent().is_some());
    menu.show_world();
    menu.show_after_disconnect();
    assert_eq!(menu.screen(), MenuScreen::Social);
}

#[test]
fn a_session_without_a_play_page_keeps_its_existing_return_defaults() {
    for launcher in [false, true] {
        let mut menu = MenuRuntime::new(launcher, 2, "Player".into());
        menu.show_world();
        menu.open_pause();
        menu.show_after_disconnect();
        assert_eq!(menu.screen(), MenuScreen::Home);
    }
    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    menu.request_connect("fixture.invalid".into());
    menu.cancel_join();
    assert_eq!(menu.screen(), MenuScreen::Play);
}

#[test]
fn tab_shortcuts_step_through_play_tabs_and_stop_at_each_end() {
    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    menu.activate(MenuAction::Navigate(MenuScreen::Play));
    menu.step_play_tab(-1);
    assert_eq!(menu.screen(), MenuScreen::Play);
    menu.step_play_tab(1);
    assert_eq!(menu.screen(), MenuScreen::Social);
    menu.step_play_tab(1);
    assert_eq!(menu.screen(), MenuScreen::Servers);
    menu.step_play_tab(1);
    assert_eq!(menu.screen(), MenuScreen::Servers);
    menu.step_play_tab(-1);
    assert_eq!(menu.screen(), MenuScreen::Social);
    menu.activate(MenuAction::AddBack);
    assert_eq!(menu.screen(), MenuScreen::Home);
}

#[test]
fn tab_shortcuts_do_nothing_off_the_play_screen_or_under_a_dialog() {
    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    menu.step_play_tab(1);
    assert_eq!(menu.screen(), MenuScreen::Home);
    menu.activate(MenuAction::Navigate(MenuScreen::Play));
    menu.activate(MenuAction::OpenExitDialog);
    menu.step_play_tab(1);
    assert_eq!(menu.screen(), MenuScreen::Play);
}
/// `[` and `]` type into a focused text field, so they must not also switch tabs.
#[test]
fn tab_shortcuts_leave_the_tabs_alone_while_a_text_field_has_focus() {
    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    menu.activate(MenuAction::Navigate(MenuScreen::Play));
    menu.focus_field(MenuField::Name);
    assert_eq!(menu.field, Some(MenuField::Name));
    menu.step_play_tab(1);
    assert_eq!(menu.screen(), MenuScreen::Play);
}

#[test]
fn java_settings_open_on_the_list_and_back_steps_out_one_level_at_a_time() {
    use launcher::menu::{
        JavaSettingsAction, JavaSettingsView,
        settings_options::{SettingId, menu_style::MenuStyle},
        settings_sections::SettingSection,
    };

    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    std::sync::Arc::make_mut(&mut menu.settings_options)
        .set(SettingId::MenuStyle.index(), MenuStyle::Java.value());
    let video = SettingSection::Video.definition().index;
    menu.show_home();
    menu.activate(MenuAction::Navigate(MenuScreen::Settings));
    assert_eq!(menu.view().settings.java_view, JavaSettingsView::Hub);
    // Hub, then a page, then the Bedrock page for the same section, and back down again.
    menu.activate(MenuAction::JavaSettings(JavaSettingsAction::Page(video)));
    assert_eq!(menu.view().settings.java_view, JavaSettingsView::Page);
    assert_eq!(menu.settings_section(), Some(SettingSection::Video));
    menu.activate(MenuAction::JavaSettings(JavaSettingsAction::Bedrock(video)));
    assert_eq!(menu.view().settings.java_view, JavaSettingsView::Bedrock);
    menu.activate(MenuAction::AddBack);
    assert_eq!(menu.view().settings.java_view, JavaSettingsView::Hub);
    assert_eq!(menu.screen(), MenuScreen::Settings);
    menu.activate(MenuAction::AddBack);
    assert_eq!(menu.screen(), MenuScreen::Home);
    // Entering Settings again starts on the list, not the page left open.
    menu.activate(MenuAction::Navigate(MenuScreen::Settings));
    assert_eq!(menu.view().settings.java_view, JavaSettingsView::Hub);
    // A page opened straight from another screen is left in one step.
    menu.activate(MenuAction::AddBack);
    menu.activate(MenuAction::OpenSettingsSection(video));
    assert_eq!(menu.view().settings.java_view, JavaSettingsView::Direct);
    menu.activate(MenuAction::AddBack);
    assert_eq!(menu.screen(), MenuScreen::Home);
}

#[test]
fn the_bedrock_style_ignores_the_java_settings_view() {
    use launcher::menu::{JavaSettingsAction, JavaSettingsView, settings_sections::SettingSection};

    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    menu.show_home();
    menu.activate(MenuAction::Navigate(MenuScreen::Settings));
    let video = SettingSection::Video.definition().index;
    menu.activate(MenuAction::JavaSettings(JavaSettingsAction::Page(video)));
    assert_eq!(menu.view().settings.java_view, JavaSettingsView::Page);
    // Back leaves Settings at once, as it always did.
    menu.activate(MenuAction::AddBack);
    assert_eq!(menu.screen(), MenuScreen::Home);
}

#[test]
fn arrows_move_a_focused_slider_on_a_java_option_page_without_selecting_it_first() {
    use launcher::menu::{
        JavaSettingsAction, JavaSettingsView,
        settings_options::{SETTINGS_OPTIONS, SettingId, SettingKind, menu_style::MenuStyle},
        settings_sections::SettingSection,
    };

    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    std::sync::Arc::make_mut(&mut menu.settings_options)
        .set(SettingId::MenuStyle.index(), MenuStyle::Java.value());
    let video = SettingSection::Video.definition().index;
    menu.show_home();
    menu.activate(MenuAction::Navigate(MenuScreen::Settings));
    menu.activate(MenuAction::JavaSettings(JavaSettingsAction::Page(video)));
    assert_eq!(menu.view().settings.java_view, JavaSettingsView::Page);
    let index = SETTINGS_OPTIONS
        .iter()
        .position(|option| {
            matches!(option.kind, SettingKind::Slider)
                && option.step > 0
                && option.min < option.max
                && option.id != SettingId::GuiScale
        })
        .expect("a slider option");
    let option = &SETTINGS_OPTIONS[index];
    let index = index as u16;
    menu.settings_input.actions = vec![MenuAction::SettingsOption(index, option.min)];
    menu.focused = 0;
    let before = menu.settings_options.get(usize::from(index));
    menu.move_horizontal_focus(1);
    assert!(menu.settings_options.get(usize::from(index)) > before);
    menu.move_horizontal_focus(-1);
    assert_eq!(menu.settings_options.get(usize::from(index)), before);
}

#[test]
fn the_java_server_tabs_switch_the_list_shown() {
    use launcher::menu::server_list::ServerGroup;

    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    assert_eq!(menu.view().feeds.java_server_group, ServerGroup::Saved);
    menu.activate(MenuAction::JavaServerGroup(ServerGroup::Creator));
    assert_eq!(menu.view().feeds.java_server_group, ServerGroup::Creator);
    menu.activate(MenuAction::JavaServerGroup(ServerGroup::Saved));
    assert_eq!(menu.view().feeds.java_server_group, ServerGroup::Saved);
}

#[test]
fn renaming_a_pack_folder_opens_the_field_and_commits_or_cancels_it() {
    use launcher::global_resources::Action;

    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    let snapshot = launcher::global_resources::Snapshot {
        folders: vec!["PVP".to_owned()],
        folder: "PVP".to_owned(),
        ..Default::default()
    };
    menu.global_resources = std::sync::Arc::new(snapshot);
    menu.activate(MenuAction::GlobalResources(Action::RenameFolder));
    assert!(menu.view().pack_renaming);
    assert_eq!(menu.view().pack_folder_name, "PVP");
    assert_eq!(menu.view().field, Some(MenuField::PackFolderName));
    menu.activate(MenuAction::GlobalResources(Action::CommitRename));
    assert!(!menu.view().pack_renaming);
    assert_eq!(menu.global_resource_renames, ["PVP"]);
    // Cancelling sends nothing.
    menu.activate(MenuAction::GlobalResources(Action::RenameFolder));
    menu.activate(MenuAction::GlobalResources(Action::CancelRename));
    assert!(!menu.view().pack_renaming);
    assert_eq!(menu.global_resource_renames.len(), 1);
    // Outside a folder there is nothing to rename.
    menu.global_resources = std::sync::Arc::new(launcher::global_resources::Snapshot::default());
    menu.activate(MenuAction::GlobalResources(Action::RenameFolder));
    assert!(!menu.view().pack_renaming);
}

#[test]
fn the_java_world_list_chooses_worlds_and_deleting_opens_the_world_then_asks() {
    use launcher::menu::LocalWorldAction;

    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    menu.activate(MenuAction::SelectPlayWorld(1));
    assert_eq!(menu.view().feeds.selected_play_world, Some(1));
    // Pressing the chosen world again lets it go.
    menu.activate(MenuAction::SelectPlayWorld(1));
    assert_eq!(menu.view().feeds.selected_play_world, None);
    // With no such world nothing is queued.
    menu.activate(MenuAction::DeleteLocalWorld(0));
    assert!(menu.queued_local_actions().is_empty());
    menu.local_worlds = vec![launcher::menu::LocalWorldCard {
        name: "Home".to_owned(),
        game_mode: String::new(),
        world_type: String::new(),
        server: String::new(),
        date: String::new(),
        size: String::new(),
        thumbnail: None,
    }]
    .into();
    menu.activate(MenuAction::DeleteLocalWorld(0));
    assert_eq!(
        menu.queued_local_actions(),
        [LocalWorldAction::Edit(0), LocalWorldAction::Delete]
    );
}

#[test]
fn a_second_press_of_the_same_world_soon_after_plays_it() {
    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    let press = |index| MenuAction::SelectPlayWorld(index);
    // The first press only selects; a quick second one on the same world plays it.
    assert_eq!(menu.world_double_press(press(1), 10.0), None);
    assert_eq!(
        menu.world_double_press(press(1), 10.2),
        Some(MenuAction::JoinPlayWorld(1))
    );
    // A double press is used up, so a third does not play again.
    assert_eq!(menu.world_double_press(press(1), 10.3), None);
    // Too slow, or another world, is not a double press.
    assert_eq!(menu.world_double_press(press(1), 11.0), None);
    assert_eq!(menu.world_double_press(press(2), 11.1), None);
    // Other actions are never double presses.
    assert_eq!(menu.world_double_press(MenuAction::AddBack, 11.2), None);
}

#[test]
fn the_play_arrow_joins_a_friends_world_or_an_own_one_by_its_place() {
    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    menu.local_worlds = vec![launcher::menu::LocalWorldCard {
        name: "Home".to_owned(),
        game_mode: String::new(),
        world_type: String::new(),
        server: String::new(),
        date: String::new(),
        size: String::new(),
        thumbnail: None,
    }]
    .into();
    // With no friends, place 0 is the first own world.
    menu.activate(MenuAction::JoinPlayWorld(0));
    assert_eq!(menu.local_world_requested, Some(0));
}

#[test]
fn enter_plays_the_chosen_world_of_the_java_list_and_nothing_else() {
    use launcher::menu::settings_options::{SettingId, menu_style::MenuStyle};

    let mut menu = MenuRuntime::new(true, 2, "Player".into());
    menu.local_worlds = vec![launcher::menu::LocalWorldCard {
        name: "Home".to_owned(),
        game_mode: String::new(),
        world_type: String::new(),
        server: String::new(),
        date: String::new(),
        size: String::new(),
        thumbnail: None,
    }]
    .into();
    menu.show_home();
    menu.activate(MenuAction::Navigate(MenuScreen::Play));
    menu.activate(MenuAction::SelectPlayWorld(0));
    // The Bedrock style leaves Enter alone.
    assert_eq!(menu.enter_plays_world(), None);
    std::sync::Arc::make_mut(&mut menu.settings_options)
        .set(SettingId::MenuStyle.index(), MenuStyle::Java.value());
    assert_eq!(menu.enter_plays_world(), Some(MenuAction::JoinPlayWorld(0)));
    // A chosen world that is gone is not played, and neither is none at all.
    menu.activate(MenuAction::SelectPlayWorld(5));
    assert_eq!(menu.enter_plays_world(), None);
    menu.activate(MenuAction::SelectPlayWorld(5));
    assert_eq!(menu.enter_plays_world(), None);
}
