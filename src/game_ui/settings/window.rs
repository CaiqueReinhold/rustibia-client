use bevy::prelude::*;
use bevy::ui_widgets::{
    Slider, SliderPrecision, SliderRange, SliderStep, SliderThumb, SliderValue, TrackClick,
    ValueChange,
};

use crate::conf::ui::chat::{TAB_TITLE_COLOR, TAB_TITLE_COLOR_INACTIVE};
use crate::conf::ui::dialog::{BUTTON_COLOR, FIELD_BG_COLOR};
use crate::conf::ui::options as conf;
use crate::conf::ui::ui_colors;
use crate::game_ui::{
    DialogButton, DialogButtonId, DialogButtonPressed, GameUiAssets, ModalDialog, ModalOrder,
};
use crate::items::thumb_left;

use super::state::Settings;

const WIDTH: f32 = 320.0;
const TAB_HEIGHT: f32 = 20.0;
const ROW_HEIGHT: f32 = 18.0;
const BOX_SIZE: f32 = 12.0;
const FONT_SIZE: f32 = 11.0;
const PAGE_MIN_HEIGHT: f32 = 110.0;
const KEY_REPEAT_MS_STEP: u16 = 10;

const THUMB_WIDTH: f32 = 10.0;
const TRACK_HEIGHT: f32 = 12.0;
const READOUT_WIDTH: f32 = 48.0;

#[derive(Event, Debug)]
pub struct OpenOptionsWindow;

#[derive(Component)]
pub(super) struct OptionsWindow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Page {
    Interface,
    Graphics,
    Controls,
}

impl Page {
    const ALL: [Page; 3] = [Page::Interface, Page::Graphics, Page::Controls];

    fn label(self) -> &'static str {
        match self {
            Page::Interface => "Interface",
            Page::Graphics => "Graphics",
            Page::Controls => "Controls",
        }
    }
}

#[derive(Component)]
pub(super) struct OptionsTab(Page);

#[derive(Component)]
pub(super) struct OptionsPage(Page);

#[derive(Event, Debug)]
pub(super) struct SelectOptionsPage(pub(super) Page);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Toggle {
    ShowPing,
    ShowNames,
    ShowHealthBars,
    ShowDamageNumbers,
    ShowSpeech,
    ShowFps,
    Vsync,
}

impl Toggle {
    pub(super) fn field(self, settings: &mut Settings) -> &mut bool {
        match self {
            Toggle::ShowPing => &mut settings.show_ping,
            Toggle::ShowNames => &mut settings.show_names,
            Toggle::ShowHealthBars => &mut settings.show_health_bars,
            Toggle::ShowDamageNumbers => &mut settings.show_damage_numbers,
            Toggle::ShowSpeech => &mut settings.show_speech,
            Toggle::ShowFps => &mut settings.show_fps,
            Toggle::Vsync => &mut settings.vsync,
        }
    }

    fn value(self, settings: &Settings) -> bool {
        *self.field(&mut settings.clone())
    }
}

const ROWS: [(Toggle, Page, &str); 7] = [
    (Toggle::ShowPing, Page::Interface, "Show ping"),
    (Toggle::ShowNames, Page::Interface, "Show names"),
    (Toggle::ShowHealthBars, Page::Interface, "Show health bars"),
    (
        Toggle::ShowDamageNumbers,
        Page::Interface,
        "Show damage numbers",
    ),
    (Toggle::ShowSpeech, Page::Interface, "Show speech"),
    (Toggle::ShowFps, Page::Graphics, "Show FPS"),
    (Toggle::Vsync, Page::Graphics, "VSync"),
];

#[derive(Component)]
pub(super) struct ToggleTick(Toggle);

#[derive(Event, Debug)]
pub(super) struct FlipSetting(pub(super) Toggle);

#[derive(Component)]
pub(super) struct KeyRepeatSlider;

#[derive(Component)]
pub(super) struct KeyRepeatReadout;

pub(super) fn key_repeat_ms(value: f32) -> u16 {
    if !value.is_finite() {
        return conf::KEY_REPEAT_MS_DEFAULT;
    }
    value.round().clamp(
        conf::KEY_REPEAT_MS_MIN as f32,
        conf::KEY_REPEAT_MS_MAX as f32,
    ) as u16
}

fn readout_label(ms: u16) -> String {
    format!("{ms} ms")
}

fn page_display(page: Page, active: Page) -> Display {
    if page == active {
        Display::Flex
    } else {
        Display::None
    }
}

