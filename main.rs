use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::process::{Command, Stdio};
use regex::Regex;

// Change READELF to match your toolchain
const READELF: &str = "arm-linux-readelf -S";

// Change these to match your files
const U_BOOT: &str = "u-boot";
const AIS_NAND_FILE: &str = "u-boot_nand.ais";
const AIS_UART_FILE: &str = "u-boot_uart.ais";

const LOAD: u32 = 0x58535901;
const JUMP_CLOSE: u32 = 0x58535906;

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}


// Adjust headers (mainly PLL and mDDR/DDR2 configuration) to your board.
fn main() {
    let ais_nand_header = [
        hex("41504954"), // Magic word
        hex("5853590d"), // Function Execute Command
        hex("00020000"), // PLL0 Configuration (Index = 0, Argument Count = 2)
        hex("00180001"),
        hex("00000205"),
        hex("5853590d"), // Function Execute Command
        hex("00080003"), // mDDR/DDR2 Controller Configuration (Index = 3, Argument Count = 8)
        hex("20020001"),
        hex("00000002"),
        hex("000000c4"),
        hex("02074622"),
        hex("129129c8"),
        hex("380f7000"),
        hex("0000040d"),
        hex("00000500"),
        hex("5853590d"), // Function Execute Command
        hex("00050005"), // EMIFA Async Configuration (Index = 5, Argument Count = 5)
        hex("00000000"),
        hex("081221ac"),
        hex("00000000"),
        hex("00000000"),
        hex("00000002"),
    ];

    let ais_uart_header = [
        hex("41504954"), // Magic word
        hex("5853590d"), // Function Execute Command
        hex("00030006"), // PLL and Clock Configuration(Index= 6, Argument Count= 3)
        hex("00180001"),
        hex("00000b05"),
        hex("00010064"),
        hex("5853590d"), // Function Execute Command
        hex("00080003"), // mDDR/DDR2 Controller Configuration (Index = 3, Argument Count = 8)
        hex("18010101"),
        hex("00000002"),
        hex("00000003"),
        hex("06074622"),
        hex("20da3291"),
        hex("42948941"),
        hex("00000492"),
        hex("00000500"),
    ];

    let u_boot = U_BOOT;
    let ais_nand_file = AIS_NAND_FILE;
    let ais_uart_file = AIS_UART_FILE;

    let readelf_output = Command::new("sh")
        .arg("-c")
        .arg(format!("{} {}", READELF, u_boot))
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run readelf command")
        .stdout
        .expect("Failed to open readelf stdout");

    let reader = BufReader::new(readelf_output);

    let mut ais_nand_file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(ais_nand_file)
        .expect("Unable to open AIS NAND file");

    let mut ais_uart_file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(ais_uart_file)
        .expect("Unable to open AIS UART file");

    for &header in &ais_nand_header {
        ais_nand_file.write_all(&header.to_le_bytes()).expect("Unable to write to AIS NAND file");
    }

    for &header in &ais_uart_header {
        ais_uart_file.write_all(&header.to_le_bytes()).expect("Unable to write to AIS UART file");
    }

    let mut u_boot_file = File::open(u_boot).expect("Unable to open U-Boot file");

    let mut start_addr = 0;
    let regex = Regex::new(r"\.(text|rodata|data|u_boot_cmd)\s+PROGBITS\s+([0-9a-f]+)\s([0-9a-f]+)\s([0-9a-f]+)\s").unwrap();

    for line in reader.lines() {
        let line = line.expect("Failed to read line from readelf output");

        if let Some(captures) = regex.captures(&line) {
            let name = &captures[1];
            let addr = u32::from_str_radix(&captures[2], 16).unwrap();
            let offset = u32::from_str_radix(&captures[3], 16).unwrap();
            let size = u32::from_str_radix(&captures[4], 16).unwrap();

            if name == "text" {
                start_addr = addr;
            }

            println!("{} {:x} {:x} {:x}", name, addr, offset, size);

            ais_nand_file.write_all(&LOAD.to_le_bytes()).expect("Unable to write to AIS NAND file");
            ais_nand_file.write_all(&addr.to_le_bytes()).expect("Unable to write to AIS NAND file");

            ais_uart_file.write_all(&LOAD.to_le_bytes()).expect("Unable to write to AIS UART file");
            ais_uart_file.write_all(&addr.to_le_bytes()).expect("Unable to write to AIS UART file");

            let rest = (4 - (size % 4)) % 4;
            println!("Rest is {} for section .{}", rest, name);

            ais_nand_file.write_all(&(size + rest).to_le_bytes()).expect("Unable to write to AIS NAND file");
            ais_uart_file.write_all(&(size + rest).to_le_bytes()).expect("Unable to write to AIS UART file");

            let mut section = vec![0; size as usize];
            u_boot_file.seek(SeekFrom::Start(offset as u64)).expect("Couldn't seek to offset");
            u_boot_file.read_exact(&mut section).expect("Couldn't read section");

            ais_nand_file.write_all(&section).expect("Unable to write to AIS NAND file");
            ais_uart_file.write_all(&section).expect("Unable to write to AIS UART file");

            let padding = vec![0; rest as usize];
            ais_nand_file.write_all(&padding).expect("Unable to write to AIS NAND file");
            ais_uart_file.write_all(&padding).expect("Unable to write to AIS UART file");
        }
    }

    ais_nand_file.write_all(&JUMP_CLOSE.to_le_bytes()).expect("Unable to write to AIS NAND file");
    ais_nand_file.write_all(&start_addr.to_le_bytes()).expect("Unable to write to AIS NAND file");

    ais_uart_file.write_all(&JUMP_CLOSE.to_le_bytes()).expect("Unable to write to AIS UART file");
    ais_uart_file.write_all(&start_addr.to_le_bytes()).expect("Unable to write to AIS UART file");
}
