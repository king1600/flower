//! All PS/2 command related functionality or constants
// TODO: Document module and all command functionalities

/// Enums representing controller commands
pub mod controller {
    // TODO: Add remaining unimplemented controller commands
    /// Represents a PS/2 controller command
    #[derive(Copy, Clone, Debug)]
    #[repr(u8)]
    pub enum Command {
        DisablePort2 = 0xA7,
        EnablePort2 = 0xA8,
        DisablePort1 = 0xAD,
        EnablePort1 = 0xAE,
        WriteCommandPort2 = 0xD4,
        /// Returns
        ReadConfig = 0x20,
        /// Returns
        TestController = 0xAA,
        /// Returns
        TestPort1 = 0xAB,
        /// Returns
        TestPort2 = 0xA9,
    }

    /// Represents a PS/2 controller command with a data value
    #[derive(Copy, Clone, Debug)]
    #[repr(u8)]
    pub enum DataCommand {
        WriteConfig = 0x60,
    }
}

/// Enums representing PS/2 device commands
pub mod device {
    /// Represents a general PS/2 device command without additional data
    #[derive(Copy, Clone, Debug)]
    #[repr(u8)]
    pub enum Command {
        SetDefaults = 0xF6,
        Reset = 0xFF,
        /// Returns
        IdentifyDevice = 0xF2,
        /// Returns
        Echo = 0xEE,
        ResetEcho = 0xEC,
    }

    pub mod keyboard {
        #[derive(Copy, Clone, Debug)]
        #[repr(u8)]
        pub enum Command {
            /// Scan set 3 only
            SendRepeatEvents = 0xF7, // TODO: Call
            /// Scan set 3 only
            SendMakeReleaseEvents = 0xF8, // TODO: Call
            /// Scan set 3 only
            SendMakeEvents = 0xF9, // TODO: Call
            /// Scan set 3 only
            SendAllEvents = 0xFA, // TODO: Call
        }

        /// Represents a PS/2 keyboard command where additional data can be sent
        #[derive(Copy, Clone, Debug)]
        #[repr(u8)]
        pub enum DataCommand {
            SetLeds = 0xED,
            SetTypematicOptions = 0xF3,  // TODO: Call
            /// Scan set 3 only
            KeySendRepeatEvents = 0xFB, // TODO: Call
            /// Scan set 3 only
            KeySendMakeReleaseEvents = 0xFC, // TODO: Call
            /// Scan set 3 only
            KeySendMakeEvents = 0xFD, // TODO: Call
            SetGetScancode = 0xF0,
        }
    }

    #[allow(dead_code)] // TODO: Mouse driver not yet implemented
    pub mod mouse {
        /// Represents a PS/2 mouse command without a return and without additional data
        #[derive(Copy, Clone, Debug)]
        #[repr(u8)]
        pub enum Command {
            SetRemoteMode = 0xF0,
            SetWrapMode = 0xEE,
            SetStreamMode = 0xEA,
            StatusRequest = 0xE9,
            RequestSinglePacket = 0xEB,
        }

        /// Represents a PS/2 mouse command where additional data can be sent
        #[derive(Copy, Clone, Debug)]
        #[repr(u8)]
        pub enum DataCommand {
            SetSampleRate = 0xF3,
            SetResolution = 0xE8,
        }
    }
}
