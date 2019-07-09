use intel8080::cpu::CPU;
use intel8080::register::Register;
use std::fs::File;
use std::io::Read;
use std::io::Result;
use std::path::Path;

fn main() -> Result<()> {
    run_rom("rom/CPUTEST.COM")?;
    run_rom("rom/8080EXM.COM")?;
    run_rom("rom/8080PRE.COM")?;
    run_rom("rom/TST8080.COM")?;
    Ok(())
}

fn load_rom(data: &mut Vec<u8>, path: impl AsRef<Path>) -> Result<()> {
    let mut file = File::open(path)?;
    let mut buf = vec![];
    file.read_to_end(&mut buf)?;
    data[0x100..(buf.len() + 0x100)].clone_from_slice(&buf[..]);
    Ok(())
}

fn run_rom(path: impl AsRef<Path>) -> Result<()> {
    println!("running: {:?}", path.as_ref());
    println!("----------------------------------");
    let mut data = vec![0; 65536];
    load_rom(&mut data, path)?;
    let mut cpu = CPU::new(data);
    cpu.set_value(0x0005, 0xC9);
    cpu.set_pc(0x0100);
    loop {
        if cpu.is_halted() {
            break;
        }
        cpu.run_once();

//        if cpu.pc() == 0x05 {
//            let c = cpu.registers[Register::C as usize];
//            if c == 0x09 {
//                let d = cpu.registers[Register::D as usize];
//                let e = cpu.registers[Register::E as usize];
//                let mut a = CPU::make_address(d, e);
//                loop {
//                    let ch = cpu.get_value(a);
//                    if ch as char == '$' {
//                        break;
//                    } else {
//                        a = a.wrapping_add(1);
//                    }
//                    print!("{}", c as char);
//                }
//            } else if c == 0x02 {
//                print!("{}", cpu.registers[Register::E as usize] as char);
//            }
//        }
//
        if cpu.pc() == 0x00 {
            println!("finish");
            break;
        }
    }

    Ok(())
}
