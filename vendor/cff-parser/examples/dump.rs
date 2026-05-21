use std::io::Read;

use cff_parser::{Table, string_by_id};
use cff_parser::charset::Charset;

fn main() {
    // open the file passed on the comanmand line
    let path = std::env::args().nth(1).expect("no file given");
    let file = std::fs::File::open(&path).expect("could not open file");
    let mut reader = std::io::BufReader::new(file);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).expect("could not read file");
    let table = Table::parse(&buffer).unwrap();
    dbg!(&table);
    match table.charset {

        Charset::ISOAdobe => println!("ISOAdobe"),
        Charset::Expert => println!("Expert"),
        Charset::ExpertSubset => println!("ExpertSubset"),
        Charset::Format0(ref array) => {
            println!("Format0:");
            for sid in array.clone() {
                println!("  {:?}", string_by_id(&table, sid));
            }
        }
        Charset::Format1(ref array) => {
            println!("Format1:");
            for range in array.clone() {
                let sid = range.first;
                println!("  {:?}", string_by_id(&table, sid));
            }
        }
        Charset::Format2(ref array) => {
            println!("Format2:");
            for range in array.clone() {
                let sid = range.first;
                println!("  {:?}", string_by_id(&table, sid));
            }
        }
    }
    match table.encoding.kind {
        cff_parser::EncodingKind::Standard => println!("Standard"),
        cff_parser::EncodingKind::Expert => println!("Expert"),
        cff_parser::EncodingKind::Format0(ref array) => {
            println!("Format0:");
            for code in array.clone() {
                println!("  {:?}", code);
            }
        }
        cff_parser::EncodingKind::Format1(ref array) => {
            println!("Format1:");
            for range in array.clone() {
                println!("  {:?} {:?}", range.first, range.left);
            }
        }
    }
}