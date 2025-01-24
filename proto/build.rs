use std::io::Result;
use std::path::PathBuf;
use std::{env, fs};
extern crate prost_build;

fn main() -> Result<()> {
    // Building proto files requires a tool PROTOC to be installed.
    // To avoid the need to have this tool on any project that depends on this crate
    // we are building the proto files locally and committing the generated files.
    if env::var("REBUILD_PROTO").is_err() {
        return Ok(());
    }

    let src_path = PathBuf::from("./src");
    prost_build::Config::new()
        .out_dir(src_path.as_path())
        .skip_debug([
            "RemoteConfigStatus",
            "AgentRemoteConfig",
            "AgentConfigMap",
            "AgentConfigFile",
            "CustomMessage",
            "AnyValue",
        ])
        .compile_protos(
            &[
                "./opamp-spec/proto/opamp.proto",
                "./opamp-spec/proto/anyvalue.proto",
            ],
            &["./opamp-spec/proto/"],
        )?;
    fs::rename(src_path.join("opamp.proto.rs"), src_path.join("proto.rs"))?;
    Ok(())
}
