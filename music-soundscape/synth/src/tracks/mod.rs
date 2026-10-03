//! The album, in running order.

use crate::track::Track;

mod last_train;
mod night_bus;
mod northern_tape;
mod sodium_rain;
mod strobe;
mod text_me;

pub const ALL: &[(&str, fn() -> Track)] = &[
    ("northern-tape-memory", northern_tape::render),
    ("night-bus-hymn", night_bus::render),
    ("text-me-when-youre-home", text_me::render),
    ("sodium-rain", sodium_rain::render),
    ("strobe-through-fog", strobe::render),
    ("last-train-polaroid-light", last_train::render),
];
