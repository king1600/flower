#![no_std]

#![feature(asm)]
#![feature(lang_items)]
#![feature(const_fn)]
#![feature(slice_rotate)]
#![feature(try_from)]
#![feature(nll)]
#![feature(try_trait)]
#![feature(range_contains, inclusive_range)]
#![feature(type_ascription)]
#![feature(ptr_internals, align_offset)]
#![feature(arbitrary_self_types)]
#![feature(inclusive_range_methods)]
#![feature(alloc, allocator_api, global_allocator, box_syntax)]
#![feature(abi_x86_interrupt)]
#![feature(compiler_builtins_lib)]
#![feature(panic_implementation)]
#![feature(panic_info_message)]

#[cfg(test)]
#[cfg_attr(test, macro_use)]
extern crate std;

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate lazy_static;
extern crate array_init;
extern crate rlibc;
extern crate alloc;
extern crate volatile;
extern crate spin;
extern crate x86_64;
// Used as a workaround until const-generics arrives
extern crate multiboot2;
extern crate bit_field;

use drivers::ps2::{self, device::Device};
use drivers::keyboard::{Keyboard, KeyEventType, Ps2Keyboard};
use terminal::TerminalOutput;

#[cfg(not(test))]
mod lang;
#[macro_use]
mod log;
#[macro_use]
mod util;
#[macro_use]
mod color;
#[macro_use]
mod terminal;
mod io;
mod interrupts;
mod memory;
mod drivers;

use memory::heap::Heap;

#[cfg_attr(not(test), global_allocator)]
pub static HEAP: Heap = Heap::new();

/// Kernel main function
#[no_mangle]
pub extern fn kmain(multiboot_info_addr: usize, guard_page_addr: usize) -> ! {
    say_hello();
    interrupts::init();
    let mb_info = unsafe { multiboot2::load(multiboot_info_addr) };
    memory::init_memory(&mb_info, guard_page_addr);

    match ps2::CONTROLLER.lock().setup() {
        Ok(_) => info!("ps2c: successful setup"),
        Err(error) => panic!("ps2c: threw error: {:?}", error),
    }

    let has_keyboard = check_keyboard();
    let has_mouse = check_mouse();

    if has_keyboard {
        let mut keyboard = Ps2Keyboard::new();
        trace!("kbd: ps/2 keyboard created");

        keyboard_echo_loop(keyboard);
    }

    halt()
}

/// Say hello to the user and print flower
fn say_hello() {
    terminal::STDOUT.write().clear().expect("Screen clear failed");

    print_flower().expect("Flower print failed");

    terminal::STDOUT.write().set_color(color!(Green on Black))
        .expect("Color should be supported");

    // Print boot message
    println!("Flower kernel boot!");
    println!("-------------------");

    // Reset colors
    terminal::STDOUT.write().set_color(color!(White on Black))
        .expect("Color should be supported");
}

fn print_flower() -> Result<(), terminal::TerminalOutputError<()>> {
    const FLOWER: &'static str = include_str!("resources/art/flower.txt");
    const FLOWER_STEM: &'static str = include_str!("resources/art/flower_stem.txt");

    let mut stdout = terminal::STDOUT.write();
    let old = stdout.cursor_pos();

    stdout.write_string_colored(FLOWER, color!(LightBlue on Black))?;
    stdout.write_string_colored(FLOWER_STEM, color!(Green on Black))?;
    stdout.set_cursor_pos(old)
}

fn keyboard_echo_loop(mut keyboard: Ps2Keyboard) -> ! {
    loop {
        if let Ok(Some(event)) = keyboard.read_event() {
            if event.event_type != KeyEventType::Break {
                if let Some(char) = event.char {
                    print!("{}", char);
                }
            }
        }
    }
}

fn check_keyboard() -> bool {
    if let Ok(keyboard) = ps2::CONTROLLER.lock().keyboard() {
        info!("kbd: detected in {:?}", keyboard.port_type().unwrap());
        true
    } else {
        warn!("kbd: not available");
        false
    }
}

fn check_mouse() -> bool {
    if let Ok(mouse) = ps2::CONTROLLER.lock().mouse() {
        info!("mouse: detected in {:?}", mouse.port_type().unwrap());
        true
    } else {
        warn!("mouse: not available");
        false
    }
}

fn halt() -> ! {
    unsafe {
        // Disable interrupts
        asm!("cli");

        // Halt forever...
        loop {
            asm!("hlt");
        }
    }
}
