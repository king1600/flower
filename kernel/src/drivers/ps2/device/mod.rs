use core::fmt;
use spin::Mutex;
use super::io::{self, command::*};
use super::Ps2Error;

pub mod keyboard;
pub mod mouse;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum PortType {
    Port1,
    Port2,
}

impl fmt::Debug for PortType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PortType::Port1 => write!(f, "port 1"),
            PortType::Port2 => write!(f, "port 2"),
        }
    }
}

impl PortType {
    pub fn test(&self) -> Result<bool, Ps2Error> {
        let cmd = match *self {
            PortType::Port1 => controller::Command::TestPort1,
            PortType::Port2 => controller::Command::TestPort2,
        };

        io::command(cmd);

        let result = io::read_blocking(&io::DATA_PORT);
        match result {
            Some(0x0) => Ok(true),
            Some(_) => Ok(false),
            None => Err(Ps2Error::ExpectedResponse)
        }
    }

    pub fn enable(&self) {
        match *self {
            PortType::Port1 => io::command(controller::Command::EnablePort1),
            PortType::Port2 => io::command(controller::Command::EnablePort2),
        }
    }

    pub fn disable(&self) {
        match *self {
            PortType::Port1 => io::command(controller::Command::DisablePort1),
            PortType::Port2 => io::command(controller::Command::DisablePort2),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DeviceType {
    Unknown,
    Keyboard(KeyboardType),
    Mouse(MouseType),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum KeyboardType {
    Unknown,
    TranslatedAtKeyboard,
    Mf2Keyboard,
    Mf2TranslatedKeyboard,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MouseType {
    Mouse,
    MouseWithScrollWheel,
    FiveButtonMouse,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DevicePort {
    pub port_type: PortType,
    enabled: bool,
    dirty: bool,
}

impl DevicePort {
    pub const fn new(port: PortType) -> Self {
        DevicePort {
            port_type: port,
            enabled: false,
            dirty: true,
        }
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    pub fn test(&self) -> Result<bool, Ps2Error> {
        self.port_type.test()
    }

    pub fn reset(&self) -> Result<(), Ps2Error> {
        self.command_port(device::Command::Reset)?;

        Ok(())
    }

    pub fn enable(&mut self) {
        self.port_type.enable();
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.port_type.disable();
        self.enabled = false;
    }

    pub fn ping(&mut self) -> Result<bool, Ps2Error> {
        // We assume that the device is dead until we get a response
        // Try ping the device 3 times before giving up and assuming dead
        for _i in 0..3 {
            // If a response was received from the echo command
            if let Ok(_) = command_raw(self, device::Command::Echo as u8) {
                // Second port requires this after calling echo, or it will repeat back all bytes
                if self.port_type == PortType::Port2 {
                    self.command_port(device::Command::ResetEcho)?;
                }
                debug!("ps2c: pinged {:?}", self.port_type);
                return Ok(true);
            }
        }

        debug!("ps2c: got no ping response from {:?}", self.port_type);
        Ok(false)
    }

    fn command_port(&self, cmd: device::Command) -> Result<(), Ps2Error> {
        // TODO: This is not ideal, as `ping` has to call command_raw directly. How can this be refactored?
        match command_raw(self, cmd as u8) {
            // We're only commanding the port so we don't care if the device doesn't exist
            Err(Ps2Error::DeviceUnavailable) | Err(Ps2Error::RetriesExceeded) => Ok(()),
            result => result,
        }
    }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn is_dirty(&self) -> bool { self.dirty }

    pub fn can_read(&self) -> bool {
        io::can_read() && match self.port_type {
            PortType::Port1 => io::can_read_port_1(),
            PortType::Port2 => io::can_read_port_2(),
        }
    }
}

pub trait Device {
    fn port(&self) -> &Mutex<Option<DevicePort>>;

    fn port_type(&self) -> Option<PortType> {
        self.port().lock().as_ref().map(|p| p.port_type)
    }

    fn is_mouse(&self) -> bool;

    fn is_keyboard(&self) -> bool;

    fn command_device(&self, cmd: device::Command) -> Result<(), Ps2Error> {
        if self.is_enabled() {
            let port = self.port().lock();
            command_raw(port.as_ref().ok_or(Ps2Error::DeviceUnavailable)?, cmd as u8)
        } else {
            Err(Ps2Error::DeviceDisabled)
        }
    }

    fn is_enabled(&self) -> bool {
        if let Some(port) = self.port().lock().as_ref() {
            port.is_enabled()
        } else {
            false
        }
    }

    fn set_port_dirty(&mut self, dirty: bool) {
        self.port().lock().as_mut().map(|p| p.set_dirty(dirty));
    }
}

#[derive(Debug)]
pub struct InternalDevice<'a> {
    port_type: PortType,
    port: &'a Mutex<Option<DevicePort>>,
    identity: DeviceType,
}

impl<'a> InternalDevice<'a> {
    pub fn new(port_type: PortType, port: &'a Mutex<Option<DevicePort>>) -> Self {
        InternalDevice {
            port_type,
            port,
            identity: DeviceType::Unknown,
        }
    }

    pub fn init(&mut self) {
        match self.identify() {
            Ok(identity) => {
                debug!("ps2c: identified device in {:?} as {:?}", self.port_type, identity);
                self.identity = identity;
            }
            Err(err) => warn!("ps2c: failed to identify device in {:?}: {:?}", self.port_type, err),
        }
    }

    pub fn as_keyboard(&self) -> Option<keyboard::Keyboard<'a>> {
        if self.is_keyboard() {
            Some(keyboard::Keyboard::new(self.port))
        } else {
            None
        }
    }

    pub fn as_mouse(&self) -> Option<mouse::Mouse<'a>> {
        if self.is_mouse() {
            Some(mouse::Mouse::new(self.port))
        } else {
            None
        }
    }

    fn identify(&mut self) -> Result<DeviceType, Ps2Error> {
        use self::DeviceType::*;

        self.command_device(device::Command::IdentifyDevice)?;

        let mut keyboard = false;
        let identity_result = loop {
            let response = io::read_blocking(&io::DATA_PORT);

            match response {
                // If the first byte we receive is 0xAB, this is an MF2 device
                Some(0xAB) if !keyboard => keyboard = true,
                // If we get an identity, break with it
                Some(identity) => break Some(identity),
                // If nothing is returned, we can assume we're not going to get any response
                None => break None,
            }
        };

        let identity = if let Some(identity) = identity_result {
            match identity {
                0x41 | 0xC1 if keyboard => Keyboard(KeyboardType::Mf2TranslatedKeyboard),
                0x83 if keyboard => Keyboard(KeyboardType::Mf2Keyboard),
                _ if keyboard => Keyboard(KeyboardType::Unknown),

                0x00 if !keyboard => Mouse(MouseType::Mouse),
                0x03 if !keyboard => Mouse(MouseType::MouseWithScrollWheel),
                0x04 if !keyboard => Mouse(MouseType::FiveButtonMouse),
                _ => Unknown,
            }
        } else {
            // If no response is returned, it must be a translated AT keyboard
            Keyboard(KeyboardType::TranslatedAtKeyboard)
        };

        Ok(identity)
    }
}

impl<'a> Device for InternalDevice<'a> {
    fn port(&self) -> &Mutex<Option<DevicePort>> { self.port }

    fn is_mouse(&self) -> bool {
        match self.identity {
            DeviceType::Mouse(_) => true,
            _ => false,
        }
    }

    fn is_keyboard(&self) -> bool {
        match self.identity {
            DeviceType::Keyboard(_) => true,
            _ => false,
        }
    }
}

pub(crate) fn command_raw(port: &DevicePort, cmd: u8) -> Result<(), Ps2Error> {
    for _ in 0..io::COMMAND_RETRIES {
        // If device is in the second port, send context switch command
        if port.port_type == PortType::Port2 {
            io::command(controller::Command::WriteCommandPort2);
        }
        io::flush_output();
        io::write_blocking(&io::DATA_PORT, cmd);
        let value = io::read_blocking(&io::DATA_PORT);
        match value {
            Some(io::ACK) | Some(io::ECHO) => return Ok(()),
            Some(io::RESEND) => continue,
            Some(unknown) => return Err(Ps2Error::UnexpectedResponse(unknown)),
            None => return Err(Ps2Error::ExpectedResponse),
        }
    }

    trace!("Exceeded {} retries while sending command {:X}", io::COMMAND_RETRIES, cmd);
    Err(Ps2Error::RetriesExceeded)
}

pub(crate) fn command_raw_data(port: &DevicePort, cmd: u8, data: u8) -> Result<(), Ps2Error> {
    match command_raw(port, cmd) {
        Ok(_) => {
            io::write_blocking(&io::DATA_PORT, data);
            match io::read_blocking(&io::DATA_PORT) {
                Some(io::ACK) => return Ok(()),
                Some(unknown) => return Err(Ps2Error::UnexpectedResponse(unknown)),
                None => return Err(Ps2Error::ExpectedResponse),
            }
        }
        result => result
    }
}
