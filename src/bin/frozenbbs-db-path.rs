use frozenbbs::{admin, BBSConfig, BBS_TAG};

fn main() {
    let cfg: BBSConfig = confy::load(BBS_TAG, "config").unwrap();
    admin::db_path(cfg)
}
