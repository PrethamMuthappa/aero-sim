#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VizMode {
    VelocityMagnitude,
    Pressure,
    Vorticity,
}

impl VizMode {
    pub fn as_u32(self) -> u32 {
        match self {
            VizMode::VelocityMagnitude => 0,
            VizMode::Pressure => 1,
            VizMode::Vorticity => 2,
        }
    }
}

pub struct GuiState {
    pub wind_speed: f32,
    pub reynolds: f32,
    pub paused: bool,
    pub steps_per_frame: u32,
    pub reset_requested: bool,
    pub fps: f64,
    pub total_steps: u64,
    pub visualization: VizMode,
}

impl Default for GuiState {
    fn default() -> Self {
        Self {
            wind_speed: 0.05,
            reynolds: 100.0,
            paused: false,
            steps_per_frame: 1,
            reset_requested: false,
            fps: 0.0,
            total_steps: 0,
            visualization: VizMode::VelocityMagnitude,
        }
    }
}

impl GuiState {
    pub fn tau(&self, char_len: f32) -> f32 {
        let nu = self.wind_speed * char_len / self.reynolds;
        (3.0 * nu + 0.5).max(0.51)
    }
}
