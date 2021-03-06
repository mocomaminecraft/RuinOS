pub mod pic;

use lazy_static::lazy_static;

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::{print, println};
use crate::gdt;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt[Interrupt::TIMER.as_usize()].set_handler_fn(timer_interrupt_handler);
        idt[Interrupt::KEYBOARD.as_usize()].set_handler_fn(keyboard_interrupt_handler);

        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

//Exceptions

extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: &mut InterruptStackFrame
) {
    println!("Breakpoint:\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut InterruptStackFrame, _error_code: u64
) -> ! {
    panic!("Double fault:\n{:#?}", stack_frame);
}

use x86_64::structures::idt::PageFaultErrorCode;
use crate::util::halt_loop;

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("Page fault:");
    println!("Accesed adress: {:?}", Cr2::read());
    println!("{:#?}", stack_frame);

    halt_loop();
}

//Interrupts

use pic::Pics;
use spin;

const PIC_1_OFFSET: usize = 32;
const PIC_2_OFFSET: usize = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<Pics> = 
    spin::Mutex::new( unsafe { Pics::new(PIC_1_OFFSET, PIC_2_OFFSET) } );

pub fn init_interrupts() {
    unsafe { PICS.lock().init(); }
    x86_64::instructions::interrupts::enable();
}

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
enum Interrupt {
    TIMER = PIC_1_OFFSET,
    KEYBOARD,
}

impl Interrupt {
    fn as_u8(&self) -> u8 {
        self.as_usize() as u8
    }

    fn as_usize(&self) -> usize {
        *self as usize
    }
}

extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: &mut InterruptStackFrame
) {
    print!(".");

    unsafe { PICS.lock().end_interrupt(Interrupt::TIMER.as_u8()); }
}


extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: &mut InterruptStackFrame
) {
    use x86_64::instructions::port::Port;
    use spin::Mutex;
    use pc_keyboard::{ DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts };

    lazy_static! {
        pub static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> = 
            Mutex::new(Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore));
    }

    //Lock the keyboard and read
    let mut kbd = KEYBOARD.lock();
    static mut scancode_port: Port<u8> = Port::new(0x60);

    //Use the keyboard
    let scancode = unsafe { scancode_port.read() };
    if let Ok(Some(key_event)) = kbd.add_byte(scancode) {
        if let Some(key) = kbd.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(c) => print!("{}",c),
                DecodedKey::RawKey(key) => print!("{:?}", key),
            }
        }
    }

    unsafe { PICS.lock().end_interrupt(Interrupt::TIMER.as_u8()); }
}
