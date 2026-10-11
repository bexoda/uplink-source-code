use ur_sim::*;

/// Everything a UI wants to do is collected here and applied after drawing,
/// so drawing code only ever needs `&Game`.
#[derive(Debug)]
pub enum Act {
    Cmd(Command),
    ToggleRelay(ServerId),
    SelectTarget(ServerId),
    /// Select the target, or clear it if it is already selected (clicking a map dot twice).
    ToggleTarget(ServerId),
    /// Forget the planned relays and target.
    ClearRoute,
    Connect,
    Disconnect,
    TogglePause,
    SaveNow,
    /// Game over: the agent is finished; back to the main menu.
    NewGame,
    /// The HUD's close button: "Close any connections and log off" to the main menu.
    LogOff,
}

impl crate::game::Game {
    /// Applies an action to the game. Returns the action back if it needs the app (saving, new game).
    pub fn apply_act(&mut self, act: Act) -> Option<Act> {
        match act {
            Act::Cmd(c) => {
                self.exec(c);
            }
            Act::ToggleRelay(id) => self.toggle_relay(id),
            Act::SelectTarget(id) => self.select_target(id),
            Act::ToggleTarget(id) => self.toggle_target(id),
            Act::ClearRoute => self.clear_route(),
            Act::Connect => self.connect(),
            Act::Disconnect => self.disconnect(),
            Act::TogglePause => self.paused = !self.paused,
            other @ (Act::SaveNow | Act::NewGame | Act::LogOff) => return Some(other),
        }
        None
    }
}
