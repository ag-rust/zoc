use std::sync::mpsc::{channel, Receiver};
use std::f32::consts::{PI};
use rand::{thread_rng, Rng};
use std::iter::IntoIterator;
use cgmath::{self, Array, Vector2, Vector3, Rad};
use glutin::{self, VirtualKeyCode, MouseButton, TouchPhase};
use glutin::ElementState::{Released};
use core;
use core::map::{Terrain};
use core::game_state::{State};
use core::event::{Command, MoveMode, ReactionFireMode};
use core::player::{PlayerId};
use core::object::{Object, ObjectClass};
use core::options::Options as CoreOptions;
use core::position::{self, MapPos, ExactPos, SlotId};
use core::unit::{UnitId, UnitTypeId};
use core::misc::{opt_rx_collect};
use core::print_info::{print_pos_info};
use gui::{ButtonManager, Button, ButtonId, is_tap};
use scene::{Scene, /*NodeId,*/ SceneNode, SceneNodeType};
use action::{self, Action};
use unit_type_visual_info::{
    UnitTypeVisualInfoManager,
    get_unit_type_visual_info
};
use selection::{SelectionManager};
use context::{Context};
use mesh::{MeshId};
use geom;
use screen::{Screen, ScreenCommand, EventStatus};
use context_menu_popup::{self, ContextMenuPopup};
use reinforcements_popup::{self, ReinforcementsPopup};
use end_turn_screen::{EndTurnScreen};
use game_results_screen::{GameResultsScreen};
use types::{Time, ScreenPos, WorldPos};
use gen;
use pick;
use player_info::{PlayerInfoManager, PlayerInfo};
use mesh_manager::{MeshIdManager, MeshManager};
use event_visualizer::{visualize_event};

fn score_text(state: &State) -> String {
    let target_score = state.target_score();
    let score = state.score();
    // TODO: get rid of magic num
    format!("P0:{}/{}, P1:{}/{}",
        score[&PlayerId{id: 0}].n,
        target_score.n,
        score[&PlayerId{id: 1}].n,
        target_score.n,
    )
}

fn reinforcement_points_text(state: &State, player_id: PlayerId) -> String {
    let rp = state.reinforcement_points()[&player_id].n;
    let rp_per_turn = 10; // TODO: magic num
    format!("reinforcements: {} (+{})", rp, rp_per_turn)
}

fn building_mesh_id(mesh_ids: &MeshIdManager, object: &Object) -> MeshId {
    let slot_id = object.pos.slot_id;
    match slot_id {
        SlotId::Id(_) => mesh_ids.building_mesh_id,
        SlotId::WholeTile => mesh_ids.big_building_mesh_id,
        _ => unimplemented!(),
    }
}

fn wireframe_building_mesh_id(mesh_ids: &MeshIdManager, object: &Object) -> MeshId {
    let slot_id = object.pos.slot_id;
    match slot_id {
        SlotId::Id(_) => mesh_ids.building_mesh_w_id,
        SlotId::WholeTile => mesh_ids.big_building_mesh_w_id,
        _ => unimplemented!(),
    }
}

#[derive(Clone, Debug)]
pub struct Gui {
    button_manager: ButtonManager,
    button_end_turn_id: ButtonId,
    button_deselect_unit_id: ButtonId,
    button_next_unit_id: ButtonId,
    button_prev_unit_id: ButtonId,
    button_zoom_in_id: ButtonId,
    button_zoom_out_id: ButtonId,
    label_unit_info_id: Option<ButtonId>,
    label_score_id: ButtonId,
    label_reinforcement_points_id: ButtonId,
}

impl Gui {
    fn new(context: &mut Context, state: &State) -> Gui {
        let mut button_manager = ButtonManager::new();
        let mut pos = ScreenPos{v: Vector2{x: 10, y: 10}};
        let button_end_turn_id = button_manager.add_button(
            Button::new(context, "[end turn]", pos));
        let ystep = (button_manager.buttons()[&button_end_turn_id].size().h as f32 * 1.2) as i32; // TODO
        pos.v.y += ystep;
        let button_deselect_unit_id = button_manager.add_button(
            Button::new(context, "[X]", pos));
        pos.v.x += button_manager.buttons()[&button_deselect_unit_id].size().w;
        let button_prev_unit_id = button_manager.add_button(
            Button::new(context, "[<]", pos));
        pos.v.x += button_manager.buttons()[&button_prev_unit_id].size().w;
        let button_next_unit_id = button_manager.add_button(
            Button::new(context, "[>]", pos));
        pos.v.y += ystep;
        pos.v.x = 10;
        let button_zoom_in_id = button_manager.add_button(
            Button::new(context, "[+]", pos));
        pos.v.x += button_manager.buttons()[&button_prev_unit_id].size().w;
        let button_zoom_out_id = button_manager.add_button(
            Button::new(context, "[-]", pos));
        let label_score_id = {
            let vp_pos = ScreenPos{v: Vector2 {
                x: context.win_size().w - 10,
                y: context.win_size().h - 10,
            }};
            let text = score_text(state);
            let mut label_score = Button::new_small(context, &text, vp_pos);
            let mut pos = label_score.pos();
            pos.v.y -= label_score.size().h;
            pos.v.x -= label_score.size().w;
            label_score.set_pos(pos);
            button_manager.add_button(label_score)
        };
        let label_reinforcement_points_id = {
            let vp_pos = ScreenPos{v: Vector2 {
                x: context.win_size().w - 10,
                y: 10,
            }};
            let text = reinforcement_points_text(state, PlayerId{id: 0}); // TODO: magic player id
            let mut button = Button::new_small(context, &text, vp_pos);
            let mut pos = button.pos();
            pos.v.x -= button.size().w;
            button.set_pos(pos);
            button_manager.add_button(button)
        };
        Gui {
            button_manager: button_manager,
            button_end_turn_id: button_end_turn_id,
            button_deselect_unit_id: button_deselect_unit_id,
            button_prev_unit_id: button_prev_unit_id,
            button_next_unit_id: button_next_unit_id,
            button_zoom_in_id: button_zoom_in_id,
            button_zoom_out_id: button_zoom_out_id,
            label_unit_info_id: None,
            label_score_id: label_score_id,
            label_reinforcement_points_id: label_reinforcement_points_id,
        }
    }
}

