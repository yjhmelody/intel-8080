//! DDD represents a destination register. SSS represents a source register. Both DDD and SSS are interpreted as follows:
//!
//! |DDD or SSS|Interpretation|
//! |----------|-------------|
//! |000|Register B|
//! |001|Register C|
//! |010|Register D|
//! |011|Register E|
//! |100|Register H|
//! |101|Register L|
//! |110|A memory register|
//! |111|The accumulator|

use std::convert::Into;

impl Into<usize> for Register {
    fn into(self) -> usize {
        self as usize
    }
}

impl From<usize> for Register {
    fn from(n: usize) -> Self {
        match n {
            0 => Register::B,
            1 => Register::C,
            2 => Register::D,
            3 => Register::E,
            4 => Register::H,
            5 => Register::L,
            6 => Register::Mem,
            7 => Register::Acc,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Register {
    B = 0b000,
    C = 0b001,
    D = 0b010,
    E = 0b011,
    H = 0b100,
    L = 0b101,
    Mem = 0b110,
    Acc = 0b111,
}

#[derive(Debug, Clone, Copy)]
pub struct Flag(u8);

impl Default for Flag {
    fn default() -> Self {
        Self(0b0000_0010)
    }
}

impl Flag {
    #[inline]
    pub fn value(&self) -> u8 {
        self.0
    }

    #[inline]
    pub fn set_value(&mut self, val: u8) {
        self.0 = val;
    }

    #[inline]
    pub fn new(b: u8) -> Self {
        Self(b)
    }

    #[inline]
    pub fn set_carry_flag(&mut self, b: bool) {
        if b {
            self.0 |= 0b0000_0001;
        } else {
            self.0 &= 0b1111_1110;
        }
    }

    #[inline]
    pub fn carry_flag(&self) -> bool {
        self.0 & 0b0000_0001 == 1
    }

    #[inline]
    pub fn set_parity_flag(&mut self, b: bool) {
        if b {
            self.0 |= 0b0000_0100;
        } else {
            self.0 &= 0b1111_1011;
        }
    }

    #[inline]
    pub fn parity_flag(&self) -> bool {
        self.0 & 0b0000_0100 == 1 << 2
    }

    #[inline]
    pub fn set_auxiliary_carry_flag(&mut self, b: bool) {
        if b {
            self.0 |= 0b0001_0000;
        } else {
            self.0 &= 0b1110_1111;
        }
    }

    #[inline]
    pub fn auxiliary_flag(&self) -> bool {
        self.0 & 0b0001_0000 == 1 << 4
    }

    #[inline]
    pub fn set_zero_flag(&mut self, b: bool) {
        if b {
            self.0 |= 0b0100_0000;
        } else {
            self.0 &= 0b1011_1111;
        }
    }

    #[inline]
    pub fn zero_flag(&self) -> bool {
        (self.0 & 0b0100_0000) == (1 << 6)
    }

    #[inline]
    pub fn set_sign_flag(&mut self, b: bool) {
        if b {
            self.0 |= 0b1000_0000;
        } else {
            self.0 &= 0b0111_1111;
        }
    }

    #[inline]
    pub fn sign_flag(&mut self) -> bool {
        self.0 & 0b1000_0000 == (1 << 7)
    }
}
