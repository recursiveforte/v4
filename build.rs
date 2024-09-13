use std::fs::File;
use std::io::Write;
use isocountry::CountryCode;

const POP_THRESHOLD: u32 = 40_000;

fn main() {
    println!("cargo::rerun-if-changed=src/include/allCountries.txt");

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(File::open("src/include/allCountries.txt").unwrap());
    let mut writer = File::create(std::env::var("OUT_DIR").unwrap() + "/cities.in").unwrap();

    let mut total = 0;

    writer.write(b"[").unwrap();
    for result in reader.records() {
        let row = result.unwrap();
        if row.get(6).unwrap() == "P" && row.get(7).unwrap() != "PPLX"
        && row.get(14).unwrap().parse::<u32>().unwrap() > POP_THRESHOLD {
            if let Ok(country) = CountryCode::for_alpha2(row.get(8).unwrap()) {
                total += 1;
                writer.write(&*format!(
                    r#"&Location {{ name: "{}", state: "{}", country: CountryCode::{}, lat: {}_f64, long: {}_f64 }},"#,
                    row.get(1).unwrap(), // name
                    row.get(10).unwrap(), // state
                    country.alpha3(), // country ISO code
                    row.get(4).unwrap(), // lat
                    row.get(5).unwrap() // long
                ).into_bytes()).unwrap();
            }
        }
    }
    writer.write(b"]").unwrap();

    File::create(std::env::var("OUT_DIR").unwrap() + "/cities_len.in").unwrap().write_all(total.to_string().as_ref()).unwrap();
}