// TODO: Use Actions?
fn init_scene(scene: &mut Scene, state: &State, mesh_ids: &MeshIdManager) {
    let map = state.map();
    scene.add_node(SceneNode {
        pos: WorldPos{v: Vector3::from_value(0.0)},
        mesh_id: Some(mesh_ids.walkable_mesh_id),
        color: [0.0, 0.0, 1.0, 1.0],
        .. Default::default()
    });
    scene.add_node(SceneNode {
        pos: WorldPos{v: Vector3::from_value(0.0)},
        mesh_id: Some(mesh_ids.targets_mesh_id),
        color: [1.0, 0.0, 0.0, 1.0],
        .. Default::default()
    });
    scene.add_node(SceneNode {
        pos: WorldPos{v: Vector3::from_value(0.0)},
        mesh_id: Some(mesh_ids.map_mesh_id),
        color: [0.8, 0.9, 0.3, 1.0],
        .. Default::default()
    });
    scene.add_node(SceneNode {
        pos: WorldPos{v: Vector3::from_value(0.0)},
        mesh_id: Some(mesh_ids.water_mesh_id),
        color: [0.6, 0.6, 0.9, 1.0],
        .. Default::default()
    });
    for (&sector_id, &sector_mesh_id) in &mesh_ids.sector_mesh_ids {
        scene.add_sector(sector_id, SceneNode {
            pos: WorldPos{v: Vector3{x: 0.0, y: 0.0, z: 0.015}}, // TODO
            mesh_id: Some(sector_mesh_id),
            color: [1.0, 1.0, 1.0, 0.5],
            node_type: SceneNodeType::Transparent,
            .. Default::default()
        });
    }
    for tile_pos in map.get_iter() {
        if *map.tile(tile_pos) == Terrain::Trees {
            let pos = geom::map_pos_to_world_pos(tile_pos);
            let rot = Rad(thread_rng().gen_range(0.0, PI * 2.0));
            scene.add_node(SceneNode {
                pos: pos,
                rot: rot,
                mesh_id: Some(mesh_ids.trees_mesh_id),
                .. Default::default()
            });
            // TODO: this shadow is just an experiment. it should be added as a child I think
            // TODO: add shadows to other objects as well
            scene.add_node(SceneNode {
                pos: WorldPos{v: pos.v + geom::vec3_z(0.01)},
                mesh_id: Some(mesh_ids.shadow_mesh_id),
                color: [1.0, 0.0, 0.0, 0.9],
                scale: 1.5,
                node_type: SceneNodeType::Transparent,
                .. Default::default()
            });
        }
    }
    for (&object_id, object) in state.objects() {
        match object.class {
            ObjectClass::ReinforcementSector => {
                let mut pos = geom::map_pos_to_world_pos(object.pos.map_pos);
                pos.v.z += 0.01; // TODO: layers
                let mut color = match object.owner_id {
                    Some(player_id) => {
                        gen::get_player_color(player_id)
                    },
                    None => [1.0, 1.0, 1.0, 1.0],
                };
                color[3] = 0.6;
                let node_id = scene.allocate_node_id();
                scene.add_object(node_id, object_id, SceneNode {
                    pos: pos,
                    rot: Rad(thread_rng().gen_range(0.0, PI * 2.0)),
                    mesh_id: Some(mesh_ids.reinforcement_sector_tile_mesh_id),
                    color: color,
                    node_type: SceneNodeType::Transparent,
                    .. Default::default()
                });
            },
            ObjectClass::Building => {
                let pos = geom::exact_pos_to_world_pos(state, object.pos);
                let rot = Rad(thread_rng().gen_range(0.0, PI * 2.0));
                let node_id = scene.allocate_node_id();
                scene.add_object(node_id, object_id, SceneNode {
                    pos: pos,
                    rot: rot,
                    mesh_id: Some(building_mesh_id(mesh_ids, object)),
                    .. Default::default()
                });
                // TODO: this shadow is an experiment. see tree's comment
                scene.add_node(SceneNode {
                    pos: WorldPos{v: pos.v + geom::vec3_z(0.01)},
                    mesh_id: Some(mesh_ids.shadow_mesh_id),
                    color: [1.0, 0.0, 0.0, 0.9],
                    scale: match object.pos.slot_id {
                        SlotId::Id(_) => 1.3,
                        _ => 3.0,
                    },
                    node_type: SceneNodeType::Transparent,
                    .. Default::default()
                });
            }
            ObjectClass::Road => {
                let pos = geom::exact_pos_to_world_pos(state, object.pos);
                let rot = match object.pos.slot_id {
                    SlotId::TwoTiles(dir) => {
                        Rad(dir.to_int() as f32 * PI / 3.0 + PI / 6.0)
                    },
                    _ => panic!(),
                };
                scene.add_node(SceneNode {
                    pos: pos,
                    rot: rot,
                    mesh_id: Some(mesh_ids.road_mesh_id),
                    .. Default::default()
                });
            }
            ObjectClass::Smoke => unimplemented!(),
        }
    }
}

