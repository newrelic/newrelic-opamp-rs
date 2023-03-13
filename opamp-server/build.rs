use std::io::Result;

fn main() -> Result<()> {
    // compiling protos using path on build time
    prost_build::compile_protos(
        &["../proto/opamp.proto", "../proto/anyvalue.proto"],
        &["../proto/"],
    )?;
    Ok(())
}
