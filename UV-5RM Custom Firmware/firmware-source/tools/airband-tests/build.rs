use std::{env, fs, path::Path};
fn main() {
    let abi = fs::read_to_string("../../abi/src/lib.rs").unwrap();
    let source = abi
        .split("pub struct Api {")
        .nth(1)
        .unwrap()
        .split("\n}")
        .next()
        .unwrap();
    let overrides = [
        "uptime_100us",
        "nor_read",
        "nor_write",
        "nor_erase_sector",
        "set_rx_freq",
        "set_modulation",
        "set_tx_enabled",
        "set_ptt",
        "tone_on",
        "settings_get",
        "squelch_open",
        "read_rssi",
        "handoff_read",
        "handoff_write",
        "draw_text",
        "fill_rect",
        "fmt_u32",
    ];
    let mut code = String::from(
        "#[allow(unused_variables)] mod generated { use bf5rh_abi::*;\n",
    );
    let mut fields = String::from("pub fn api()->Api { Api {\n");
    let source = source
        .lines()
        .map(|l| l.split("//").next().unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    for field in source.split("pub ").skip(1) {
        let clean = field
            .lines()
            .filter(|l| !l.trim().starts_with("//"))
            .collect::<Vec<_>>()
            .join(" ");
        let clean = clean.trim();
        if clean.is_empty() {
            continue;
        }
        let (name, ty) = clean.split_once(':').unwrap();
        let name = name.trim();
        let ty = ty.trim().trim_end_matches(',').trim();
        if overrides.contains(&name) {
            fields += &format!("{name}:super::sim_{name},\n");
        } else {
            let sig = ty.replacen("fn(", &format!("fn stub_{name}("), 1);
            let result =
                ty.rsplit_once("->").map(|(_, s)| s.trim()).unwrap_or("()");
            let body = match result {
                "bool" => "false",
                "u8" | "u16" | "u32" | "i32" => "0",
                "!" => "panic!(\"unexpected app fault\")",
                _ => "()",
            };
            code += &format!("{sig} {{ {body} }}\n");
            fields += &format!("{name}:stub_{name},\n");
        }
    }
    code += &fields;
    code += "} }\n}\n";
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("api.rs"),
        code,
    )
    .unwrap();
    println!("cargo:rerun-if-changed=../../abi/src/lib.rs");
}