pub struct TacticalScreen {
    gui: Gui,

    core: core::Core,

    // TODO: all CoreEvents must be received as vectors:
    //
    // `fn get_event(&mut self) -> Option<CoreEvent>`
    // must be
    // `fn get_events(&mut self) -> Vec<CoreEvent>`.

    // TODO: think about extracting this field and all
    // connected methods to some ActionInterpreter
    actions: Vec<Box<Action>>,

    // TODO: pack this fields into some `StaticActionContext` struct?
    mesh_ids: MeshIdManager,
    meshes: MeshManager,
    unit_type_visual_info: UnitTypeVisualInfoManager,
    player_info: PlayerInfoManager,

    selected_unit_id: Option<UnitId>,
    selection_manager: SelectionManager,
    context_menu_popup_rx: Option<Receiver<context_menu_popup::Command>>,
    reinforcements_popup_rx: Option<Receiver<(UnitTypeId, ExactPos)>>,
}

impl TacticalScreen {
    pub fn new(context: &mut Context, core_options: &CoreOptions) -> TacticalScreen {
        let core = core::Core::new(core_options);

        // TODO: Do I really need to crate temporary state
        // just to get map's size from it?!
        let tmp_state = State::new_partial(
            core.db().clone(), core_options, PlayerId{id: 0});

        let mut meshes = MeshManager::new();
        let mesh_ids = MeshIdManager::new(context, &mut meshes, &tmp_state);
        let mut player_info = PlayerInfoManager::new(
            core.db().clone(), context, core_options, &mesh_ids);
        let unit_type_visual_info
            = get_unit_type_visual_info(core.db(), context, &mut meshes);
        let gui = Gui::new(context, &player_info.get(core.player_id()).game_state);
        let selection_manager = SelectionManager::new(mesh_ids.selection_marker_mesh_id);
        for player_info in player_info.info.values_mut() {
            init_scene(&mut player_info.scene, &player_info.game_state, &mesh_ids);
        }
        let mut screen = TacticalScreen {
            gui: gui,
            player_info: player_info,
            core: core,
            actions: Vec::new(),
            mesh_ids: mesh_ids,
            meshes: meshes,
            unit_type_visual_info: unit_type_visual_info,
            selected_unit_id: None,
            selection_manager: selection_manager,
            context_menu_popup_rx: None,
            reinforcements_popup_rx: None,
        };
        let actions = screen.make_fow_action_rename_me();
        screen.actions.extend(actions);
        screen
    }

    fn show_reinforcements_menu(&mut self, context: &mut Context, pos: MapPos) {
        let options = reinforcements_popup::get_options(
            self.core.db(),
            self.current_state(),
            self.core.player_id(),
            pos,
        );
        if options == reinforcements_popup::Options::new() {
            return;
        }
        let (tx, rx) = channel();
        let mut menu_pos = ScreenPos{v: Vector2{x: 10, y: 10}};
        menu_pos.v.y = context.win_size().h - menu_pos.v.y;
        let screen = ReinforcementsPopup::new(
            self.core.db(), context, menu_pos, options, tx);
        self.reinforcements_popup_rx = Some(rx);
        context.add_command(ScreenCommand::PushPopup(Box::new(screen)));
        self.deselect_unit(context);
    }

    fn end_turn(&mut self, context: &mut Context) {
        if self.player_info.info.len() > 1 {
            let next_id = self.core.next_player_id(self.core.player_id());
            let screen = Box::new(EndTurnScreen::new(context, next_id));
            context.add_command(ScreenCommand::PushScreen(screen));
        }
        self.deselect_unit(context);
        self.core.do_command(Command::EndTurn);
    }

    fn bobble_helicopters(&mut self, context: &Context, dtime: Time) {
        let i = self.player_info.get_mut(self.core.player_id());
        for (_, unit) in i.game_state.units() {
            let unit_type = self.core.db().unit_type(unit.type_id);
            if unit_type.is_air {
                let node_id = i.scene.unit_id_to_node_id(unit.id);
                let node = i.scene.node_mut(node_id);
                let n = context.current_time().n + unit.id.id as f32;
                node.pos.v.z += (n * 1.5).sin() * 0.4 * dtime.n;
            }
        }
    }

    fn hide_selected_unit_meshes(&mut self, context: &mut Context) {
        let scene = &mut self.player_info.get_mut(self.core.player_id()).scene;
        self.selection_manager.deselect(scene);
        self.meshes.set(self.mesh_ids.walkable_mesh_id, gen::empty_mesh(context));
        self.meshes.set(self.mesh_ids.targets_mesh_id, gen::empty_mesh(context));
    }

