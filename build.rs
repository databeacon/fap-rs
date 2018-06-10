extern crate bindgen;
extern crate cc;
// extern crate autotools;

use std::env;
use std::path::PathBuf;

fn main() {
      let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
      let libfap_dir = "src/libfap-1.5";
 
      cc::Build::new()
            .include(format!("{}/src", libfap_dir))
            .files([
                  format!("{}/src/fap.c", libfap_dir),
                  format!("{}/src/helpers.c", libfap_dir),
                  format!("{}/src/helpers2.c", libfap_dir),
            ].iter())
            .warnings(false)
            .compile("libfap");

      // let libfap_install_dir = autotools::build(libfap_dir);
      
      // println!("cargo:rustc-link-search=native={}", libfap_install_dir.display());
      // println!("cargo:rustc-link-lib=static=fap");

      bindgen::Builder::default()
        .clang_arg("-Wall")
        .clang_arg("-Wextra")
        .header(format!("{}/src/fap.h", libfap_dir))
        .whitelist_type("fap_packet_t")
        .whitelist_function("fap_init")
        .whitelist_function("fap_free")
        .whitelist_function("fap_explain_error")
        .whitelist_function("fap_parseaprs")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings!");

}
