use anyhow::{anyhow, Result};
use std::fmt::Debug;

#[derive(Clone, Copy)]
pub struct Reg(pub u8);
#[derive(Clone, Copy)]
pub struct Const(pub u8);

impl Debug for Reg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "R{:X}", self.0)
    }
}

impl Debug for Const {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:X}", self.0)
    }
}

#[derive(Debug)]
pub enum Instr {
    None,
    LoadFromMemory(Reg, Const),
    LoadWithConstant(Reg, Const),
    StoreToMemory(Reg, Const),
    Move(Reg, Reg),
    AddInt(Reg, Reg, Reg),
    AddFloat(Reg, Reg, Reg),
    Or(Reg, Reg, Reg),
    And(Reg, Reg, Reg),
    Xor(Reg, Reg, Reg),
    Rotate(Reg, Const),
    JumpIfEqual(Reg, Const),
    Halt,
    LoadFromPointer(Reg, Reg),
    StoreToPointer(Reg, Reg),
    JumpIfLess(Reg, Const),
}

impl Instr {
    pub fn new(i0: u8, i1: u8) -> Self {
        let low = |byte: u8| byte & 0xf;
        let high = |byte: u8| (byte >> 4) & 0xf;
        use Instr::*;
        match high(i0) {
            0 => None,
            1 => LoadFromMemory(Reg(low(i0)), Const(i1)),
            2 => LoadWithConstant(Reg(low(i0)), Const(i1)),
            3 => StoreToMemory(Reg(low(i0)), Const(i1)),
            4 => Move(Reg(high(i1)), Reg(low(i1))),
            5 => AddInt(Reg(low(i0)), Reg(high(i1)), Reg(low(i1))),
            6 => AddFloat(Reg(low(i0)), Reg(high(i1)), Reg(low(i1))),
            7 => Or(Reg(low(i0)), Reg(high(i1)), Reg(low(i1))),
            8 => And(Reg(low(i0)), Reg(high(i1)), Reg(low(i1))),
            9 => Xor(Reg(low(i0)), Reg(high(i1)), Reg(low(i1))),
            10 => Rotate(Reg(low(i0)), Const(i1)),
            11 => JumpIfEqual(Reg(low(i0)), Const(i1)),
            12 => Halt,
            13 => LoadFromPointer(Reg(low(i0)), Reg(low(i1))),
            14 => StoreToPointer(Reg(low(i0)), Reg(low(i1))),
            15 => JumpIfLess(Reg(low(i0)), Const(i1)),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub enum Action {
    None,
    SetReg(Reg, Const),
    SetMem(Const, Const),
    Jump(Const),
}

pub struct VM {
    pub regs: [u8; 16],
    pub memory: [u8; 256],
    pub pc: Const,
    pub actions: Vec<Action>,
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

impl VM {
    pub fn new() -> Self {
        Self {
            regs: [0; 16],
            memory: [0; 256],
            pc: Const(0),
            actions: Vec::new(),
        }
    }

    pub fn fill(&mut self, memory: &[u8]) {
        self.memory.fill(0);
        self.memory[..memory.len()].copy_from_slice(memory);
    }

    pub fn execute(&mut self, action: Action) -> Action {
        use std::mem::replace;
        use Action::*;
        match action {
            None => None,
            SetReg(reg, value) => {
                SetReg(reg, Const(replace(&mut self.regs[reg.0 as usize], value.0)))
            }
            SetMem(addr, value) => SetMem(
                addr,
                Const(replace(&mut self.memory[addr.0 as usize], value.0)),
            ),
            Jump(addr) => Jump(replace(&mut self.pc, addr)),
        }
    }

    pub fn redo(&mut self, action: Action) {
        let action = self.execute(action);
        self.actions.push(action);
    }

    pub fn undo(&mut self) {
        if let Some(action) = self.actions.pop() {
            self.execute(action);
            self.pc.0 -= 2;
        }
    }

    pub fn getr(&self, reg: Reg) -> Const {
        Const(self.regs[reg.0 as usize])
    }

    pub fn load(&self, addr: Const) -> Const {
        Const(self.memory[addr.0 as usize])
    }

    pub fn reset(&mut self) {
        self.regs.fill(0);
        self.pc = Const(0);
        self.actions.clear();
    }

    pub fn dis(&self, addr: Const) -> Instr {
        let addr = addr.0 as usize;
        Instr::new(self.memory[addr], self.memory[addr + 1])
    }

    pub fn exec(&mut self, instr: Instr) -> bool {
        use Action::None;
        use Action::*;
        use Instr::*;
        self.redo(match instr {
            Instr::None => None,
            LoadFromMemory(reg, addr) => SetReg(reg, self.load(addr)),
            LoadWithConstant(reg, value) => SetReg(reg, value),
            StoreToMemory(reg, addr) => SetMem(addr, self.getr(reg)),
            Move(from, to) => SetReg(to, self.getr(from)),
            AddInt(r0, r1, r2) => SetReg(r0, Const(self.getr(r1).0.wrapping_add(self.getr(r2).0))),
            Or(r0, r1, r2) => SetReg(r0, Const(self.getr(r1).0 | self.getr(r2).0)),
            And(r0, r1, r2) => SetReg(r0, Const(self.getr(r1).0 & self.getr(r2).0)),
            Xor(r0, r1, r2) => SetReg(r0, Const(self.getr(r1).0 ^ self.getr(r2).0)),
            Rotate(reg, shift) => {
                let shift = shift.0 & 7;
                let val = self.getr(reg).0;
                SetReg(
                    reg,
                    Const((val >> shift) | ((val & ((1 << shift) - 1)) << (8 - shift))),
                )
            }
            JumpIfEqual(reg, addr) => {
                if self.getr(reg).0 == self.getr(Reg(0)).0 {
                    Jump(addr)
                } else {
                    None
                }
            }
            Halt => {
                return false;
            }
            LoadFromPointer(reg, ptr) => SetReg(reg, self.load(self.getr(ptr))),
            StoreToPointer(reg, ptr) => SetMem(self.getr(ptr), self.getr(reg)),
            JumpIfLess(reg, addr) => {
                if self.getr(reg).0 < self.getr(Reg(0)).0 {
                    Jump(addr)
                } else {
                    None
                }
            }
            _ => unimplemented!(),
        });
        true
    }

    pub fn step(&mut self) -> Result<bool> {
        let instr = self.dis(self.pc);
        self.pc.0 = self
            .pc
            .0
            .checked_add(2)
            .ok_or_else(|| anyhow!("Program counter exceeded memory bounds (> 256)"))?;
        Ok(self.exec(instr))
    }
}
