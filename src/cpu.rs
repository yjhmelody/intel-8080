use crate::opcode::Opcode;
use crate::register::{Flag, Register};

#[derive(Debug, Clone)]
pub struct CPU {
    /// PSW
    pub flag: Flag,
    /// B C D E H L (H plus L are M)
    pub registers: [u8; 6],
    pub acc: u8,
    sp: u16,
    pc: u16,
    data: Vec<u8>,
    interrupted: bool,
    interrupted_addr: u16,
    halted: bool,
}

impl CPU {
    #[inline]
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            flag: Flag::default(),
            registers: [0, 0, 0, 0, 0, 0],
            acc: 0,
            sp: data.len() as u16,
            pc: 0,
            data,
            interrupted: true,
            interrupted_addr: 0,
            halted: false,
        }
    }

    #[inline]
    pub fn pc(&self) -> usize {
        self.pc as usize
    }

    #[inline]
    pub fn sp(&self) -> usize {
        self.sp as usize
    }

    #[inline]
    pub fn set_data(&mut self, data: Vec<u8>) {
        self.data = data;
    }

    #[inline]
    pub fn data(&self) -> Vec<u8> {
        self.data.clone()
    }

    #[inline]
    fn make_address(val1: u8, val2: u8) -> usize {
        (val1 as usize) << 8 | (val2 as usize)
    }

    #[inline]
    fn compose_to_u16(val1: u8, val2: u8) -> u16 {
        u16::from(val1) << 8 | u16::from(val2)
    }

    #[inline]
    fn decompose_to_u8(val: u16) -> (u8, u8) {
        let res = ((val & 0xFF_00) >> 8) as u8;
        let res2 = (val & 0x00_FF) as u8;
        (res, res2)
    }

    #[inline]
    pub fn set_memory_address(&mut self, addr: u16) {
        let (h, l) = Self::decompose_to_u8(addr);
        self.registers[Register::H as usize] = h;
        self.registers[Register::L as usize] = l;
    }

    #[inline]
    fn memory_address(&self) -> usize {
        Self::make_address(
            self.registers[Register::H as usize],
            self.registers[Register::L as usize],
        )
    }

    #[inline]
    fn register_or_memory_data(&self, reg: u8) -> u8 {
        if reg == Register::Mem as u8 {
            self.data[self.memory_address()]
        } else if reg == Register::Acc as u8 {
            self.acc
        } else {
            self.registers[reg as usize]
        }
    }

    #[inline]
    fn set_register_or_memory_data(&mut self, reg: u8, data: u8) {
        if reg == Register::Mem as u8 {
            let addr = self.memory_address();
            self.data[addr] = data;
        } else if reg == Register::Acc as u8 {
            self.acc = data;
        } else {
            self.registers[reg as usize] = data;
        }
    }

    #[inline]
    fn set_jump_pc(&mut self) {
        self.pc = Self::compose_to_u16(self.data[self.pc() - 1], self.data[self.pc() - 2]);
    }

    #[inline]
    fn inc(val1: u8, val2: u8) -> (u8, u8) {
        let mut val1 = val1;
        let (val2, b) = val2.overflowing_add(1);
        if b {
            val1 += 1;
        }
        (val1, val2)
    }

    #[inline]
    fn dec(val1: u8, val2: u8) -> (u8, u8) {
        let mut val1 = val1;
        let (val2, b) = val2.overflowing_sub(1);
        if b {
            val1 -= 1;
        }
        (val1, val2)
    }

    #[inline]
    fn update_zero_flag(&mut self, val: u8) {
        if val == 0 {
            self.flag.set_zero_flag(true);
        } else {
            self.flag.set_zero_flag(false);
        }
    }

    #[inline]
    fn update_sign_flag(&mut self, val: u8) {
        if val > 0b0111_1111 {
            self.flag.set_sign_flag(true);
        } else {
            self.flag.set_sign_flag(false);
        }
    }

    #[inline]
    fn update_carry_flag_with_carry(&mut self, val1: u8, val2: u8, carry: u8) {
        use std::u16;
        if u16::from(val1) + u16::from(val2) + u16::from(carry) > 0xff {
            self.flag.set_carry_flag(true);
        } else {
            self.flag.set_carry_flag(false);
        }
    }

    #[inline]
    fn update_aux_flag(&mut self, val1: u8, val2: u8) {
        self.flag
            .set_auxiliary_carry_flag((val1 & 0b0000_1111) + (val2 & 0b0000_1111) > 0b0000_1111);
    }

    #[inline]
    fn update_aux_flag_with_carry(&mut self, val1: u8, val2: u8, carry: u8) {
        self.flag.set_auxiliary_carry_flag(
            (val1 & 0b0000_1111) + (val2 & 0b0000_1111) + carry > 0b0000_1111,
        );
    }

    fn update_parity_flag(&mut self, val: u8) {
        if val.count_ones() & 0b0000_0001 == 0 {
            self.flag.set_parity_flag(true);
        } else {
            self.flag.set_parity_flag(false);
        }
    }

    fn stack_push_u8(&mut self, val: u8) {
        self.sp -= 1;
        let sp = self.sp();
        self.data[sp] = val;
    }

    fn stack_pop_u8(&mut self) -> u8 {
        let res = self.data[self.sp()];
        self.sp += 1;
        res
    }

    pub fn stack_push(&mut self, val: u16) {
        let (val1, val2) = Self::decompose_to_u8(val);
        self.stack_push_u8(val1);
        self.stack_push_u8(val2);
    }

    pub fn stack_pop(&mut self) -> u16 {
        let val1 = self.stack_pop_u8();
        let val2 = self.stack_pop_u8();
        Self::compose_to_u16(val2, val1)
    }

    #[inline]
    fn op_return(&mut self) {
        self.pc = self.stack_pop();
    }

    #[inline]
    fn op_call(&mut self) {
        self.stack_push(self.pc);
        let val1 = self.data[self.pc() - 2];
        let val2 = self.data[self.pc() - 1];
        self.pc = Self::compose_to_u16(val2, val1);
    }

    pub fn execute(&mut self, opcode: Opcode) {
        match opcode {
            Opcode::NOP => {
                self.pc += 1;
            }

            Opcode::LXI_B | Opcode::LXI_D | Opcode::LXI_H | Opcode::LXI_SP => {
                if opcode.get_rp_num_2() != 0b11 {
                    let reg1 = opcode.get_rp_num_2() << 1;
                    let reg2 = (opcode.get_rp_num_2() << 1) + 1;
                    self.registers[reg1 as usize] = self.data[self.pc() + 1];
                    self.registers[reg2 as usize] = self.data[self.pc() + 2];
                } else {
                    self.sp = (u16::from(self.data[self.pc() + 1]) << 8)
                        + u16::from(self.data[self.pc() + 2]);
                }
                self.pc += 3;
            }

            Opcode::STAX_B | Opcode::STAX_D => {
                self.pc += 1;
                let reg1 = (opcode.get_rp_num() as usize) << 1;
                let reg2 = (opcode.get_rp_num() << 1) as usize + 1;
                let val1 = self.registers[reg1];
                let val2 = self.registers[reg2];
                self.data[Self::make_address(val1, val2)] = self.acc;
            }

            Opcode::STA => {
                let low_addr = self.data[self.pc() + 1];
                let high_addr = self.data[self.pc() + 2];
                let addr = Self::make_address(high_addr, low_addr);
                self.pc += 3;
                self.data[addr] = self.acc;
            }

            Opcode::LDAX_B | Opcode::LDAX_D => {
                self.pc += 1;
                let reg1 = (opcode.get_rp_num() as usize) << 1;
                let reg2 = (opcode.get_rp_num() << 1) as usize + 1;
                let val1 = self.registers[reg1];
                let val2 = self.registers[reg2];
                self.acc = self.data[Self::make_address(val1, val2)];
            }

            Opcode::LDA => {
                let low_addr = self.data[self.pc() + 1];
                let high_addr = self.data[self.pc() + 2];
                let addr = Self::make_address(high_addr, low_addr);
                self.pc += 3;
                self.acc = self.data[addr];
            }

            Opcode::INX_B | Opcode::INX_D | Opcode::INX_H | Opcode::INX_SP => {
                if opcode.get_rp_num_2() != 0b11 {
                    let reg1 = (opcode.get_rp_num_2() << 1) as usize;
                    let reg2 = (opcode.get_rp_num_2() << 1) as usize + 1;
                    let (val1, val2) = Self::inc(self.registers[reg1], self.registers[reg2]);
                    self.registers[reg1] = val1;
                    self.registers[reg2] = val2;
                } else {
                    self.sp += 1;
                }
                self.pc += 1;
            }

            Opcode::INR_A
            | Opcode::INR_B
            | Opcode::INR_C
            | Opcode::INR_D
            | Opcode::INR_E
            | Opcode::INR_H
            | Opcode::INR_L
            | Opcode::INR_M => {
                let reg = opcode.get_dest_num();
                let data = self.register_or_memory_data(reg);
                self.flag.set_auxiliary_carry_flag(data & 0b0000_1111 == 15);
                let data = data.wrapping_add(1);
                self.update_sign_flag(data);
                self.update_zero_flag(data);
                self.update_parity_flag(data);
                self.set_register_or_memory_data(reg, data);
                self.pc += 1;
            }

            Opcode::DCR_A
            | Opcode::DCR_B
            | Opcode::DCR_C
            | Opcode::DCR_D
            | Opcode::DCR_E
            | Opcode::DCR_H
            | Opcode::DCR_L
            | Opcode::DCR_M => {
                let reg = opcode.get_dest_num();
                let data = self.register_or_memory_data(reg);
                self.flag.set_auxiliary_carry_flag(data & 0b0000_1111 == 15);
                let data = data.wrapping_sub(1);
                self.update_sign_flag(data);
                self.update_zero_flag(data);
                self.update_parity_flag(data);
                self.set_register_or_memory_data(reg, data);
                self.pc += 1;
            }

            Opcode::MVI_A
            | Opcode::MVI_B
            | Opcode::MVI_C
            | Opcode::MVI_D
            | Opcode::MVI_E
            | Opcode::MVI_H
            | Opcode::MVI_L
            | Opcode::MVI_M => {
                let reg = opcode.get_dest_num();
                let val = self.data[self.pc() + 1];
                self.pc += 2;
                self.set_register_or_memory_data(reg, val);
            }

            Opcode::RLC => {
                self.pc += 1;
                self.flag.set_carry_flag(self.acc & 0b1000_0000 != 0);
                self.acc = self.acc.rotate_left(1);
            }

            Opcode::RRC => {
                self.pc += 1;
                self.flag.set_carry_flag(self.acc & 0b0000_00001 != 0);
                self.acc = self.acc.rotate_right(1);
            }

            Opcode::RAL => {
                self.pc += 1;
                self.acc = self.acc.rotate_left(1);
                let b = self.acc | 0b0000_0001;
                if self.flag.carry_flag() {
                    self.acc |= 0b0000_0001;
                } else {
                    self.acc &= 0b1111_1110;
                }
                self.flag.set_carry_flag(b != 0);
            }

            Opcode::RAR => {
                self.pc += 1;
                self.acc = self.acc.rotate_right(1);
                let b = self.acc & 0b1000_0000;
                if self.flag.carry_flag() {
                    self.acc |= 0b1000_0000;
                } else {
                    self.acc &= 0b0111_1111;
                }

                self.flag.set_carry_flag(b != 0);
            }

            Opcode::DAD_B | Opcode::DAD_D | Opcode::DAD_H | Opcode::DAD_SP => {
                if opcode.get_rp_num_2() != 0b11 {
                    let reg1 = (opcode.get_rp_num_2() << 1) as usize;
                    let reg2 = (opcode.get_rp_num_2() << 1) as usize + 1;

                    let (res1, b) =
                        self.registers[Register::L as usize].overflowing_add(self.registers[reg2]);
                    self.registers[Register::L as usize] = res1;

                    if b {
                        let (res2, b) = self.registers[reg1].overflowing_add(1);
                        self.flag.set_carry_flag(b);
                        let (res3, b) = res2.overflowing_add(self.registers[Register::H as usize]);
                        self.flag.set_carry_flag(b);
                        self.registers[Register::H as usize] = res3;
                    } else {
                        let (res2, b) = self.registers[Register::H as usize]
                            .overflowing_add(self.registers[reg1]);

                        self.flag.set_carry_flag(b);
                        self.registers[Register::H as usize] = res2;
                    }
                } else {
                    let (h, l) =
                        Self::decompose_to_u8(self.memory_address().wrapping_add(self.sp()) as u16);
                    self.registers[Register::H as usize] = h;
                    self.registers[Register::L as usize] = l;
                }
                self.pc += 1;
            }

            Opcode::DCX_B | Opcode::DCX_D | Opcode::DCX_H | Opcode::DCX_SP => {
                if opcode.get_rp_num_2() != 0b11 {
                    let reg1 = (opcode.get_rp_num_2() << 1) as usize;
                    let reg2 = (opcode.get_rp_num_2() << 1) as usize + 1;
                    let (val1, val2) = Self::dec(self.registers[reg1], self.registers[reg2]);
                    self.registers[reg1] = val1;
                    self.registers[reg2] = val2;
                } else {
                    self.sp -= 1;
                }
                self.pc += 1;
            }

            Opcode::SHLD => {
                let low_addr = self.data[self.pc() + 1];
                let high_addr = self.data[self.pc() + 2];
                let addr = Self::make_address(high_addr, low_addr);
                self.pc += 3;
                self.data[addr] = self.registers[Register::L as usize];
                self.data[addr + 1] = self.registers[Register::H as usize];
            }

            Opcode::LHLD => {
                let low_addr = self.data[self.pc() + 1];
                let high_addr = self.data[self.pc() + 2];
                let addr = Self::make_address(high_addr, low_addr);
                self.pc += 3;
                self.registers[Register::L as usize] = self.data[addr];
                self.registers[Register::H as usize] = self.data[addr + 1];
            }

            Opcode::CMA => {
                self.pc += 1;
                self.acc ^= 0b1111_1111;
            }

            Opcode::CMC => {
                self.pc += 1;
                self.flag.set_carry_flag(!self.flag.carry_flag());
            }

            Opcode::DAA => {
                self.pc += 1;
                let low = self.acc & 0x0F;

                if low >= 10 {
                    self.acc += 6;
                    self.flag.set_auxiliary_carry_flag(true);
                } else if self.flag.auxiliary_flag() {
                    self.acc += 6;
                    self.flag.set_auxiliary_carry_flag(false);
                }

                let high = (self.acc & 0xF0) >> 4;

                if high >= 10 {
                    self.acc = ((high - 10) << 4) | (self.acc & 0x0F);
                    self.flag.set_carry_flag(true);
                } else if self.flag.carry_flag() {
                    self.acc = ((high + 6) << 4) | (self.acc & 0x0F);
                }

                self.update_zero_flag(self.acc);
                self.update_parity_flag(self.acc);
                self.update_sign_flag(self.acc);
            }

            Opcode::STC => {
                self.pc += 1;
                self.flag.set_carry_flag(true);
            }

            Opcode::IN => {
                let _device = self.data[self.pc() + 1];
                self.pc += 2;
                // todo
            }

            Opcode::OUT => {
                let _device = self.data[self.pc() + 1];
                self.pc += 2;
                // todo
            }

            Opcode::HLT => {
                self.pc += 1;
                self.halted = true;
            }

            Opcode::EI => {
                self.pc += 1;
                self.interrupted = true;
            }

            Opcode::DI => {
                self.pc += 1;
                self.interrupted = false;
            }

            Opcode::RST_0
            | Opcode::RST_1
            | Opcode::RST_2
            | Opcode::RST_3
            | Opcode::RST_4
            | Opcode::RST_5
            | Opcode::RST_6
            | Opcode::RST_7 => {
                self.pc += 1;
                self.stack_push(self.pc);
                self.pc = u16::from(opcode as u8 & 0b0011_1000);
            }

            Opcode::XCHG => {
                self.pc += 1;
                let d = self.registers[Register::D as usize];
                let e = self.registers[Register::E as usize];
                self.registers[Register::D as usize] = self.registers[Register::H as usize];
                self.registers[Register::E as usize] = self.registers[Register::L as usize];
                self.registers[Register::H as usize] = d;
                self.registers[Register::L as usize] = e;
            }

            Opcode::XTHL => {
                self.pc += 1;
                let l = self.stack_pop_u8();
                let h = self.stack_pop_u8();
                self.stack_push_u8(self.registers[Register::H as usize]);
                self.stack_push_u8(self.registers[Register::L as usize]);
                self.registers[Register::H as usize] = h;
                self.registers[Register::L as usize] = l;
            }

            Opcode::PUSH_B | Opcode::PUSH_D | Opcode::PUSH_H | Opcode::PUSH_PSW => {
                self.pc += 1;
                if opcode.get_rp_num_2() != 0b11 {
                    let reg1 = opcode.get_rp_num_2() << 1;
                    let reg2 = (opcode.get_rp_num_2() << 1) + 1;
                    self.stack_push_u8(self.registers[reg1 as usize]);
                    self.stack_push_u8(self.registers[reg2 as usize]);
                } else {
                    self.stack_push_u8(self.acc);
                    self.stack_push_u8(self.flag.value());
                }
            }

            Opcode::POP_B | Opcode::POP_D | Opcode::POP_H | Opcode::POP_PSW => {
                self.pc += 1;
                if opcode.get_rp_num_2() != 0b11 {
                    let reg1 = opcode.get_rp_num_2() << 1;
                    let reg2 = (opcode.get_rp_num_2() << 1) + 1;
                    self.registers[reg2 as usize] = self.stack_pop_u8();
                    self.registers[reg1 as usize] = self.stack_pop_u8();
                } else {
                    let flag = self.stack_pop_u8();
                    self.flag.set_value(flag);
                    self.acc = self.stack_pop_u8();
                }
            }

            Opcode::ADI => {
                let data = self.data[self.pc() + 1];
                self.pc += 2;
                self.flag
                    .set_carry_flag(u16::from(self.acc) + u16::from(data) > 0xff);
                self.update_aux_flag(self.acc, data);
                self.acc = self.acc.wrapping_add(data);
                self.update_parity_flag(self.acc);
                self.update_zero_flag(self.acc);
                self.update_sign_flag(self.acc);
            }

            Opcode::ACI => {
                let carry = self.flag.carry_flag() as u8;
                let data = self.data[self.pc() + 1];
                self.pc += 2;
                self.update_carry_flag_with_carry(self.acc, data, carry);
                self.update_aux_flag_with_carry(self.acc, data, carry);
                self.acc = self.acc.wrapping_add(data).wrapping_add(carry);
                self.update_parity_flag(self.acc);
                self.update_zero_flag(self.acc);
                self.update_sign_flag(self.acc);
            }

            Opcode::SUI => {
                let data = self.data[self.pc() + 1];
                self.pc += 2;
                self.flag.set_carry_flag(self.acc < data);
                self.flag
                    .set_auxiliary_carry_flag((self.acc & 0b0000_1111) >= (data & 0b0000_1111));
                self.acc = self.acc.wrapping_sub(data);
                self.update_parity_flag(self.acc);
                self.update_zero_flag(self.acc);
                self.update_sign_flag(self.acc);
            }

            Opcode::SBI => {
                let data = self.data[self.pc() + 1];
                self.pc += 2;
                let carry = self.flag.carry_flag() as u8;

                self.flag
                    .set_carry_flag(u16::from(self.acc) < u16::from(data) + u16::from(carry));
                self.flag.set_auxiliary_carry_flag(
                    (self.acc & 0b0000_1111) < (data & 0b0000_1111) + carry,
                );
                self.acc = self.acc.wrapping_sub(data).wrapping_sub(carry);
                self.update_parity_flag(self.acc);
                self.update_zero_flag(self.acc);
                self.update_sign_flag(self.acc);
            }

            Opcode::ANI => {
                let data = self.data[self.pc() + 1];
                self.pc += 2;
                self.acc &= data;
                self.flag.set_carry_flag(false);
                self.update_zero_flag(self.acc);
                self.update_sign_flag(self.acc);
                self.update_parity_flag(self.acc);
            }

            Opcode::XRI => {
                let data = self.data[self.pc() + 1];
                self.pc += 2;
                self.acc ^= data;
                self.flag.set_carry_flag(false);
                self.update_zero_flag(self.acc);
                self.update_sign_flag(self.acc);
                self.update_parity_flag(self.acc);
            }

            Opcode::ORI => {
                let data = self.data[self.pc() + 1];
                self.pc += 2;
                self.acc |= data;
                self.flag.set_carry_flag(false);
                self.update_zero_flag(self.acc);
                self.update_sign_flag(self.acc);
                self.update_parity_flag(self.acc);
            }

            Opcode::CPI => {
                let data = self.data[self.pc() + 1];
                self.pc += 2;
                self.flag.set_carry_flag(self.acc < data);
                self.flag
                    .set_auxiliary_carry_flag((self.acc & 0b0000_1111) < (data & 0b0000_1111));
                let res = self.acc.wrapping_sub(data);
                self.update_parity_flag(res);
                self.update_zero_flag(res);
                self.update_sign_flag(res);
            }

            Opcode::JMP => {
                self.pc += 3;
                self.set_jump_pc();
            }

            Opcode::JC => {
                self.pc += 3;
                if self.flag.carry_flag() {
                    self.set_jump_pc();
                }
            }

            Opcode::JNC => {
                self.pc += 3;
                if !self.flag.carry_flag() {
                    self.set_jump_pc();
                }
            }

            Opcode::JZ => {
                self.pc += 3;
                if self.flag.zero_flag() {
                    self.set_jump_pc();
                }
            }

            Opcode::JNZ => {
                self.pc += 3;
                if !self.flag.zero_flag() {
                    self.set_jump_pc();
                }
            }

            Opcode::JM => {
                self.pc += 3;
                if self.flag.sign_flag() {
                    self.set_jump_pc();
                }
            }

            Opcode::JP => {
                self.pc += 3;
                if !self.flag.sign_flag() {
                    self.set_jump_pc();
                }
            }

            Opcode::JPE => {
                self.pc += 3;
                if self.flag.parity_flag() {
                    self.set_jump_pc();
                }
            }

            Opcode::JPO => {
                self.pc += 3;
                if !self.flag.parity_flag() {
                    self.set_jump_pc();
                }
            }

            Opcode::CALL => {
                self.pc += 3;
                self.op_call();
            }

            Opcode::CC => {
                self.pc += 3;
                if self.flag.carry_flag() {
                    self.op_call();
                }
            }

            Opcode::CNC => {
                self.pc += 3;
                if !self.flag.carry_flag() {
                    self.op_call();
                }
            }

            Opcode::CZ => {
                self.pc += 3;
                if self.flag.zero_flag() {
                    self.op_call();
                }
            }

            Opcode::CNZ => {
                self.pc += 3;
                if !self.flag.zero_flag() {
                    self.op_call();
                }
            }

            Opcode::CM => {
                self.pc += 3;
                if self.flag.sign_flag() {
                    self.op_call();
                }
            }

            Opcode::CP => {
                self.pc += 3;
                if !self.flag.sign_flag() {
                    self.op_call();
                }
            }

            Opcode::CPE => {
                self.pc += 3;
                if self.flag.parity_flag() {
                    self.op_call();
                }
            }

            Opcode::CPO => {
                self.pc += 3;
                if !self.flag.parity_flag() {
                    self.op_call();
                }
            }

            Opcode::RET => {
                self.pc += 1;
                self.op_return();
            }

            Opcode::RC => {
                self.pc += 1;
                if self.flag.carry_flag() {
                    self.op_return();
                }
            }

            Opcode::RNC => {
                self.pc += 1;
                if !self.flag.carry_flag() {
                    self.op_return();
                }
            }

            Opcode::RZ => {
                self.pc += 1;
                if self.flag.zero_flag() {
                    self.op_return();
                }
            }
            Opcode::RNZ => {
                self.pc += 1;
                if !self.flag.zero_flag() {
                    self.op_return();
                }
            }

            Opcode::RM => {
                self.pc += 1;
                if self.flag.sign_flag() {
                    self.op_return();
                }
            }

            Opcode::RP => {
                self.pc += 1;
                if !self.flag.sign_flag() {
                    self.op_return();
                }
            }

            Opcode::RPE => {
                self.pc += 1;
                if self.flag.parity_flag() {
                    self.op_return();
                }
            }

            Opcode::RPO => {
                self.pc += 1;
                if !self.flag.parity_flag() {
                    self.op_return();
                }
            }

            Opcode::SPHL => {
                self.pc += 1;
                self.sp = self.memory_address() as u16;
            }

            other => {
                // other instruction's length is 8 bits
                self.pc += 1;

                // mov
                if (other as usize & 0b1100_0000) == 1 << 6 {
                    let dst = opcode.get_dest_num();
                    let src = opcode.get_src_num();

                    if dst != src {
                        let src_data = self.register_or_memory_data(src);
                        self.set_register_or_memory_data(dst, src_data);
                    }
                }

                let alu = other as usize & 0b1111_1000;
                let reg = opcode.get_src_num();
                let data = self.register_or_memory_data(reg);
                // Condition bits affected: Carry, Sign, Zero, Parity, Auxiliary Carry

                // add
                if alu == 0b1000_0000 {
                    self.update_aux_flag(self.acc, data);
                    self.flag
                        .set_carry_flag(u16::from(self.acc) + u16::from(data) > 0xff);
                    self.acc = self.acc.wrapping_add(data);
                    self.update_zero_flag(self.acc);
                    self.update_sign_flag(self.acc);
                    self.update_parity_flag(self.acc);
                }
                // adc
                else if alu == 0b1000_1000 {
                    let carry = self.flag.carry_flag() as u8;
                    self.update_aux_flag_with_carry(self.acc, data, carry);
                    self.update_carry_flag_with_carry(self.acc, data, carry);
                    self.acc = self.acc.wrapping_add(data).wrapping_add(carry);
                    self.update_zero_flag(self.acc);
                    self.update_sign_flag(self.acc);
                    self.update_parity_flag(self.acc);
                }
                // sub
                else if alu == 0b1001_0000 {
                    self.flag.set_auxiliary_carry_flag(self.acc >= data);
                    self.flag.set_carry_flag(self.acc < data);
                    self.acc = self.acc.wrapping_sub(data);
                    self.update_zero_flag(self.acc);
                    self.update_sign_flag(self.acc);
                    self.update_parity_flag(self.acc);
                }
                // sbb
                else if alu == 0b1001_1000 {
                    let carry = self.flag.carry_flag() as u8;
                    self.flag.set_auxiliary_carry_flag(
                        (self.acc & 0b0000_1111) >= (data & 0b0000_1111) + carry,
                    );

                    let data = data.wrapping_add(carry);
                    self.flag.set_carry_flag(self.acc < data);
                    self.acc = self.acc.wrapping_sub(data);
                    self.update_zero_flag(self.acc);
                    self.update_sign_flag(self.acc);
                    self.update_parity_flag(self.acc);
                }
                // ana
                else if alu == 0b1010_0000 {
                    self.acc &= data;
                    self.flag.set_carry_flag(false);
                    self.update_zero_flag(self.acc);
                    self.update_sign_flag(self.acc);
                    self.update_parity_flag(self.acc);
                }
                // xra
                else if alu == 0b1010_1000 {
                    self.acc ^= data;
                    self.flag.set_carry_flag(false);
                    self.update_zero_flag(self.acc);
                    self.update_sign_flag(self.acc);
                    self.update_parity_flag(self.acc);
                }
                // ora
                else if alu == 0b1011_0000 {
                    self.acc |= data;
                    self.flag.set_carry_flag(false);
                    self.update_zero_flag(self.acc);
                    self.update_sign_flag(self.acc);
                    self.update_parity_flag(self.acc);
                }
                // cmp
                else if alu == 0b1011_1000 {
                    self.flag.set_auxiliary_carry_flag(self.acc >= data);
                    self.flag.set_carry_flag(self.acc < data);
                    let res = self.acc.wrapping_sub(data);
                    self.update_zero_flag(res);
                    self.update_sign_flag(res);
                    self.update_parity_flag(res);
                }
            }
        }
    }

    pub fn interrupt(&mut self) {
        // todo
        if self.interrupted {
            self.interrupted = false;
            self.stack_push(self.pc);

        }
    }

    #[inline]
    pub fn run_once(&mut self) {
        if self.halted {
            self.handle_interrupt();
            return;
        }
        self.execute(Opcode::from(self.data[self.pc()]));
        self.handle_interrupt();
    }

    pub fn handle_interrupt(&mut self) {
        if self.interrupted {
            self.interrupted = false;
            self.stack_push(self.pc);
            self.pc = self.interrupted_addr;
        }
    }

    #[inline]
    pub fn send_interrupt(&mut self, addr: u16) {
        self.interrupted_addr = addr;
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn test_NOP() {
        let data = vec![Opcode::NOP.into()];
        let mut cpu = CPU::new(data);
        cpu.run_once();

        assert_eq!(cpu.pc(), 1);
    }

    #[test]
    fn test_LXI() {
        let data = vec![Opcode::LXI_H.into(), 1, 3];
        let mut cpu = CPU::new(data);
        cpu.run_once();

        assert_eq!(cpu.pc(), 3);
        assert_eq!(cpu.registers[Register::H as usize], 1);
        assert_eq!(cpu.registers[Register::L as usize], 3);
    }

    #[test]
    fn test_STAX() {
        let data = vec![Opcode::STAX_B.into(), Opcode::STAX_D.into(), 0, 0];

        let mut cpu = CPU::new(data);
        cpu.registers[Register::B as usize] = 0;
        cpu.registers[Register::C as usize] = 2;
        cpu.acc = 1;
        cpu.run_once();

        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.data[2], 1);

        cpu.registers[Register::D as usize] = 0;
        cpu.registers[Register::E as usize] = 3;
        cpu.acc = 255;
        cpu.run_once();

        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.data[3], 255);
    }

    #[test]
    fn test_STA() {
        let data = vec![Opcode::STA.into(), 3, 0, 0];
        let mut cpu = CPU::new(data);

        cpu.acc = 255;
        cpu.run_once();

        assert_eq!(cpu.pc(), 3);
        assert_eq!(cpu.data[3], 255);
    }

    #[test]
    fn test_LDAX() {
        let data = vec![Opcode::LDAX_B.into(), Opcode::LDAX_D.into(), 1, 255];

        let mut cpu = CPU::new(data);
        cpu.registers[Register::B as usize] = 0;
        cpu.registers[Register::C as usize] = 2;
        cpu.acc = 0;
        cpu.run_once();

        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.acc, 1);

        cpu.registers[Register::D as usize] = 0;
        cpu.registers[Register::E as usize] = 3;
        cpu.acc = 0;
        cpu.run_once();

        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.acc, 255);
    }

    #[test]
    fn test_LDA() {
        let data = vec![Opcode::LDA.into(), 3, 0, 255];

        let mut cpu = CPU::new(data);
        cpu.run_once();

        assert_eq!(cpu.pc(), 3);
        assert_eq!(cpu.acc, 255);
    }

    #[test]
    fn test_INX() {
        let data = vec![Opcode::INX_D.into(), Opcode::INX_SP.into()];
        let mut cpu = CPU::new(data);

        cpu.registers[Register::D as usize] = 0x38;
        cpu.registers[Register::E as usize] = 0xff;
        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.registers[Register::D as usize], 0x39);
        assert_eq!(cpu.registers[Register::E as usize], 0x00);

        let sp = cpu.sp();
        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(sp + 1, cpu.sp());
    }

    #[test]
    fn test_DCX() {
        let data = vec![Opcode::DCX_H.into(), Opcode::DCX_SP.into()];
        let mut cpu = CPU::new(data);

        cpu.registers[Register::H as usize] = 0x98;
        cpu.registers[Register::L as usize] = 0x00;
        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.registers[Register::H as usize], 0x97);
        assert_eq!(cpu.registers[Register::L as usize], 0xff);

        let sp = cpu.sp();
        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(sp - 1, cpu.sp());
    }

    #[test]
    fn test_INR() {
        let data = vec![Opcode::INR_C.into()];
        let mut cpu = CPU::new(data);
        cpu.registers[Register::C as usize] = 0x99;
        cpu.run_once();

        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.registers[Register::C as usize], 0x9A);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.auxiliary_flag(), false);
        assert_eq!(cpu.flag.sign_flag(), true);
    }

    #[test]
    fn test_DCR() {
        let data = vec![Opcode::DCR_M.into(), 0x40];
        let mut cpu = CPU::new(data);
        cpu.set_memory_address(1);
        cpu.run_once();

        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.data[1], 0x3f);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.auxiliary_flag(), false);
        assert_eq!(cpu.flag.sign_flag(), false);
    }

    #[test]
    fn test_MVI() {
        let data = vec![Opcode::MVI_A.into(), 1, Opcode::MVI_M.into(), 255];
        let mut cpu = CPU::new(data);

        cpu.acc = 0;
        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.acc, 1);

        cpu.set_memory_address(1);
        cpu.run_once();
        assert_eq!(cpu.pc(), 4);
        assert_eq!(cpu.data[1], 255);
    }

    #[test]
    fn test_RLC() {
        let data = vec![Opcode::RLC.into()];
        let mut cpu = CPU::new(data);

        cpu.acc = 0xf2;
        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.acc, 0xe5);
        assert_eq!(cpu.flag.carry_flag(), true);
    }

    #[test]
    fn test_RRC() {
        let data = vec![Opcode::RRC.into()];
        let mut cpu = CPU::new(data);

        cpu.acc = 0xf2;
        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.acc, 0x79);
        assert_eq!(cpu.flag.carry_flag(), false);
    }

    #[test]
    fn test_RAL() {
        let data = vec![Opcode::RAL.into()];
        let mut cpu = CPU::new(data);

        cpu.acc = 0xb5;
        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.acc, 0x6a);
        assert_eq!(cpu.flag.carry_flag(), true);
    }

    #[test]
    fn test_RAR() {
        let data = vec![Opcode::RAR.into()];
        let mut cpu = CPU::new(data);

        cpu.acc = 0x6a;
        cpu.flag.set_carry_flag(true);
        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.acc, 0xb5);
        assert_eq!(cpu.flag.carry_flag(), false);
    }

    #[test]
    fn test_SHLD() {
        let data = vec![Opcode::SHLD.into(), 3, 0, 0, 0];
        let mut cpu = CPU::new(data);
        cpu.registers[Register::L as usize] = 1;
        cpu.registers[Register::H as usize] = 2;

        cpu.run_once();
        assert_eq!(cpu.pc(), 3);
        assert_eq!(cpu.data[3], 1);
        assert_eq!(cpu.data[4], 2);
    }

    #[test]
    fn test_LHLD() {
        let data = vec![Opcode::LHLD.into(), 3, 0, 1, 2];
        let mut cpu = CPU::new(data);

        cpu.run_once();
        assert_eq!(cpu.pc(), 3);
        assert_eq!(cpu.registers[Register::L as usize], 1);
        assert_eq!(cpu.registers[Register::H as usize], 2);
    }

    #[test]
    fn test_DAA() {
        let data = vec![Opcode::DAA.into()];
        let mut cpu = CPU::new(data);

        cpu.acc = 0x9b;
        cpu.run_once();

        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.acc, 1);
        assert_eq!(cpu.flag.carry_flag(), true);
        assert_eq!(cpu.flag.auxiliary_flag(), true);
    }

    #[test]
    fn test_DAD() {
        // sp == 5
        let data = vec![Opcode::DAD_B.into(), Opcode::DAD_SP.into(), 0, 0, 0];
        let mut cpu = CPU::new(data);

        cpu.registers[Register::B as usize] = 0x33;
        cpu.registers[Register::C as usize] = 0x9f;
        cpu.registers[Register::H as usize] = 0xa1;
        cpu.registers[Register::L as usize] = 0x7b;
        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.registers[Register::H as usize], 0xd5);
        assert_eq!(cpu.registers[Register::L as usize], 0x1a);

        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.registers[Register::H as usize], 0xd5);
        assert_eq!(cpu.registers[Register::L as usize], 0x1f);
    }

    #[test]
    fn test_STC() {
        let data = vec![Opcode::STC.into()];
        let mut cpu = CPU::new(data);

        assert_eq!(cpu.flag.carry_flag(), false);
        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.flag.carry_flag(), true);
    }

    #[test]
    fn test_CMA() {
        let data = vec![Opcode::CMA.into()];
        let mut cpu = CPU::new(data);

        cpu.acc = 0x51;
        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.acc, 0xae);
    }

    #[test]
    fn test_CMC() {
        let data = vec![Opcode::CMC.into(), Opcode::CMC.into()];
        let mut cpu = CPU::new(data);

        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.flag.carry_flag(), true);

        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.flag.carry_flag(), false);
    }

    #[test]
    fn test_MOV() {
        let data = vec![
            Opcode::MOV_AE.into(),
            Opcode::MOV_DD.into(),
            Opcode::MOV_MA.into(),
            0,
        ];
        let mut cpu = CPU::new(data);

        cpu.registers[Register::E as usize] = 1;
        cpu.registers[Register::D as usize] = 2;
        cpu.set_memory_address(3);

        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.acc, 1);

        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.registers[Register::D as usize], 2);

        cpu.acc = 3;
        cpu.run_once();
        assert_eq!(cpu.pc(), 3);
        assert_eq!(cpu.data[3], 3);
    }

    #[test]
    fn test_HLT() {
        let data = vec![Opcode::HLT.into()];
        let mut cpu = CPU::new(data);

        assert_eq!(cpu.halted, false);
        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.halted, true);
    }

    #[test]
    fn test_ADD() {
        let data = vec![Opcode::ADD_D.into(), Opcode::ADD_M.into(), 2];
        let mut cpu = CPU::new(data);

        cpu.registers[Register::D as usize] = 0x2e;
        cpu.acc = 0x6c;
        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.parity_flag(), true);
        assert_eq!(cpu.flag.sign_flag(), true);
        assert_eq!(cpu.acc, 0x9a);

        cpu.set_memory_address(2);
        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.parity_flag(), true);
        assert_eq!(cpu.flag.sign_flag(), true);
        assert_eq!(cpu.acc, 0x9c);
    }

    #[test]
    fn test_ADC() {
        let data = vec![Opcode::ADC_C.into(), Opcode::ADC_C.into()];
        let mut cpu = CPU::new(data);

        cpu.registers[Register::C as usize] = 0x3d;
        cpu.acc = 0x42;
        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.parity_flag(), false);
        assert_eq!(cpu.flag.sign_flag(), false);
        assert_eq!(cpu.flag.auxiliary_flag(), false);
        assert_eq!(cpu.acc, 0x7f);

        cpu.registers[Register::C as usize] = 0x3d;
        cpu.acc = 0x42;
        cpu.flag.set_carry_flag(true);
        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.parity_flag(), false);
        assert_eq!(cpu.flag.sign_flag(), true);
        assert_eq!(cpu.flag.auxiliary_flag(), true);
        assert_eq!(cpu.acc, 0x80);
    }

    #[test]
    fn test_SUB() {
        let data = vec![Opcode::SUB_B.into()];
        let mut cpu = CPU::new(data);

        cpu.acc = 0x3e;
        cpu.registers[Register::B as usize] = 0x3e;
        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.acc, 0);
        assert_eq!(cpu.flag.zero_flag(), true);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.auxiliary_flag(), true);
        assert_eq!(cpu.flag.parity_flag(), true);
        assert_eq!(cpu.flag.sign_flag(), false);
    }

    #[test]
    fn test_SBB() {
        let data = vec![Opcode::SBB_L.into()];
        let mut cpu = CPU::new(data);

        cpu.registers[Register::L as usize] = 2;
        cpu.acc = 4;
        cpu.flag.set_carry_flag(true);

        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.parity_flag(), false);
        assert_eq!(cpu.flag.sign_flag(), false);
        assert_eq!(cpu.flag.auxiliary_flag(), true);
        assert_eq!(cpu.acc, 1);
    }

    #[test]
    fn test_ANA() {
        let data = vec![Opcode::ANA_C.into()];
        let mut cpu = CPU::new(data);

        cpu.registers[Register::C as usize] = 0x0f;
        cpu.acc = 0xfc;

        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.parity_flag(), true);
        assert_eq!(cpu.flag.sign_flag(), false);
        assert_eq!(cpu.acc, 0x0c);
    }

    #[test]
    fn test_XRA() {
        let data = vec![Opcode::XRA_B.into()];
        let mut cpu = CPU::new(data);

        cpu.registers[Register::B as usize] = 0b0101_1100;
        cpu.acc = 0b0111_1000;

        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.parity_flag(), true);
        assert_eq!(cpu.flag.sign_flag(), false);
        //        assert_eq!(cpu.flag.auxiliary_flag(), false);
        assert_eq!(cpu.acc, 0b0010_0100);
    }

    #[test]
    fn test_ORA() {
        let data = vec![Opcode::ORA_C.into()];
        let mut cpu = CPU::new(data);

        cpu.registers[Register::C as usize] = 0x0f;
        cpu.acc = 0x33;

        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.parity_flag(), true);
        assert_eq!(cpu.flag.sign_flag(), false);
        //        assert_eq!(cpu.flag.auxiliary_flag(), false);
        assert_eq!(cpu.acc, 0x3f);
    }

    #[test]
    fn test_CMP() {
        let data = vec![Opcode::CMP_E.into()];
        let mut cpu = CPU::new(data);

        cpu.registers[Register::E as usize] = 0x5;
        cpu.acc = 0xa;

        cpu.run_once();
        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.parity_flag(), true);
        assert_eq!(cpu.flag.sign_flag(), false);
        assert_eq!(cpu.flag.auxiliary_flag(), true);
        assert_eq!(cpu.acc, 0xa);
    }

    #[test]
    fn test_RET() {
        let data = vec![Opcode::RET.into(), 0, 0, Opcode::NOP.into(), 0, 0];
        let mut cpu = CPU::new(data);

        // return to nop
        cpu.stack_push(3);
        assert_eq!(cpu.data[cpu.data.len() - 2], 3);
        cpu.run_once();
        assert_eq!(cpu.pc(), 3);
    }

    #[test]
    fn test_CALL() {
        let data = vec![
            Opcode::CALL.into(),
            4,
            0,
            Opcode::NOP.into(),
            Opcode::NOP.into(),
            Opcode::RET.into(),
            0,
            0,
            0,
            0,
        ];
        let mut cpu = CPU::new(data);

        cpu.run_once();
        assert_eq!(cpu.pc(), 4);

        cpu.run_once();
        assert_eq!(cpu.pc(), 5);

        cpu.run_once();
        assert_eq!(cpu.pc(), 3);

        cpu.run_once();
        assert_eq!(cpu.pc(), 4);
    }

    #[test]
    fn test_PUSH() {
        let data = vec![
            Opcode::PUSH_D.into(),
            Opcode::PUSH_PSW.into(),
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let mut cpu = CPU::new(data);

        cpu.registers[Register::D as usize] = 0x8f;
        cpu.registers[Register::E as usize] = 0x9d;
        cpu.run_once();

        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.data[cpu.data.len() - 1], 0x8f);
        assert_eq!(cpu.data[cpu.data.len() - 2], 0x9d);

        cpu.acc = 0x1f;
        cpu.flag.set_carry_flag(true);
        cpu.flag.set_zero_flag(true);
        cpu.flag.set_parity_flag(true);
        cpu.flag.set_sign_flag(false);
        cpu.flag.set_auxiliary_carry_flag(false);
        cpu.run_once();

        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.data[cpu.data.len() - 3], 0x1f);
        assert_eq!(cpu.data[cpu.data.len() - 4], 0x47);
    }

    #[test]
    fn test_POP() {
        let data = vec![
            Opcode::POP_H.into(),
            Opcode::PUSH_PSW.into(),
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let mut cpu = CPU::new(data);

        cpu.stack_push(0x933d);
        cpu.run_once();

        assert_eq!(cpu.pc(), 1);
        assert_eq!(cpu.registers[Register::H as usize], 0x93);
        assert_eq!(cpu.registers[Register::L as usize], 0x3d);
    }

    #[test]
    fn test_JMP() {
        // loop
        let data = vec![Opcode::JMP.into(), 3, 0, Opcode::JMP.into(), 0, 0];
        let mut cpu = CPU::new(data);

        cpu.run_once();
        assert_eq!(cpu.pc(), 3);

        cpu.run_once();
        assert_eq!(cpu.pc(), 0);

        cpu.run_once();
        assert_eq!(cpu.pc(), 3);
    }

    #[test]
    fn test_ADI() {
        let data = vec![Opcode::ADI.into(), 0x42];
        let mut cpu = CPU::new(data);

        cpu.acc = 0x14;
        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.acc, 0x56);
        assert_eq!(cpu.flag.parity_flag(), true);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.auxiliary_flag(), false);
        assert_eq!(cpu.flag.sign_flag(), false);
    }

    #[test]
    fn test_SUI() {
        let data = vec![Opcode::SUI.into(), 1];
        let mut cpu = CPU::new(data);

        cpu.acc = 0;
        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.acc, 0xFF);
        assert_eq!(cpu.flag.parity_flag(), true);
        assert_eq!(cpu.flag.sign_flag(), true);
        assert_eq!(cpu.flag.carry_flag(), true);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.auxiliary_flag(), false);
    }

    #[test]
    fn test_ANI() {
        let data = vec![Opcode::ANI.into(), 0x0f];
        let mut cpu = CPU::new(data);

        cpu.acc = 0x3a;
        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.acc, 0x0a);
        assert_eq!(cpu.flag.parity_flag(), true);
        assert_eq!(cpu.flag.sign_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.zero_flag(), false);
    }

    #[test]
    fn test_ORI() {
        let data = vec![Opcode::ORI.into(), 0x0f];
        let mut cpu = CPU::new(data);

        cpu.acc = 0xb5;
        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.acc, 0xbf);
        assert_eq!(cpu.flag.parity_flag(), false);
        assert_eq!(cpu.flag.sign_flag(), true);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.zero_flag(), false);
    }

    #[test]
    fn test_ACI() {
        let data = vec![Opcode::ACI.into(), 0x42];
        let mut cpu = CPU::new(data);

        cpu.flag.set_carry_flag(true);
        cpu.acc = 0x14;
        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.acc, 0x57);
        assert_eq!(cpu.flag.parity_flag(), false);
        assert_eq!(cpu.flag.sign_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.auxiliary_flag(), false);
    }

    #[test]
    fn test_SBI() {
        let data = vec![Opcode::SBI.into(), 0x1];
        let mut cpu = CPU::new(data);

        cpu.flag.set_carry_flag(true);
        cpu.acc = 0;
        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.acc, 0xFE);
        assert_eq!(cpu.flag.parity_flag(), false);
        assert_eq!(cpu.flag.sign_flag(), true);
        assert_eq!(cpu.flag.carry_flag(), true);
        assert_eq!(cpu.flag.zero_flag(), false);
        // todo: check
        assert_eq!(cpu.flag.auxiliary_flag(), true);
    }

    #[test]
    fn test_XRI() {
        let data = vec![Opcode::XRI.into(), 0x81];
        let mut cpu = CPU::new(data);

        cpu.acc = 0x3b;
        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.acc, 0xBA);
        assert_eq!(cpu.flag.parity_flag(), false);
        assert_eq!(cpu.flag.sign_flag(), true);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.zero_flag(), false);
    }

    #[test]
    fn test_CPI() {
        let data = vec![Opcode::CPI.into(), 0x40];
        let mut cpu = CPU::new(data);

        cpu.acc = 0x4a;
        cpu.run_once();
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cpu.acc, 0x4a);
        assert_eq!(cpu.flag.parity_flag(), true);
        assert_eq!(cpu.flag.sign_flag(), false);
        assert_eq!(cpu.flag.carry_flag(), false);
        assert_eq!(cpu.flag.zero_flag(), false);
        assert_eq!(cpu.flag.auxiliary_flag(), false);
    }

    #[test]
    fn test_SPHL() {
        let data = vec![Opcode::SPHL.into()];
        let mut cpu = CPU::new(data);

        cpu.registers[Register::H as usize] = 0x50;
        cpu.registers[Register::L as usize] = 0x6c;
        cpu.run_once();
        assert_eq!(cpu.sp(), 0x506c);
    }

    #[test]
    fn test_XCHG() {
        let data = vec![Opcode::XCHG.into()];
        let mut cpu = CPU::new(data);

        cpu.registers[Register::H as usize] = 0x00;
        cpu.registers[Register::L as usize] = 0xFF;
        cpu.registers[Register::D as usize] = 0x33;
        cpu.registers[Register::E as usize] = 0x55;

        cpu.run_once();

        assert_eq!(cpu.registers[Register::H as usize], 0x33);
        assert_eq!(cpu.registers[Register::L as usize], 0x55);
        assert_eq!(cpu.registers[Register::D as usize], 0x00);
        assert_eq!(cpu.registers[Register::E as usize], 0xFF);
    }

    #[test]
    fn test_XTHL() {
        let data = vec![Opcode::XTHL.into(), 0, 1, 2, 3];
        let mut cpu = CPU::new(data);
        cpu.stack_push(0x0DF0);

        cpu.registers[Register::H as usize] = 0x0B;
        cpu.registers[Register::L as usize] = 0x3C;
        cpu.run_once();

        assert_eq!(cpu.registers[Register::H as usize], 0x0D);
        assert_eq!(cpu.registers[Register::L as usize], 0xF0);
        assert_eq!(cpu.data[cpu.data.len() - 1], 0x0B);
        assert_eq!(cpu.data[cpu.data.len() - 2], 0x3C);
    }
}
