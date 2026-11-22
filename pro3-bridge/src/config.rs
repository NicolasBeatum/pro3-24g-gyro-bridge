use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Es,
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloseAction {
    Ask,
    MinimizeToTray,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PadMode {
    Ds4NativeGyro,
    Xbox360,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ButtonLayout {
    XboxPs,
    Nintendo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GyroPcMode {
    Off,
    Mouse,
    RightStick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GyroTrigger {
    LeftTrigger,
    RightTrigger,
    LeftBumper,
    RightBumper,
    AlwaysOn,
    ToggleCapture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaddleAction {
    None,
    TouchpadClick,
    RecalibrateGyro,
    PauseGyro,
    PadL3,
    PadR3,
    PadSouth,
    PadEast,
    PadWest,
    PadNorth,
    PadL1,
    PadR1,
    KeySpace,
    KeyShift,
    KeyCtrl,
    KeyE,
    KeyR,
    KeyF,
    KeyTab,
    KeyF12,
}

impl PaddleAction {
    pub const ALL: &'static [PaddleAction] = &[
        PaddleAction::None,
        PaddleAction::TouchpadClick,
        PaddleAction::RecalibrateGyro,
        PaddleAction::PauseGyro,
        PaddleAction::PadL3,
        PaddleAction::PadR3,
        PaddleAction::PadSouth,
        PaddleAction::PadEast,
        PaddleAction::PadWest,
        PaddleAction::PadNorth,
        PaddleAction::PadL1,
        PaddleAction::PadR1,
        PaddleAction::KeySpace,
        PaddleAction::KeyShift,
        PaddleAction::KeyCtrl,
        PaddleAction::KeyE,
        PaddleAction::KeyR,
        PaddleAction::KeyF,
        PaddleAction::KeyTab,
        PaddleAction::KeyF12,
    ];

    pub fn label(self, lang: Language) -> &'static str {
        match (lang, self) {
            (Language::Es, PaddleAction::None) => "Sin asignar",
            (Language::Es, PaddleAction::TouchpadClick) => "DS4 Touchpad Click",
            (Language::Es, PaddleAction::RecalibrateGyro) => "Recalibrar Giroscopio",
            (Language::Es, PaddleAction::PauseGyro) => "Pausar Giroscopio (mantener)",
            (Language::Es, PaddleAction::PadL3) => "Mando: L3 (Click Stick Izq)",
            (Language::Es, PaddleAction::PadR3) => "Mando: R3 (Click Stick Der)",
            (Language::Es, PaddleAction::PadSouth) => "Mando: A / Cruz",
            (Language::Es, PaddleAction::PadEast) => "Mando: B / Círculo",
            (Language::Es, PaddleAction::PadWest) => "Mando: X / Cuadrado",
            (Language::Es, PaddleAction::PadNorth) => "Mando: Y / Triángulo",
            (Language::Es, PaddleAction::PadL1) => "Mando: L1 / LB",
            (Language::Es, PaddleAction::PadR1) => "Mando: R1 / RB",
            (Language::Es, PaddleAction::KeySpace) => "Tecla: Espacio",
            (Language::Es, PaddleAction::KeyShift) => "Tecla: Shift Izq",
            (Language::Es, PaddleAction::KeyCtrl) => "Tecla: Ctrl Izq",
            (Language::Es, PaddleAction::KeyE) => "Tecla: E",
            (Language::Es, PaddleAction::KeyR) => "Tecla: R",
            (Language::Es, PaddleAction::KeyF) => "Tecla: F",
            (Language::Es, PaddleAction::KeyTab) => "Tecla: Tab",
            (Language::Es, PaddleAction::KeyF12) => "Tecla: F12 (Captura Steam)",

            (Language::En, PaddleAction::None) => "Unassigned",
            (Language::En, PaddleAction::TouchpadClick) => "DS4 Touchpad Click",
            (Language::En, PaddleAction::RecalibrateGyro) => "Recalibrate Gyroscope",
            (Language::En, PaddleAction::PauseGyro) => "Pause Gyroscope (hold)",
            (Language::En, PaddleAction::PadL3) => "Pad: L3 (Left Stick Click)",
            (Language::En, PaddleAction::PadR3) => "Pad: R3 (Right Stick Click)",
            (Language::En, PaddleAction::PadSouth) => "Pad: A / Cross",
            (Language::En, PaddleAction::PadEast) => "Pad: B / Circle",
            (Language::En, PaddleAction::PadWest) => "Pad: X / Square",
            (Language::En, PaddleAction::PadNorth) => "Pad: Y / Triangle",
            (Language::En, PaddleAction::PadL1) => "Pad: L1 / LB",
            (Language::En, PaddleAction::PadR1) => "Pad: R1 / RB",
            (Language::En, PaddleAction::KeySpace) => "Key: Space",
            (Language::En, PaddleAction::KeyShift) => "Key: Left Shift",
            (Language::En, PaddleAction::KeyCtrl) => "Key: Left Ctrl",
            (Language::En, PaddleAction::KeyE) => "Key: E",
            (Language::En, PaddleAction::KeyR) => "Key: R",
            (Language::En, PaddleAction::KeyF) => "Key: F",
            (Language::En, PaddleAction::KeyTab) => "Key: Tab",
            (Language::En, PaddleAction::KeyF12) => "Key: F12 (Steam Screenshot)",
        }
    }

    pub fn virtual_key(self) -> Option<u16> {
        match self {
            PaddleAction::KeySpace => Some(0x20),
            PaddleAction::KeyShift => Some(0xA0),
            PaddleAction::KeyCtrl => Some(0xA2),
            PaddleAction::KeyE => Some(0x45),
            PaddleAction::KeyR => Some(0x52),
            PaddleAction::KeyF => Some(0x46),
            PaddleAction::KeyTab => Some(0x09),
            PaddleAction::KeyF12 => Some(0x7B),
            _ => None,
        }
    }
}

fn default_lang() -> Language {
    Language::Es
}

fn default_close_action() -> CloseAction {
    CloseAction::Ask
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BridgeConfig {
    #[serde(default = "default_lang")]
    pub lang: Language,
    #[serde(default = "default_close_action")]
    pub close_action: CloseAction,

    pub pad_mode: PadMode,
    pub layout: ButtonLayout,
    pub dsu_enabled: bool,
    pub dsu_port: u16,
    pub rumble_enabled: bool,
    pub rumble_strength: f32,

    pub gyro_pc_mode: GyroPcMode,
    pub gyro_trigger: GyroTrigger,
    pub gyro_sens: f32,
    pub gyro_roll_mix: f32,
    pub gyro_deadzone: f32,
    pub invert_x: bool,
    pub invert_y: bool,

    // Extra buttons (P1=bit25 PL, P2=bit26 PR, P3=bit21, P4=bit30)
    pub paddle_p1: PaddleAction,
    pub paddle_p2: PaddleAction,
    pub paddle_p3: PaddleAction,
    pub paddle_p4: PaddleAction,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            lang: Language::Es,
            close_action: CloseAction::Ask,
            pad_mode: PadMode::Ds4NativeGyro,
            layout: ButtonLayout::XboxPs,
            dsu_enabled: true,
            dsu_port: 26760,
            rumble_enabled: true,
            rumble_strength: 1.0,
            gyro_pc_mode: GyroPcMode::Off,
            gyro_trigger: GyroTrigger::LeftTrigger,
            gyro_sens: 1.4,
            gyro_roll_mix: 0.35,
            gyro_deadzone: 0.8,
            invert_x: false,
            invert_y: false,
            paddle_p1: PaddleAction::None,
            paddle_p2: PaddleAction::None,
            paddle_p3: PaddleAction::None,
            paddle_p4: PaddleAction::None,
        }
    }
}

impl BridgeConfig {
    pub fn config_path() -> PathBuf {
        if let Ok(mut exe) = std::env::current_exe() {
            exe.set_file_name("pro3_bridge_config.json");
            return exe;
        }
        PathBuf::from("pro3_bridge_config.json")
    }

    pub fn load() -> Self {
        let p = Self::config_path();
        if let Ok(s) = std::fs::read_to_string(&p) {
            if let Ok(cfg) = serde_json::from_str::<BridgeConfig>(&s) {
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let p = Self::config_path();
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, s);
        }
    }
}