    fn deselect_unit(&mut self, context: &mut Context) {
        if let Some(label_id) = self.gui.label_unit_info_id.take() {
            self.gui.button_manager.remove_button(label_id);
        }
        self.selected_unit_id = None;
        self.hide_selected_unit_meshes(context);
    }

    fn current_state(&self) -> &State {
        &self.player_info.get(self.core.player_id()).game_state
    }

    fn current_state_mut(&mut self) -> &mut State {
        &mut self.player_info.get_mut(self.core.player_id()).game_state
    }

    fn current_player_info(&self) -> &PlayerInfo {
        self.player_info.get(self.core.player_id())
    }

    fn current_player_info_mut(&mut self) -> &mut PlayerInfo {
        self.player_info.get_mut(self.core.player_id())
    }

    // TODO: show commands preview
    fn try_create_context_menu_popup(
        &mut self,
        context: &mut Context,
        pos: MapPos,
    ) {
        let options = context_menu_popup::get_options(
            &self.core,
            self.current_player_info(),
            self.selected_unit_id,
            pos,
        );
        if options == context_menu_popup::Options::new() {
            return;
        }
        let mut menu_pos = context.mouse().pos;
        menu_pos.v.y = context.win_size().h - menu_pos.v.y;
        let (tx, rx) = channel();
        let screen = ContextMenuPopup::new(
            self.current_state(),
            self.core.db(),
            context,
            menu_pos,
            options,
            tx,
        );
        self.context_menu_popup_rx = Some(rx);
        context.add_command(ScreenCommand::PushPopup(Box::new(screen)));
    }

    // TODO: add ability to select enemy units
    fn select_unit(&mut self, context: &mut Context, unit_id: UnitId) {
        if self.selected_unit_id.is_some() {
            self.deselect_unit(context);
        }
        self.selected_unit_id = Some(unit_id);
        let mut player_info = self.player_info.get_mut(self.core.player_id());
        let state = &player_info.game_state;
        let pf = &mut player_info.pathfinder;
        pf.fill_map(state, state.unit(unit_id));
        let move_points = state.unit(unit_id).move_points.unwrap();
        let new_walkable_mesh = gen::build_walkable_mesh(
            context, pf, state, move_points);
        self.meshes.set(self.mesh_ids.walkable_mesh_id, new_walkable_mesh);
        let new_targets_mesh = gen::build_targets_mesh(self.core.db(), context, state, unit_id);
        self.meshes.set(self.mesh_ids.targets_mesh_id, new_targets_mesh);
        let scene = &mut player_info.scene;
        self.selection_manager.create_selection_marker(
            &self.unit_type_visual_info, state, scene, unit_id);
        { // TODO: remove superfluous brackets
            let pos = ScreenPos{v: Vector2{x: 10, y: context.win_size().h - 10}};
            let text = {
                let unit = state.unit(unit_id);
                let unit_type = self.core.db().unit_type(unit.type_id);
                // TODO: core.rs: print_unit_info
                format!("MP={}/{}, AP={}/{}, RAP={}/{}, C={}, M={}",
                    if let Some(mp) = unit.move_points { mp.n } else { 0 },
                    unit_type.move_points.n,
                    if let Some(ap) = unit.attack_points { ap.n } else { 0 },
                    unit_type.attack_points.n,
                    if let Some(rap) = unit.reactive_attack_points { rap.n } else { 0 },
                    unit_type.reactive_attack_points.n,
                    unit.count,
                    unit.morale,
                )
                // TODO: print info about unit type and weapon
            };
            let mut unit_info_button = Button::new_small(context, &text, pos);
            let mut pos = unit_info_button.pos();
            pos.v.y -= unit_info_button.size().h;
            unit_info_button.set_pos(pos);
            if let Some(label_id) = self.gui.label_unit_info_id.take() {
                self.gui.button_manager.remove_button(label_id);
            }
            self.gui.label_unit_info_id = Some(self.gui.button_manager.add_button(
                unit_info_button));
        }
    }

    fn move_unit(&mut self, pos: ExactPos, move_mode: MoveMode) {
        let unit_id = self.selected_unit_id.unwrap();
        let i = self.player_info.get_mut(self.core.player_id());
        // TODO: duplicated get_path =\
        let path = i.pathfinder.get_path(pos).unwrap();
        self.core.do_command(Command::Move {
            unit_id: unit_id,
            path: path,
            mode: move_mode,
        });
    }

    fn handle_camera_move(&mut self, context: &Context, pos: ScreenPos) {
        let diff = pos.v - context.mouse().pos.v;
        let camera_move_speed = geom::HEX_EX_RADIUS * 12.0;
        let per_x_pixel = camera_move_speed / (context.win_size().w as f32);
        let per_y_pixel = camera_move_speed / (context.win_size().h as f32);
        let camera = &mut self.current_player_info_mut().camera;
        camera.move_in_direction(Rad(PI), diff.x as f32 * per_x_pixel);
        camera.move_in_direction(Rad(PI * 1.5), diff.y as f32 * per_y_pixel);
    }

