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
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    data[0x0100..(buf.len() + 0x0100)].clone_from_slice(&buf[..]);
    Ok(())
}

fn run_rom(path: impl AsRef<Path>) -> Result<()> {
    println!("running: {:?}", path.as_ref());
    println!("----------------------------------");
    let mut data = vec![0; 65536];
    load_rom(&mut data, path);

    let mut cpu = CPU::new(data);
    cpu.set_value(0x0005, 0xC9);
    // Because tests used the pseudo instruction ORG 0x0100
    cpu.set_pc(0x0100);
    loop {
        if cpu.is_halted() {
            break;
        }
        cpu.run_once();

//        0DF5    F5                    bdos:	push	psw
//        0DF6    C5                    	push	b
//        0DF7    D5                    	push	d
//        0DF8    E5                    	push	h
//        0DF9    CD 0005               	call	5
//        0DFC    E1                    	pop	h
//        0DFD    D1                    	pop	d
//        0DFE    C1                    	pop	b
//        0DFF    F1                    	pop	psw
//        0E00    C9                    	ret

        if cpu.pc() == 0x05 {
            let c = cpu.registers[Register::C as usize];
            if c == 0x09 {
                let d = cpu.registers[Register::D as usize];
                let e = cpu.registers[Register::E as usize];
                let mut addr = CPU::make_address(d, e) as u16;

                loop {
                    let ch = cpu.get_value(addr as usize);
                    if ch == b'$' {
                        break;
                    } else {
                        addr = addr.wrapping_add(1);
                    }
                    print!("{}", ch as char);
                }
            } else if c == 0x02 {
                print!("{}", cpu.registers[Register::E as usize] as char);
            }
        }

        if cpu.pc() == 0x00 {
            println!("\nfinish\n");
            break;
        }
    }

    Ok(())
}
