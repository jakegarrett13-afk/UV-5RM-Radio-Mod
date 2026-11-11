use super::{scan, App};
use cortex_m::peripheral::SYST;

/// The menu slot TEXT is installed in.
pub(super) const TEXT_SLOT: u8 = 7;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeyFunction {
    None,
    WideNarrow,
    Monitor,
    Mode,
    TxTone,
    Scan,
    Power,
    Flashlight,
    Reverse,
    OpenText,
}

impl KeyFunction {
    pub fn label(self) -> &'static str {
        match self {
            KeyFunction::None => "NONE",
            KeyFunction::WideNarrow => "WIDE/NAR",
            KeyFunction::Monitor => "MONITOR",
            KeyFunction::Mode => "MODE",
            KeyFunction::TxTone => "TX TONE",
            KeyFunction::Scan => "SCAN",
            KeyFunction::Power => "POWER",
            KeyFunction::Flashlight => "LIGHT",
            KeyFunction::Reverse => "REVERSE",
            KeyFunction::OpenText => "TEXT",
        }
    }
}

pub(super) fn from_u8(v: u8) -> KeyFunction {
    match v {
        1 => KeyFunction::WideNarrow,
        2 => KeyFunction::Monitor,
        3 => KeyFunction::Mode,
        4 => KeyFunction::TxTone,
        5 => KeyFunction::OpenText,
        6 => KeyFunction::Scan,
        7 => KeyFunction::Power,
        8 => KeyFunction::Flashlight,
        // 9 => search (now app)
        10 => KeyFunction::Reverse,
        _ => KeyFunction::None,
    }
}

pub(super) fn invoke(app: &mut App, syst: &mut SYST, func: KeyFunction) {
    match func {
        KeyFunction::None | KeyFunction::TxTone => {}
        KeyFunction::WideNarrow => app.toggle_wide_narrow(syst),
        KeyFunction::Monitor => app.radio.toggle_monitor(),
        KeyFunction::Mode => app.toggle_modulation(syst),
        // Side keys still work under key lock, but an app's keys wouldn't, so
        // opening one on a locked radio would leave no way to close it.
        KeyFunction::OpenText => {
            if !app.key_lock {
                super::overlay::enter(app, syst, TEXT_SLOT);
            }
        }
        KeyFunction::Scan => scan::enter(app, syst),
        KeyFunction::Power => app.toggle_power(syst),
        KeyFunction::Flashlight => app.flashlight.toggle(),
        // TODO: search
        KeyFunction::Reverse => app.toggle_reverse(syst),
    }
}