    fn handle_camera_rotate(&mut self, context: &Context, pos: ScreenPos) {
        let diff = pos.v - context.mouse().pos.v;
        let per_x_pixel = PI / (context.win_size().w as f32);
        // TODO: get max angles from camera
        let per_y_pixel = (PI / 4.0) / (context.win_size().h as f32);
        let camera = &mut self.current_player_info_mut().camera;
        camera.add_horizontal_angle(Rad(diff.x as f32 * per_x_pixel));
        camera.add_vertical_angle(Rad(diff.y as f32 * per_y_pixel));
    }

    fn handle_event_mouse_move(&mut self, context: &Context, pos: ScreenPos) {
        self.handle_event_mouse_move_platform(context, pos);
    }

    #[cfg(not(target_os = "android"))]
    fn handle_event_mouse_move_platform(&mut self, context: &Context, pos: ScreenPos) {
        if context.mouse().is_left_button_pressed {
            self.handle_camera_move(context, pos);
        } else if context.mouse().is_right_button_pressed {
            self.handle_camera_rotate(context, pos);
        }
    }

    #[cfg(target_os = "android")]
    fn handle_event_mouse_move_platform(&mut self, context: &Context, pos: ScreenPos) {
        if !context.mouse().is_left_button_pressed {
            return;
        }
        if self.must_rotate_camera(context) {
            self.handle_camera_rotate(context, pos);
        } else {
            self.handle_camera_move(context, pos);
        }
    }

    #[cfg(target_os = "android")]
    fn must_rotate_camera(&self, context: &Context) -> bool {
        let win_size = context.win_size();
        if win_size.w > win_size.h {
            context.mouse().last_press_pos.v.x > win_size.w / 2
        } else {
            context.mouse().last_press_pos.v.y < win_size.h / 2
        }
    }

    fn print_info(&mut self, context: &Context) {
        // TODO: move this to `fn Core::get_unit_info(...) -> &str`?
        let pick_result = self.pick_tile(context);
        if let Some(pos) = pick_result {
            print_pos_info(self.core.db(), self.current_state(), pos);
        }
    }

    fn handle_event_key_press(&mut self, context: &mut Context, key: VirtualKeyCode) {
        let camera_move_speed_on_keypress = geom::HEX_EX_RADIUS;
        let s = camera_move_speed_on_keypress;
        match key {
            VirtualKeyCode::Q | VirtualKeyCode::Escape => {
                context.add_command(ScreenCommand::PopScreen);
            },
            VirtualKeyCode::W | VirtualKeyCode::Up => {
                self.current_player_info_mut().camera.move_in_direction(Rad(PI * 1.5), s);
            },
            VirtualKeyCode::S | VirtualKeyCode::Down => {
                self.current_player_info_mut().camera.move_in_direction(Rad(PI * 0.5), s);
            },
            VirtualKeyCode::D | VirtualKeyCode::Right => {
                self.current_player_info_mut().camera.move_in_direction(Rad(PI * 0.0), s);
            },
            VirtualKeyCode::A | VirtualKeyCode::Left => {
                self.current_player_info_mut().camera.move_in_direction(Rad(PI * 1.0), s);
            },
            VirtualKeyCode::I => {
                self.print_info(context);
            },
            VirtualKeyCode::Subtract | VirtualKeyCode::Key1 => {
                self.current_player_info_mut().camera.change_zoom(1.3);
            },
            VirtualKeyCode::Equals | VirtualKeyCode::Key2 => {
                self.current_player_info_mut().camera.change_zoom(0.7);
            },
            _ => println!("Unknown key pressed"),
        }
    }

    /// Left mouse button
    fn handle_event_lmb_release(&mut self, context: &mut Context) {
        if !self.actions.is_empty() || !is_tap(context) {
            return;
        }
        let pick_result = self.pick_tile(context);
        if let Some(button_id) = self.gui.button_manager.get_clicked_button_id(context) {
            self.handle_event_button_press(context, button_id);
        } else if let Some(pick_result) = pick_result {
            self.try_create_context_menu_popup(context, pick_result);
        }
    }

    fn handle_event_button_press(&mut self, context: &mut Context, button_id: ButtonId) {
        if button_id == self.gui.button_end_turn_id {
            self.end_turn(context);
        } else if button_id == self.gui.button_deselect_unit_id {
            self.deselect_unit(context);
        } else if button_id == self.gui.button_prev_unit_id {
            if let Some(id) = self.selected_unit_id {
                let prev_id = position::find_prev_player_unit_id(
                    self.current_state(), self.core.player_id(), id);
                self.select_unit(context, prev_id);
            }
        } else if button_id == self.gui.button_next_unit_id {
            if let Some(id) = self.selected_unit_id {
                let next_id = position::find_next_player_unit_id(
                    self.current_state(), self.core.player_id(), id);
                self.select_unit(context, next_id);
            }
        } else if button_id == self.gui.button_zoom_in_id {
            self.current_player_info_mut().camera.change_zoom(0.7);
        } else if button_id == self.gui.button_zoom_out_id {
            self.current_player_info_mut().camera.change_zoom(1.3);
        }
    }

    fn scene(&self) -> &Scene {
        &self.player_info.get(self.core.player_id()).scene
    }

