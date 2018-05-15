//! # Keyboard Driver
//!
//! The keyboard driver handles all keyboard related functionality, intended to support both PS/2 and USB.
//! Currently, only PS/2 support has been implemented through the use of the PS/2 driver.
//!
//! The driver is event based, and events are received through the `read_event` method, which blocks until an event is received.
//! The event contains the keycode pressed, which can be compared to `keymap::codes`, an optional `char`, the type of press, and various modifier flags.
//!
//! # Examples
//!
//! ```rust,no_run
//! let device = drivers::ps2::CONTROLLER.device(drivers::ps2::DevicePort::Keyboard);
//! let mut keyboard = Ps2Keyboard::new(device);
//!
//! keyboard.enable()?;
//! loop {
//!     let event = keyboard.read_event()?;
//!     handle_event(event);
//! }
//! ```

// TODO: Redo all examples

use core::convert::{TryFrom, TryInto};
use drivers::ps2::{self, device::Device, Ps2Error};

pub mod keymap;

bitflags! {
    pub struct ModifierFlags: u8 {
        /// If a SHIFT modifier is active
        const SHIFT = 1 << 0;
        /// If the NUM_LOCK modifier is active
        const NUM_LOCK = 1 << 1;
        /// If a CAPS_LOCK modifier is active
        const CAPS_LOCK = 1 << 2;
    }
}

bitflags! {
    /// Flags to hold the current keyboard lock states
    pub struct StateFlags: u8 {
        /// If num lock is enabled
        const NUM_LOCK = 1 << 0;
        /// If scroll lock is enabled
        const SCROLL_LOCK = 1 << 1;
        /// If caps lock is enabled
        const CAPS_LOCK = 1 << 2;
        /// If function lock is enabled
        const FUNCTION_LOCK = 1 << 3;
    }
}

impl ModifierFlags {
    /// Creates `ModifierFlags` from the given contained booleans
    ///
    /// # Examples
    ///
    /// ```rust
    /// let modifiers = ModifierFlags::from_modifiers(true, true, true);
    /// assert_eq!(modifiers, ModifierFlags::SHIFT | ModifierFlags::NUM_LOCK | ModifierFlags::CAPS_LOCK);
    /// ```
    fn from_modifiers(shift: bool, num_lock: bool, caps_lock: bool) -> Self {
        let mut flags = ModifierFlags::empty();
        flags.set(ModifierFlags::SHIFT, shift);
        flags.set(ModifierFlags::NUM_LOCK, num_lock);
        flags.set(ModifierFlags::CAPS_LOCK, caps_lock);
        flags
    }
}

/// Mapping from a keycode into a character
pub enum KeyCharMapping {
    /// A key with no character mapping
    Empty,
    /// A key with a constant character mapping
    Single(char),
    /// A key with an alternative character mapping when shift is pressed
    Shifted(char, char),
    /// A key with an alternative character mapping when either CAPS is enabled or shift is pressed
    Capitalized(char, char),
    /// A key that only maps to a character when numlock is disabled
    NumLocked(char),
}

impl KeyCharMapping {
    /// Gets the character for this char based on the given modifiers
    pub fn char(&self, modifiers: ModifierFlags) -> Option<char> {
        use self::KeyCharMapping::*;
        match *self {
            Single(character) => Some(character),
            Shifted(character, shifted) => if modifiers.contains(ModifierFlags::SHIFT) {
                Some(shifted)
            } else {
                Some(character)
            },
            Capitalized(character, capital) => if modifiers.contains(ModifierFlags::CAPS_LOCK) ^ modifiers.contains(ModifierFlags::SHIFT) {
                Some(capital)
            } else {
                Some(character)
            },
            NumLocked(character) if !modifiers.contains(ModifierFlags::NUM_LOCK) => Some(character),
            _ => None,
        }
    }
}

pub type Keycode = u8;

/// Contains data relating to a key press event
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct KeyEvent {
    pub keycode: Keycode,
    pub char: Option<char>,
    pub event_type: KeyEventType,
    pub modifiers: ModifierFlags,
}

/// The type of key event that occurred
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum KeyEventType {
    /// When the key is initially pressed
    Make,
    /// When the key is released
    Break,
    /// When the key is held down, and a repeat is fired
    Repeat,
}

/// Interface to a generic keyboard.
pub trait Keyboard {
    type Error;

    /// Polls the device for a new key state event, or returns `None` if none have occurred since the last poll.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let device = drivers::ps2::CONTROLLER.device(drivers::ps2::DevicePort::Keyboard);
    /// let mut keyboard = Ps2Keyboard::new(device);
    ///
    /// if let Some(event) = keyboard.read_event()? {
    ///     println!("Event occurred for char: {}", event.char.unwrap_or(' '));
    /// }
    /// ```
    // TODO: This should eventually use interrupts and hold a queue
    fn read_event(&mut self) -> Result<Option<KeyEvent>, Self::Error>;

    /// Returns `true` if the given keycode is currently being pressed
    ///
    /// ```rust,no_run
    /// let device = drivers::ps2::CONTROLLER.device(drivers::ps2::DevicePort::Keyboard);
    /// let mut keyboard = Ps2Keyboard::new(device);
    ///
    /// if keyboard.pressed(keymap::codes::LEFT_SHIFT) {
    ///     println!("Left shift pressed");
    /// } else {
    ///     println!("Left shift not pressed");
    /// }
    /// ```
    fn pressed(&self, keycode: Keycode) -> bool;

    fn num_lock(&self) -> bool;

    fn scroll_lock(&self) -> bool;

    fn caps_lock(&self) -> bool;

    fn function_lock(&self) -> bool;
}

const KEY_STATE_LENGTH: usize = 0xFF / 8;

