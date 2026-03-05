use vergen_gix::{BuildBuilder, Emitter, GixBuilder};

fn main() {
    let build = BuildBuilder::default()
        .build_timestamp(true)
        .use_local(true)
        .build()
        .unwrap();
    let gix = GixBuilder::all_git().unwrap();
    Emitter::default()
        .add_instructions(&build)
        .unwrap()
        .add_instructions(&gix)
        .unwrap()
        .emit()
        .unwrap();
}