    fn draw_scene_node(
        &self,
        context: &mut Context,
        node: &SceneNode,
        mat: cgmath::Matrix4<f32>,
    ) {
        let minimum_alpha = 0.01;
        if node.color[3] < minimum_alpha {
            return;
        }
        let tr_mat = cgmath::Matrix4::from_translation(node.pos.v);
        let rot_mat = cgmath::Matrix4::from(cgmath::Matrix3::from_angle_z(node.rot));
        let scale_mat = cgmath::Matrix4::from_scale(node.scale);
        let mat = mat * tr_mat * rot_mat * scale_mat;
        if let Some(mesh_id) = node.mesh_id {
            context.set_mvp(mat); // TODO: use separate model matrix
            context.set_basic_color(node.color);
            context.draw_mesh(self.meshes.get(mesh_id));
        }
        for &node_id in &node.children {
            let node = self.scene().node(node_id);
            self.draw_scene_node(context, node, mat);
        }
    }

    fn sort_nodes(&mut self) {
        let i = self.player_info.get_mut(self.core.player_id());
        i.scene.sort_transparent_nodes(i.camera.pos());
    }

    fn draw_scene(&mut self, context: &mut Context) {
        self.sort_nodes();
        let mat = self.current_player_info().camera.mat();
        for &node_id in self.scene().normal_node_ids() {
            self.draw_scene_node(context, self.scene().node(node_id), mat);
        }
        for &node_id in self.scene().static_plane_node_ids() {
            self.draw_scene_node(context, self.scene().node(node_id), mat);
        }
        for &node_id in self.scene().transparent_node_ids() {
            self.draw_scene_node(context, self.scene().node(node_id), mat);
        }
    }

    fn draw(&mut self, context: &mut Context) {
        context.clear();
        self.draw_scene(context);
        context.set_basic_color([0.0, 0.0, 0.0, 1.0]);
        self.gui.button_manager.draw(context);
    }

    fn pick_tile(&self, context: &Context) -> Option<MapPos> {
        let camera = &self.current_player_info().camera;
        let state = self.current_state();
        pick::pick_tile(context, state, camera)
    }

    /// handle case when attacker == selected_unit and it dies from reaction fire
    fn attacker_died_from_reaction_fire(&mut self) {
        // TODO: resurrect this
        /*
        let attack_info = match self.event {
            Some(CoreEvent{event: Event::AttackUnit{ref attack_info}, ..}) => attack_info,
            _ => return,
        };
        let i = self.player_info.get_mut(self.core.player_id());
        let state = &player_info.game_state;
        let selected_unit_id = match self.selected_unit_id {
            Some(id) => id,
            None => return,
        };
        let defender = state.unit(attack_info.defender_id);
        if selected_unit_id == attack_info.defender_id
            && defender.count - attack_info.killed <= 0
        {
            self.selected_unit_id = None;
        }
        */
    }

    fn check_game_end(&mut self, context: &mut Context) {
        let state = self.current_state();
        for score in state.score().values() {
            if score.n >= state.target_score().n {
                context.add_command(ScreenCommand::PopScreen);
                let screen = Box::new(GameResultsScreen::new(context, self.current_state()));
                context.add_command(ScreenCommand::PushScreen(screen));
            }
        }
    }

    fn update_score_labels(&mut self, context: &mut Context) {
        let pos = self.gui.button_manager.buttons()[&self.gui.label_score_id].pos();
        let label_score = Button::new_small(context, &score_text(self.current_state()), pos);
        self.gui.button_manager.remove_button(self.gui.label_score_id);
        self.gui.label_score_id = self.gui.button_manager.add_button(label_score);
    }

    fn update_reinforcement_points_label(&mut self, context: &mut Context) {
        let id = self.gui.label_reinforcement_points_id;
        let pos = self.gui.button_manager.buttons()[&id].pos();
        let text = reinforcement_points_text(self.current_state(), self.core.player_id());
        let label = Button::new_small(context, &text, pos);
        self.gui.button_manager.remove_button(id);
        self.gui.label_reinforcement_points_id = self.gui.button_manager.add_button(label);
    }