/// Handles interface to a PS/2 keyboard, if available
pub struct Ps2Keyboard {
    key_state_map: [u8; KEY_STATE_LENGTH],
    state: StateFlags,
}

impl Ps2Keyboard {
    /// Creates a new Ps2Keyboard from the given PS/2 device
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let device = drivers::ps2::CONTROLLER.device(drivers::ps2::DevicePort::Keyboard);
    /// let mut keyboard = Ps2Keyboard::new(device);
    /// ```
    pub fn new() -> Self {
        ps2::CONTROLLER.lock().set_keyboard_change_listener(Ps2Keyboard::on_keyboard_change);
        Ps2Keyboard {
            key_state_map: [0; KEY_STATE_LENGTH],
            state: StateFlags::empty(),
        }
    }

    fn on_keyboard_change(keyboard: ps2::device::keyboard::Keyboard) -> Result<(), Ps2Error> {
        keyboard.set_scanset(ps2::device::keyboard::Scanset::Two)
    }

    /// Creates a [KeyEvent] from the given scancode and key state
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let scancode = ps2::device::keyboard::Scancode::new(0x15, false, true);
    /// let event = keyboard.create_event(&scancode).unwrap();
    /// assert_eq!(event.keycode, keymap::codes::Q);
    /// assert_eq!(event.char, Some('q'));
    /// assert_eq!(event.event_type, KeyEventType::Make);
    /// ```
    fn create_event(&self, scancode: &ps2::device::keyboard::Scancode) -> Option<KeyEvent> {
        let shift = self.pressed(keymap::codes::LEFT_SHIFT) || self.pressed(keymap::codes::RIGHT_SHIFT);
        let num_lock = self.state.contains(StateFlags::NUM_LOCK);
        let caps_lock = self.state.contains(StateFlags::CAPS_LOCK);
        let modifiers = ModifierFlags::from_modifiers(shift, num_lock, caps_lock);

        if let Ok(keycode) = (*scancode).try_into() {
            let char = keymap::get_us_qwerty_char(keycode).char(modifiers);

            // If the key was already pressed and make was sent, this is a repeat event
            let event_type = match scancode.make {
                true if self.pressed(keycode) => KeyEventType::Repeat,
                true => KeyEventType::Make,
                false => KeyEventType::Break,
            };

            Some(KeyEvent { keycode, char, event_type, modifiers })
        } else {
            None
        }
    }

    // TODO: Update LEDs
    fn handle_state(&mut self, event: KeyEvent) {
        if event.event_type == KeyEventType::Make {
            use self::keymap::codes::*;
            match event.keycode {
                SCROLL_LOCK => self.state.toggle(StateFlags::SCROLL_LOCK),
                NUM_LOCK => self.state.toggle(StateFlags::NUM_LOCK),
                CAPS_LOCK => self.state.toggle(StateFlags::CAPS_LOCK),
                ESCAPE if self.pressed(FUNCTION) => self.state.toggle(StateFlags::FUNCTION_LOCK),
                _ => (),
            }
        }
    }
}

impl Keyboard for Ps2Keyboard {
    type Error = Ps2Error;

    fn read_event(&mut self) -> Result<Option<KeyEvent>, Ps2Error> {
        let mut keyboard = ps2::CONTROLLER.lock().keyboard()?;
        Ok(keyboard.read_scancode()?.map(|scancode| {
            let event = self.create_event(&scancode);
            if let Some(event) = event {
                // Update states such as caps lock with this key event
                self.handle_state(event);
                let index = event.keycode as usize;
                let bit = 1 << (index % 8);
                let bucket_index = index / 8;
                if scancode.make {
                    self.key_state_map[bucket_index] |= bit;
                } else {
                    self.key_state_map[bucket_index] &= !bit;
                }
            } else {
                // If we received a scancode but it was invalid, the device probably changed.
                keyboard.set_port_dirty(true);
            }
            event
        }).unwrap_or(None))
    }

    fn pressed(&self, keycode: Keycode) -> bool {
        let index = keycode as usize;
        let bucket = *self.key_state_map.get(index / 8).unwrap_or(&0);
        let bit_index = (index % 8);
        ((bucket >> bit_index) & 1) != 0
    }

    fn num_lock(&self) -> bool { self.state.contains(StateFlags::NUM_LOCK) }

    fn scroll_lock(&self) -> bool { self.state.contains(StateFlags::SCROLL_LOCK) }

    fn caps_lock(&self) -> bool { self.state.contains(StateFlags::CAPS_LOCK) }

    fn function_lock(&self) -> bool { self.state.contains(StateFlags::FUNCTION_LOCK) }
}

pub enum UnknownScancode {
    UnknownPlainScancode(u8),
    UnknownExtendedScancode(u8),
}

impl TryFrom<ps2::device::keyboard::Scancode> for Keycode {
    type Error = UnknownScancode;

    /// Gets the Flower keycode for this scancode
    ///
    /// # Examples
    ///
    /// ```rust
    /// let scancode = ps2::device::keyboard::Scancode::new(0x01, false, true);
    /// assert_eq!(scancode.keycode(), Some(keymap::codes::KEY_F9));
    /// ```
    fn try_from(scancode: ps2::device::keyboard::Scancode) -> Result<Keycode, UnknownScancode> {
        use self::UnknownScancode::*;

        let converted = if !scancode.extended {
            keymap::get_code_ps2_set_2(scancode.code)
        } else {
            keymap::get_extended_code_ps2_set_2(scancode.code)
        };

        match (converted, scancode.extended) {
            (Some(keycode), _) => Ok(keycode),
            (None, false) => Err(UnknownPlainScancode(scancode.code)),
            (None, true) => Err(UnknownExtendedScancode(scancode.code)),
        }
    }
}
