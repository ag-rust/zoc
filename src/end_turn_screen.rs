use cgmath::{Vector2};
use glutin::{self, WindowEvent, MouseButton, VirtualKeyCode};
use glutin::ElementState::{Released};
use core::player::{PlayerId};
use screen::{Screen, ScreenCommand, EventStatus};
use context::{Context};
use gui::{ButtonManager, Button, is_tap};
use types::{ScreenPos, Time};

#[derive(Clone, Debug)]
pub struct EndTurnScreen {
    button_manager: ButtonManager,
}

impl EndTurnScreen {
    pub fn new(
        context: &mut Context,
        player_id: PlayerId,
    ) -> EndTurnScreen {
        let mut button_manager = ButtonManager::new();
        let pos = ScreenPos{v: Vector2{x: 10, y: 10}};
        let str = format!("Pass the device to Player {}", player_id.id);
        // TODO: button -> label + center on screen
        let _ = button_manager.add_button(Button::new(
            context, &str, pos));
        EndTurnScreen {
            button_manager: button_manager,
        }
    }

    fn handle_event_lmb_release(&mut self, context: &mut Context) {
        if is_tap(context) {
            context.add_command(ScreenCommand::PopScreen);
        }
    }

    fn handle_event_key_press(&mut self, context: &mut Context, key: VirtualKeyCode) {
        if key == glutin::VirtualKeyCode::Q
            || key == glutin::VirtualKeyCode::Escape
        {
            context.add_command(ScreenCommand::PopScreen);
        }
    }
}

impl Screen for EndTurnScreen {
    fn tick(&mut self, context: &mut Context, _: Time) {
        context.set_basic_color([0.0, 0.0, 0.0, 1.0]);
        self.button_manager.draw(context);
    }

    fn handle_event(&mut self, context: &mut Context, event: &WindowEvent) -> EventStatus {
        match *event {
            WindowEvent::MouseInput(Released, MouseButton::Left) => {
                self.handle_event_lmb_release(context);
            },
            WindowEvent::Touch(glutin::Touch{phase, ..}) => {
                if glutin::TouchPhase::Ended == phase {
                    self.handle_event_lmb_release(context);
                }
            },
            WindowEvent::KeyboardInput(Released, _, Some(key), _) => {
                self.handle_event_key_press(context, key);
            },
            _ => {},
        }
        EventStatus::Handled
    }
}