    fn switch_wireframe(&mut self) {
        let i = self.player_info.get_mut(self.core.player_id());
        let scene = &mut i.scene;
        let state = &mut i.game_state;
        'object_loop: for (&object_id, object) in state.objects() {
            if object.class != ObjectClass::Building {
                continue;
            }
            let node = {
                let node_ids = scene.object_id_to_node_id(object_id).clone();
                assert_eq!(node_ids.len(), 1);
                let node_id = node_ids.into_iter().next().unwrap();
                scene.node_mut(node_id)
            };
            for (_, unit) in state.units() {
                let unit_type = self.core.db().unit_type(unit.type_id);
                if unit_type.is_air {
                    continue;
                }
                if position::is_unit_in_object(unit, object) {
                    node.mesh_id = Some(wireframe_building_mesh_id(&self.mesh_ids, object));
                    node.color = [0.0, 0.0, 0.0, 1.0];
                    continue 'object_loop;
                }
            }
            node.mesh_id = Some(building_mesh_id(&self.mesh_ids, object));
            node.color = [1.0, 1.0, 1.0, 1.0];
        }
    }

    // TODO: rename as this functions is now called only when
    // all the actions are finished
    fn end_action(&mut self, context: &mut Context) {
        self.attacker_died_from_reaction_fire(); // TODO: is this a good place to call it?
        self.switch_wireframe();
        if let Some(label_id) = self.gui.label_unit_info_id.take() {
            self.gui.button_manager.remove_button(label_id);
        }
        self.update_reinforcement_points_label(context);
        if let Some(unit_id) = self.selected_unit_id {
            self.select_unit(context, unit_id);
        }
    }

    // TODO: rename
    fn make_fow_action_rename_me(&mut self) -> Vec<Box<action::Action>> {
        let mut actions: Vec<Box<action::Action>> = Vec::new();
        let i = self.player_info.get_mut(self.core.player_id());
        let fow = &mut i.fow_map;
        let state = &i.game_state;
        for pos in state.map().get_iter() {
            let is_visible = state.is_ground_tile_visible(pos);
            let old_tile_info = fow.tile(pos).clone(); // TODO: rename?
            let duration = Time{n: 0.2};
            if is_visible && !old_tile_info.is_visible {
                let color = [0.0, 0.0, 0.0, 0.0];
                actions.push(Box::new(action::Fork::new(
                    Box::new(action::ChangeColor::new(
                        old_tile_info.node_id, color, duration)))));
            } else if !is_visible && old_tile_info.is_visible {
                let color = [0.0, 0.0, 0.0, 0.4]; // TODO: use the same constant
                actions.push(Box::new(action::Fork::new(
                    Box::new(action::ChangeColor::new(
                        old_tile_info.node_id, color, duration)))));
            }
            fow.tile_mut(pos).is_visible = is_visible;
        }
        actions
    }

    // TODO: rename
    fn try_get_new_action_rename_me(&mut self, context: &mut Context) {
        let mut actions: Vec<Box<Action>> = Vec::new();
        while let Some(event) = self.core.get_event() {
            let action = {
                let i = self.player_info.get_mut(self.core.player_id());
                let action_context = &mut action::ActionContext {
                    context: context,
                    scene: &mut i.scene,
                    camera: &i.camera,
                    meshes: &mut self.meshes,
                    mesh_ids: &self.mesh_ids,
                    visual_info: &self.unit_type_visual_info,
                };
                visualize_event(&i.game_state, action_context, &event)
            };
            self.current_state_mut().apply_event(&event);
            if let Some(action) = action {
                actions.push(action);
            }
            actions.extend(self.make_fow_action_rename_me());
        }
        if !actions.is_empty() {
            self.hide_selected_unit_meshes(context);

            // TODO: reduce code duplication
            let i = self.player_info.get_mut(self.core.player_id());
            let action_context = &mut action::ActionContext {
                context: context,
                scene: &mut i.scene,
                camera: &i.camera,
                meshes: &mut self.meshes,
                mesh_ids: &self.mesh_ids,
                visual_info: &self.unit_type_visual_info,
            };
            let mut action_sequence = if actions.len() > 1 {
                Box::new(action::Sequence::new(actions))
            } else {
                actions.pop().unwrap()
            };
            action_sequence.begin(action_context);
            self.actions.push(action_sequence);
        }
    }

    fn try_fork_action(&mut self, context: &mut Context) {
        let mut forked_actions = Vec::new();
        for action in &mut self.actions {
            if let Some(action) = action.fork() {
                forked_actions.push(action);
            }
        }
        for mut forked_action in forked_actions {
            // TODO: duplication
            let i = self.player_info.get_mut(self.core.player_id());
            let action_context = &mut action::ActionContext {
                context: context,
                scene: &mut i.scene,
                camera: &i.camera,
                meshes: &mut self.meshes,
                mesh_ids: &self.mesh_ids,
                visual_info: &self.unit_type_visual_info,
            };
            forked_action.begin(action_context);
            self.actions.push(forked_action);
        }
    }

    // TODO: rename
    fn try_end_action(&mut self, context: &mut Context) {
        for action in &mut self.actions {
            if action.is_finished() {
                let i = self.player_info.get_mut(self.core.player_id());
                let action_context = &mut action::ActionContext {
                    context: context,
                    scene: &mut i.scene,
                    camera: &i.camera,
                    meshes: &mut self.meshes,
                    mesh_ids: &self.mesh_ids,
                    visual_info: &self.unit_type_visual_info,
                };
                action.end(action_context);
            }
        }
        self.actions.retain(|action| !action.is_finished());
    }

    // TODO: teach this thing to fastforward action::Fork events in one frame
    fn update_actions(&mut self, context: &mut Context, dtime: Time) {
        if self.actions.is_empty() {
            self.try_get_new_action_rename_me(context);
        }
        for action in &mut self.actions {
            let i = self.player_info.get_mut(self.core.player_id());
            let action_context = &mut action::ActionContext {
                context: context,
                scene: &mut i.scene,
                camera: &i.camera,
                meshes: &mut self.meshes,
                mesh_ids: &self.mesh_ids,
                visual_info: &self.unit_type_visual_info,
            };
            action.update(action_context, dtime);
        }
        self.try_fork_action(context);
        self.try_end_action(context);
        if self.actions.is_empty() {
            self.end_action(context);
            // TODO: should this really be executed every frame? =\
            // Maybe it should be moved to end_action func
            self.update_score_labels(context);
            self.check_game_end(context);
        }
    }

    fn update(&mut self, context: &mut Context, dtime: Time) {
        self.update_actions(context, dtime);
        self.bobble_helicopters(context, dtime);
        // TODO: sort transparent nodes HERE, not in draw()
    }

    fn handle_context_menu_popup_command(
        &mut self,
        context: &mut Context,
        command: context_menu_popup::Command,
    ) {
        if let context_menu_popup::Command::Select{id} = command {
            self.select_unit(context, id);
            return;
        }
        match command {
            context_menu_popup::Command::Select{id} => {
                self.select_unit(context, id);
            },
            context_menu_popup::Command::Move{pos} => {
                self.move_unit(pos, MoveMode::Fast);
            },
            context_menu_popup::Command::Hunt{pos} => {
                self.move_unit(pos, MoveMode::Hunt);
            },
            context_menu_popup::Command::Attack{id} => {
                let selected_unit_id = self.selected_unit_id.unwrap();
                self.core.do_command(Command::AttackUnit {
                    attacker_id: selected_unit_id,
                    defender_id: id,
                });
            },
            context_menu_popup::Command::LoadUnit{passenger_id} => {
                let selected_unit_id = self.selected_unit_id.unwrap();
                self.core.do_command(Command::LoadUnit {
                    transporter_id: selected_unit_id,
                    passenger_id: passenger_id,
                });
            },
            context_menu_popup::Command::UnloadUnit{pos} => {
                let selected_unit_id = self.selected_unit_id.unwrap();
                let passenger_id = {
                    let transporter = self.current_state()
                        .unit(selected_unit_id);
                    transporter.passenger_id.unwrap()
                };
                self.core.do_command(Command::UnloadUnit {
                    transporter_id: selected_unit_id,
                    passenger_id: passenger_id,
                    pos: pos,
                });
            },
            context_menu_popup::Command::Attach{attached_unit_id} => {
                let selected_unit_id = self.selected_unit_id.unwrap();
                self.core.do_command(Command::Attach {
                    transporter_id: selected_unit_id,
                    attached_unit_id: attached_unit_id,
                });
            },
            context_menu_popup::Command::Detach{pos} => {
                self.core.do_command(Command::Detach {
                    transporter_id: self.selected_unit_id.unwrap(),
                    pos: pos,
                });
            },
            context_menu_popup::Command::EnableReactionFire{id} => {
                self.core.do_command(Command::SetReactionFireMode {
                    unit_id: id,
                    mode: ReactionFireMode::Normal,
                });
            },
            context_menu_popup::Command::DisableReactionFire{id} => {
                self.core.do_command(Command::SetReactionFireMode {
                    unit_id: id,
                    mode: ReactionFireMode::HoldFire,
                });
            },
            context_menu_popup::Command::Smoke{pos} => {
                let selected_unit_id = self.selected_unit_id.unwrap();
                self.core.do_command(Command::Smoke {
                    unit_id: selected_unit_id,
                    pos: pos,
                });
            },
            context_menu_popup::Command::CallReiforcements{pos} => { // TODO: fix typo
                self.show_reinforcements_menu(context, pos);
            },
        }
    }

    fn handle_reinforce_command(&mut self, type_id: UnitTypeId, pos: ExactPos) {
        self.core.do_command(Command::CreateUnit {
            pos: pos,
            type_id: type_id,
        });
    }

    fn handle_context_menu_popup_commands(&mut self, context: &mut Context) {
        for command in opt_rx_collect(&self.context_menu_popup_rx) {
            self.handle_context_menu_popup_command(context, command);
        }
        for (type_id, pos) in opt_rx_collect(&self.reinforcements_popup_rx) {
            self.handle_reinforce_command(type_id, pos);
        }
    }
}