fn tab_color(page: Page, active: Page) -> Color {
    if page == active {
        TAB_TITLE_COLOR.into()
    } else {
        TAB_TITLE_COLOR_INACTIVE.into()
    }
}

fn label_text(ui_assets: &GameUiAssets, text: impl Into<String>, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: ui_assets.font.clone(),
            font_size: FONT_SIZE,
            ..default()
        },
        TextColor(color),
    )
}

fn raised_border() -> BorderColor {
    BorderColor {
        top: ui_colors::LIGHT_BORDER_COLOR.into(),
        right: ui_colors::DARK_BORDER_COLOR.into(),
        bottom: ui_colors::DARK_BORDER_COLOR.into(),
        left: ui_colors::LIGHT_BORDER_COLOR.into(),
    }
}

fn sunken_border() -> BorderColor {
    BorderColor {
        top: ui_colors::DARK_BORDER_COLOR.into(),
        right: ui_colors::LIGHT_BORDER_COLOR.into(),
        bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
        left: ui_colors::DARK_BORDER_COLOR.into(),
    }
}

fn spawn_tab_strip(commands: &mut Commands, ui_assets: &GameUiAssets) -> Entity {
    let strip = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    for page in Page::ALL {
        let tab = commands
            .spawn((
                OptionsTab(page),
                Button,
                Node {
                    height: Val::Px(TAB_HEIGHT),
                    padding: UiRect::horizontal(Val::Px(8.0)),
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                raised_border(),
                BackgroundColor(BUTTON_COLOR.into()),
                Children::spawn(Spawn(label_text(
                    ui_assets,
                    page.label(),
                    tab_color(page, Page::Interface),
                ))),
            ))
            .observe(
                move |mut event: On<Pointer<Click>>, mut commands: Commands| {
                    event.propagate(false);
                    commands.trigger(SelectOptionsPage(page));
                },
            )
            .id();
        commands.entity(strip).add_child(tab);
    }
    strip
}

fn spawn_page(commands: &mut Commands, page: Page) -> Entity {
    commands
        .spawn((
            OptionsPage(page),
            Node {
                display: page_display(page, Page::Interface),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                min_height: Val::Px(PAGE_MIN_HEIGHT),
                padding: UiRect::top(Val::Px(8.0)),
                ..default()
            },
        ))
        .id()
}

fn spawn_toggle_row(
    commands: &mut Commands,
    ui_assets: &GameUiAssets,
    toggle: Toggle,
    label: &str,
) -> Entity {
    commands
        .spawn((
            Button,
            Node {
                height: Val::Px(ROW_HEIGHT),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Children::spawn((
                Spawn((
                    Node {
                        width: Val::Px(BOX_SIZE),
                        height: Val::Px(BOX_SIZE),
                        border: UiRect::all(Val::Px(1.0)),
                        padding: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    sunken_border(),
                    BackgroundColor(FIELD_BG_COLOR.into()),
                    Children::spawn(Spawn((
                        ToggleTick(toggle),
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(ui_colors::FONT_COLOR_CONTENT.into()),
                    ))),
                )),
                Spawn(label_text(
                    ui_assets,
                    label,
                    ui_colors::FONT_COLOR_CONTENT.into(),
                )),
            )),
        ))
        .observe(
            move |mut event: On<Pointer<Click>>, mut commands: Commands| {
                event.propagate(false);
                commands.trigger(FlipSetting(toggle));
            },
        )
        .id()
}

fn spawn_key_repeat_row(
    commands: &mut Commands,
    ui_assets: &GameUiAssets,
    settings: &Settings,
) -> Entity {
    let caption = commands
        .spawn(label_text(
            ui_assets,
            "Key repeat interval",
            ui_colors::FONT_COLOR_CONTENT.into(),
        ))
        .id();
    let slider = commands
        .spawn((
            KeyRepeatSlider,
            Slider {
                track_click: TrackClick::Snap,
            },
            SliderValue(settings.key_repeat_ms as f32),
            SliderRange::new(
                conf::KEY_REPEAT_MS_MIN as f32,
                conf::KEY_REPEAT_MS_MAX as f32,
            ),
            SliderStep(KEY_REPEAT_MS_STEP as f32),
            SliderPrecision(0),
            Node {
                flex_grow: 1.0,
                height: Val::Px(TRACK_HEIGHT),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            sunken_border(),
            BackgroundColor(FIELD_BG_COLOR.into()),
            Children::spawn(Spawn((
                SliderThumb,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(THUMB_WIDTH),
                    height: Val::Percent(100.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                raised_border(),
                BackgroundColor(BUTTON_COLOR.into()),
            ))),
        ))
        .observe(on_key_repeat_slider)
        .id();
    let readout = commands
        .spawn((
            KeyRepeatReadout,
            label_text(
                ui_assets,
                readout_label(settings.key_repeat_ms),
                ui_colors::FONT_COLOR_CONTENT.into(),
            ),
            TextLayout::new_with_justify(Justify::Right),
            Node {
                width: Val::Px(READOUT_WIDTH),
                ..default()
            },
        ))
        .id();
    let track_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .add_children(&[slider, readout])
        .id();
    commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .add_children(&[caption, track_row])
        .id()
}

fn on_key_repeat_slider(
    change: On<ValueChange<f32>>,
    mut commands: Commands,
    mut settings: ResMut<Settings>,
) {
    commands
        .entity(change.source)
        .insert(SliderValue(change.value));
    settings.key_repeat_ms = key_repeat_ms(change.value);
}

pub(super) fn on_open_options_window(
    _event: On<OpenOptionsWindow>,
    mut commands: Commands,
    ui_assets: Res<GameUiAssets>,
    mut order: ResMut<ModalOrder>,
    settings: Res<Settings>,
    existing: Query<(), With<OptionsWindow>>,
) {
    if !existing.is_empty() {
        return;
    }

    let handle = ModalDialog::new("Options")
        .with_width(Val::Px(WIDTH))
        .with_buttons([DialogButton {
            id: DialogButtonId::Ok,
            label: "Close",
        }])
        .spawn(&mut commands, &ui_assets, &mut order);
    commands.entity(handle.root).insert(OptionsWindow);

    let strip = spawn_tab_strip(&mut commands, &ui_assets);
    let pages = Page::ALL.map(|page| spawn_page(&mut commands, page));
    for (toggle, page, label) in ROWS {
        let row = spawn_toggle_row(&mut commands, &ui_assets, toggle, label);
        commands.entity(pages[page as usize]).add_child(row);
    }
    let key_repeat = spawn_key_repeat_row(&mut commands, &ui_assets, &settings);
    commands
        .entity(pages[Page::Controls as usize])
        .add_child(key_repeat);

    commands
        .entity(handle.content)
        .add_child(strip)
        .add_children(&pages);
}

pub(super) fn on_select_options_page(
    event: On<SelectOptionsPage>,
    mut pages_q: Query<(&OptionsPage, &mut Node)>,
    tabs_q: Query<(&OptionsTab, &Children)>,
    mut text_q: Query<&mut TextColor>,
) {
    let active = event.0;
    for (page, mut node) in &mut pages_q {
        node.display = page_display(page.0, active);
    }
    for (tab, children) in &tabs_q {
        for child in children.iter() {
            if let Ok(mut color) = text_q.get_mut(child) {
                color.0 = tab_color(tab.0, active);
            }
        }
    }
}

pub(super) fn on_flip_setting(event: On<FlipSetting>, mut settings: ResMut<Settings>) {
    let field = event.0.field(&mut settings);
    *field = !*field;
}

pub(super) fn on_options_button(
    event: On<DialogButtonPressed>,
    mut commands: Commands,
    dialog_q: Query<(), With<OptionsWindow>>,
) {
    if dialog_q.contains(event.dialog) {
        commands.entity(event.dialog).despawn();
    }
}

pub(super) fn sync_toggle_ticks(
    settings: Res<Settings>,
    mut ticks_q: Query<(&ToggleTick, &mut Node)>,
) {
    for (tick, mut node) in &mut ticks_q {
        let display = if tick.0.value(&settings) {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
}

pub(super) fn sync_options_slider(
    settings: Res<Settings>,
    slider_q: Query<(&SliderValue, &SliderRange, &ComputedNode, &Children), With<KeyRepeatSlider>>,
    mut thumb_q: Query<(&mut Node, &ComputedNode), With<SliderThumb>>,
    mut readout_q: Query<&mut Text, With<KeyRepeatReadout>>,
) {
    for (value, range, slider_node, children) in &slider_q {
        for child in children.iter() {
            let Ok((mut thumb, thumb_node)) = thumb_q.get_mut(child) else {
                continue;
            };
            let left = thumb_left(range, value.0, slider_node, thumb_node);
            if thumb.left != left {
                thumb.left = left;
            }
        }
    }
    let label = readout_label(settings.key_repeat_ms);
    for mut text in &mut readout_q {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_ui::assets::{UiButtons, UiInventory, UiWindow};
    use bevy::ecs::system::RunSystemOnce;

    fn options_world() -> World {
        let mut world = World::new();
        world.insert_resource(GameUiAssets {
            font: Handle::default(),
            name_font: Handle::default(),
            window: UiWindow::default(),
            inventory: UiInventory::default(),
            background_dark: Handle::default(),
            background_light: Handle::default(),
            bar_overlay: Handle::default(),
            title_background: Handle::default(),
            buttons: UiButtons::default(),
        });
        world.init_resource::<ModalOrder>();
        world.init_resource::<Settings>();
        world.add_observer(on_open_options_window);
        world.add_observer(on_select_options_page);
        world.add_observer(on_flip_setting);
        world.add_observer(on_options_button);
        world
    }

    fn open(world: &mut World) -> Entity {
        world.trigger(OpenOptionsWindow);
        world.flush();
        let mut query = world.query_filtered::<Entity, With<OptionsWindow>>();
        query.single(world).unwrap()
    }

    fn displayed_pages(world: &mut World) -> Vec<Page> {
        let mut query = world.query::<(&OptionsPage, &Node)>();
        query
            .iter(world)
            .filter(|(_, node)| node.display == Display::Flex)
            .map(|(page, _)| page.0)
            .collect()
    }

    #[test]
    fn a_second_open_does_not_spawn_a_second_modal() {
        let mut world = options_world();
        open(&mut world);

        world.trigger(OpenOptionsWindow);
        world.flush();

        let mut query = world.query_filtered::<(), With<OptionsWindow>>();
        assert_eq!(query.iter(&world).count(), 1);
    }

    #[test]
    fn the_modal_opens_on_the_interface_tab() {
        let mut world = options_world();
        open(&mut world);

        assert_eq!(displayed_pages(&mut world), vec![Page::Interface]);
    }

    #[test]
    fn selecting_a_tab_displays_exactly_its_page() {
        let mut world = options_world();
        open(&mut world);

        world.trigger(SelectOptionsPage(Page::Graphics));
        world.flush();

        assert_eq!(displayed_pages(&mut world), vec![Page::Graphics]);
    }

    #[test]
    fn every_row_names_a_distinct_field() {
        let flipped: Vec<Settings> = ROWS
            .iter()
            .map(|(toggle, _, _)| {
                let mut settings = Settings::default();
                let field = toggle.field(&mut settings);
                *field = !*field;
                settings
            })
            .collect();

        for (i, a) in flipped.iter().enumerate() {
            assert_ne!(*a, Settings::default(), "{:?}", ROWS[i].0);
            for b in &flipped[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn flipping_a_setting_changes_only_that_field() {
        let mut world = options_world();

        world.trigger(FlipSetting(Toggle::ShowSpeech));
        world.flush();

        assert_eq!(
            *world.resource::<Settings>(),
            Settings {
                show_speech: false,
                ..default()
            }
        );
    }

    #[test]
    fn a_tick_shows_exactly_when_its_setting_is_on() {
        let mut world = options_world();
        open(&mut world);
        world.resource_mut::<Settings>().show_fps = true;

        world.run_system_once(sync_toggle_ticks).unwrap();

        let mut query = world.query::<(&ToggleTick, &Node)>();
        for (tick, node) in query.iter(&world) {
            let expected = if tick.0.value(&Settings {
                show_fps: true,
                ..default()
            }) {
                Display::Flex
            } else {
                Display::None
            };
            assert_eq!(node.display, expected, "{:?}", tick.0);
        }
    }

    #[test]
    fn close_despawns_the_modal() {
        let mut world = options_world();
        let root = open(&mut world);

        world.trigger(DialogButtonPressed {
            dialog: root,
            button: DialogButtonId::Ok,
        });
        world.flush();

        let mut query = world.query_filtered::<(), With<OptionsWindow>>();
        assert_eq!(query.iter(&world).count(), 0);
    }

    #[test]
    fn a_slider_position_becomes_a_whole_interval_in_range() {
        assert_eq!(key_repeat_ms(123.6), 124);
        assert_eq!(key_repeat_ms(10.0), 50);
        assert_eq!(key_repeat_ms(9000.0), 500);
        assert_eq!(key_repeat_ms(f32::NAN), 200);
    }
}
