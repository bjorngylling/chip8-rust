use log::error;
use pixels::{Pixels, SurfaceTexture};
use std::fs::File;
use std::io::prelude::*;
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

const WIDTH: u32 = 64;
const HEIGHT: u32 = 32;

fn main() {
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new((WIDTH * 10) as f64, (HEIGHT * 10) as f64);
        WindowBuilder::new()
            .with_title("CHIP8")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };
    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(WIDTH, HEIGHT, surface_texture).unwrap()
    };
    let mut emulator = Emulator::new();

    // Load a rom
    let mut f = File::open("rom/test_opcode.ch8").unwrap();
    let mut rom = Vec::new();
    f.read_to_end(&mut rom).unwrap();
    emulator.load_rom(&rom);

    event_loop.run(move |event, _, control_flow| {
        // Draw
        if let Event::RedrawRequested(_) = event {
            emulator.draw(pixels.get_frame());
            if pixels
                .render()
                .map_err(|e| error!("pixels.render() failed: {}", e))
                .is_err()
            {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        // Handle input
        if input.update(&event) {
            // Close events
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                pixels.resize_surface(size.width, size.height);
            }

            // Update internal state and request a redraw
            window.request_redraw();
        }

        emulator.process();
        window.request_redraw();
    });
}

const FONT: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

struct Emulator {
    i: u16,
    pc: u16,
    mem: [u8; 4096],
    v: [u8; 16],
    stack: Vec<u16>,
    vmem: [u8; 32 * 64],
}

impl Emulator {
    fn new() -> Self {
        let mut e = Self {
            i: 0,
            pc: 0x200,
            mem: [0x0; 4096],
            v: [0x0; 16],
            stack: Vec::new(),
            vmem: [0x0; 32 * 64],
        };
        // Load Font into memory at 0x50 - 0x9f
        e.mem[0x50..=0x9f].copy_from_slice(&FONT);
        return e;
    }

    fn load_rom(&mut self, rom: &[u8]) {
        if rom.len() > (4096 - 512) {
            panic!("size too large to load into memory")
        }

        for (i, v) in rom.iter().enumerate() {
            self.mem[0x200..][i] = *v;
        }
    }

    fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            if self.vmem[i] == 1 {
                pixel.copy_from_slice(&[0xff, 0xff, 0xff, 0xff])
            } else {
                pixel.copy_from_slice(&[0x0, 0x0, 0x0, 0xff])
            }
        }
    }

    fn process(&mut self) {
        // Fetch instruction from memory and move PC forward
        let instr = Emulator::read_word(self.mem, self.pc);
        self.pc += 2;

        self.run_instr(instr)
    }

    fn run_instr(&mut self, instr: u16) {
        // Split the instruction into relevant parts
        let t = (instr & 0xf000) >> 12;
        let x = ((instr & 0x0f00) >> 8) as usize;
        let y = ((instr & 0x00f0) >> 4) as usize;
        let n = (instr & 0x000f) as u8;
        let nn = (instr & 0x00ff) as u8;
        let nnn = instr & 0x0fff;
        let vx = self.v[x];
        let vy = self.v[y];
        match (t, x, y, n) {
            // clear screen
            (0x0, 0x0, 0xe, 0x0) => self.vmem.fill(0),
            // return from subroutine
            (0x0, 0x0, 0xe, 0xe) => self.pc = self.stack.pop().unwrap(),
            // jump
            (0x1, _, _, _) => self.pc = nnn,
            // call subroutine
            (0x2, _, _, _) => {
                self.stack.push(self.pc);
                self.pc = nnn;
            }
            // skip if vx eq
            (0x3, _, _, _) => {
                if vx == nn {
                    self.pc += 2
                }
            }
            // skip if vx neq
            (0x4, _, _, _) => {
                if vx != nn {
                    self.pc += 2
                }
            }
            // skip if vx eq vy
            (0x5, _, _, _) => {
                if vx == vy {
                    self.pc += 2
                }
            }
            // set register vx
            (0x6, _, _, _) => self.v[x] = nn,
            // add value register vx
            (0x7, _, _, _) => self.v[x] = vx.wrapping_add(nn),
            // set vx to vy
            (0x8, _, _, 0) => self.v[x] = vy,
            // set vx to vx OR vy
            (0x8, _, _, 1) => self.v[x] = vx | vy,
            // set vx to vx AND vy
            (0x8, _, _, 2) => self.v[x] = vx & vy,
            // set vx to vx XOR vy
            (0x8, _, _, 3) => self.v[x] = vx ^ vy,
            // set vx to vx XOR vy
            (0x8, _, _, 4) => {
                let wrapped: bool;
                (self.v[x], wrapped) = vx.overflowing_add(vy);
                self.v[0xf] = if wrapped { 1 } else { 0 }
            }
            // set vx to vx - vy
            (0x8, _, _, 5) => {
                let wrapped: bool;
                (self.v[x], wrapped) = vx.overflowing_sub(vy);
                self.v[0xf] = if wrapped { 0 } else { 1 }
            }
            // set vx to vy - vx
            (0x8, _, _, 7) => {
                let wrapped: bool;
                (self.v[x], wrapped) = vy.overflowing_sub(vx);
                self.v[0xf] = if wrapped { 0 } else { 1 }
            }
            // set vx to vy and shift right
            (0x8, _, _, 6) => {
                self.v[0xf] = vx & 0b00000001;
                self.v[x] = vy >> 1;
            }
            // set vx to vy and shift left
            (0x8, _, _, 0xe) => {
                self.v[0xf] = (vx & 0b10000000) >> 7;
                self.v[x] = vy << 1;
            }
            // skip if vx neq vy
            (0x9, _, _, _) => {
                if vx != vy {
                    self.pc += 2
                }
            }
            // set index register
            (0xa, _, _, _) => self.i = nnn,
            // jump with offset
            (0xb, _, _, _) => self.pc = self.v[0] as u16 + nnn,
            // random
            (0xc, _, _, _) => self.v[x] = rand::random::<u8>() & nn,
            // draw
            (0xd, _, _, _) => {
                let dx = vx & 63;
                let dy = vy & 31;
                self.v[0xf] = 0;
                let sprite = &self.mem[self.i as usize..(self.i + n as u16) as usize];
                for j in 0..n {
                    let r = sprite[j as usize];
                    for i in 0..8 {
                        let v = r >> (7 - i) & 0x01;
                        if v == 1 {
                            let idx = (dx + i) as usize + (dy + j) as usize * 64;
                            let ov = self.vmem[idx];
                            self.v[0xf] = ov; // If old value was 1 then we mark collision
                            self.vmem[idx] = v ^ ov;
                        }
                    }
                }
            }

            // unimplemented instruction
            (t, x, y, n) => println!(
                "missing instr {:#02x?} {:#02x?} {:#02x?} {:#02x?}",
                t, x, y, n
            ),
        }
    }

    fn read_word(mem: [u8; 4096], addr: u16) -> u16 {
        (mem[addr as usize] as u16) << 8 | (mem[addr as usize + 1] as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emulator_loads_a_default_font() {
        let e = Emulator::new();
        assert_eq!(e.mem[0x50..=0x54], [0xF0, 0x90, 0x90, 0x90, 0xF0]);
        assert_eq!(e.mem[0x9b..=0x9f], [0xF0, 0x80, 0xF0, 0x80, 0x80]);
    }

    #[test]
    fn emulator_loads_a_rom() {
        let mut e = Emulator::new();
        e.load_rom(&[0xa, 0xb, 0x1]);
        assert_eq!(e.mem[0x200..=0x202], [0xa, 0xb, 0x1]);
    }

    #[test]
    fn emulator_instr_jump() {
        let mut e = Emulator::new();
        e.run_instr(0x1caf);
        assert_eq!(e.pc, 0xcaf);
    }

    #[test]
    fn emulator_instr_set_vx() {
        let mut e = Emulator::new();
        e.run_instr(0x6321);
        assert_eq!(e.v[3], 0x21);
    }

    #[test]
    fn emulator_instr_add_to_vx() {
        let mut e = Emulator::new();
        e.v[3] = 0x10;
        e.run_instr(0x730f);
        assert_eq!(e.v[3], 0x1f);
    }

    #[test]
    fn emulator_instr_skip_if_vx_eq() {
        let mut e = Emulator::new();
        e.v[5] = 0x5a;
        e.run_instr(0x350f);
        assert_eq!(e.pc, 512);
        e.run_instr(0x355a);
        assert_eq!(e.pc, 514);
    }

    #[test]
    fn emulator_instr_skip_if_vx_neq() {
        let mut e = Emulator::new();
        e.v[5] = 0xfa;
        e.run_instr(0x350f);
        assert_eq!(e.pc, 512);
        e.run_instr(0x35fa);
        assert_eq!(e.pc, 514);
    }

    #[test]
    fn emulator_instr_skip_if_vx_eq_vy() {
        let mut e = Emulator::new();
        e.v[4] = 0xfa;
        e.v[5] = 0xfa;
        e.run_instr(0x5500);
        assert_eq!(e.pc, 512);
        e.run_instr(0x5450);
        assert_eq!(e.pc, 514);
    }

    #[test]
    fn emulator_instr_skip_if_vx_neq_vy() {
        let mut e = Emulator::new();
        e.v[4] = 0xfa;
        e.v[5] = 0xfa;
        e.run_instr(0x9450);
        assert_eq!(e.pc, 512);
        e.run_instr(0x9460);
        assert_eq!(e.pc, 514);
    }

    #[test]
    fn emulator_instr_add_to_vx_with_overflow() {
        let mut e = Emulator::new();
        e.v[3] = 0xfe;
        e.run_instr(0x7302);
        assert_eq!(e.v[3], 0x00);
    }

    #[test]
    fn emulator_instr_set_vx_to_vy() {
        let mut e = Emulator::new();
        e.v[1] = 0xfe;
        e.run_instr(0x8010);
        assert_eq!(e.v[0], 0xfe);
    }

    #[test]
    fn emulator_instr_set_vx_to_vx_or_vy() {
        let mut e = Emulator::new();
        e.v[0] = 0b00011010;
        e.v[1] = 0b00010101;
        e.run_instr(0x8011);
        assert_eq!(e.v[0], 0b00011111);
    }

    #[test]
    fn emulator_instr_set_vx_to_vx_and_vy() {
        let mut e = Emulator::new();
        e.v[0] = 0b00011010;
        e.v[1] = 0b00010101;
        e.run_instr(0x8012);
        assert_eq!(e.v[0], 0b00010000);
    }

    #[test]
    fn emulator_instr_set_vx_to_vx_xor_vy() {
        let mut e = Emulator::new();
        e.v[0] = 0b00011010;
        e.v[1] = 0b00010101;
        e.run_instr(0x8013);
        assert_eq!(e.v[0], 0b00001111);
    }

    #[test]
    fn emulator_instr_add_vy_to_vx_with_carry_flag() {
        let mut e = Emulator::new();
        e.v[0] = 0x03;
        e.v[1] = 0xfe;
        e.run_instr(0x8014);
        assert_eq!(e.v[0], 0x1);
        assert_eq!(e.v[0xf], 0x1);
    }

    #[test]
    fn emulator_instr_set_vx_to_vx_sub_vy() {
        let mut e = Emulator::new();
        e.v[0] = 0xa;
        e.v[1] = 0x4;
        e.run_instr(0x8015);
        assert_eq!(e.v[0], 0x6);
        assert_eq!(e.v[0xf], 0x1);
    }

    #[test]
    fn emulator_instr_set_vx_to_vy_sub_vx() {
        let mut e = Emulator::new();
        e.v[0] = 0xa;
        e.v[1] = 0x4;
        e.run_instr(0x8017);
        assert_eq!(e.v[0], 0xfa);
        assert_eq!(e.v[0xf], 0x0);
    }

    #[test]
    fn emulator_instr_set_vx_to_vy_and_shift_left() {
        let mut e = Emulator::new();
        e.v[1] = 0b01100000;
        e.run_instr(0x801e);
        assert_eq!(e.v[0], 0b11000000);
        assert_eq!(e.v[0xf], 0x0, "flag should have value of overflowed bit");
        e.v[1] = 0b11000000;
        e.run_instr(0x801e);
        assert_eq!(e.v[0], 0b10000000);
        assert_eq!(e.v[0xf], 0x1, "flag should have value of overflowed bit");
    }

    #[test]
    fn emulator_instr_set_vx_to_vy_and_shift_right() {
        let mut e = Emulator::new();
        e.v[1] = 0b00000110;
        e.run_instr(0x8016);
        assert_eq!(e.v[0], 0b00000011);
        assert_eq!(e.v[0xf], 0x0, "flag should have value of overflowed bit");
        e.v[1] = 0b00000011;
        e.run_instr(0x8016);
        assert_eq!(e.v[0], 0b00000001);
        assert_eq!(e.v[0xf], 0x1, "flag should have value of overflowed bit");
    }

    #[test]
    fn emulator_instr_subroutine_call() {
        let mut e = Emulator::new();
        e.run_instr(0x2abc);
        assert_eq!(e.pc, 0xabc);
        assert_eq!(e.stack[0], 0x200);
    }

    #[test]
    fn emulator_instr_subroutine_return() {
        let mut e = Emulator::new();
        e.stack.push(0xabc);
        e.run_instr(0x00ee);
        assert_eq!(e.pc, 0xabc);
        assert_eq!(e.stack.len(), 0);
    }

    #[test]
    fn emulator_instr_set_i() {
        let mut e = Emulator::new();
        e.run_instr(0xa123);
        assert_eq!(e.i, 0x123);
    }

    #[test]
    fn emulator_instr_jump_with_offset() {
        let mut e = Emulator::new();
        e.run_instr(0xb2fd);
        assert_eq!(e.pc, 0x2fd);
        e.pc = 0x200;
        e.v[0x0] = 0x002;
        e.run_instr(0xb2fd);
        assert_eq!(e.pc, 0x2ff);
    }

    #[test]
    fn emulator_instr_rand() {
        let mut e = Emulator::new();
        e.run_instr(0xc0ff);
        e.run_instr(0xc1ff);
        assert_ne!(e.v[0], e.v[1]);
        for _ in 0..20 {
            println!("hi");
            e.run_instr(0xc00f);
            assert_eq!(
                e.v[0] < 0xf0,
                true,
                "the random number should be smaller than 0xf0"
            );
        }
    }

    #[test]
    fn emulator_instr_display() {
        let mut e = Emulator::new();
        e.mem[0x300] = 0b11001100;
        e.mem[0x301] = 0b01010101;
        e.i = 0x300;
        e.v[0] = 0;
        e.v[1] = 3;
        e.run_instr(0xd012);
        assert_eq!(e.vmem[3 * 64..3 * 64 + 8], [1, 1, 0, 0, 1, 1, 0, 0]);
        assert_eq!(e.vmem[4 * 64..4 * 64 + 8], [0, 1, 0, 1, 0, 1, 0, 1]);
    }

    #[test]
    fn emulator_handles_missing_instructions() {
        let mut e = Emulator::new();
        e.process()
    }

    #[test]
    fn read_word_reads_16_bits() {
        let mut mem: [u8; 4096] = [0x0; 4096];
        mem[0x10..=0x11].copy_from_slice(&[0x5c, 0xa3]);
        assert_eq!(Emulator::read_word(mem, 0x10), 0x5ca3);
    }
}