impl Screen for TacticalScreen {
    fn tick(&mut self, context: &mut Context, dtime: Time) {
        self.update(context, dtime);
        self.draw(context);
        self.handle_context_menu_popup_commands(context);
    }

    fn handle_event(&mut self, context: &mut Context, event: &glutin::Event) -> EventStatus {
        match *event {
            glutin::Event::Resized(..) => {
                for player_info in self.player_info.info.values_mut() {
                    player_info.camera.regenerate_projection_mat(context.win_size());
                }
            },
            glutin::Event::MouseMoved(x, y) => {
                let pos = ScreenPos{v: Vector2{x: x as i32, y: y as i32}};
                self.handle_event_mouse_move(context, pos);
            },
            glutin::Event::MouseInput(Released, MouseButton::Left) => {
                self.handle_event_lmb_release(context);
            },
            glutin::Event::KeyboardInput(Released, _, Some(key)) => {
                self.handle_event_key_press(context, key);
            },
            glutin::Event::Touch(glutin::Touch{location: (x, y), phase, ..}) => {
                let pos = ScreenPos{v: Vector2{x: x as i32, y: y as i32}};
                match phase {
                    TouchPhase::Started | TouchPhase::Moved => {
                        self.handle_event_mouse_move(context, pos);
                    },
                    TouchPhase::Ended => {
                        self.handle_event_mouse_move(context, pos);
                        self.handle_event_lmb_release(context);
                    },
                    TouchPhase::Cancelled => {
                        unimplemented!();
                    },
                }
            },
            _ => {},
        }
        EventStatus::Handled
    }
}
