pub mod gen {
    include!(concat!(env!("OUT_DIR"), "/greplog.v1.rs"));
}

pub mod redact;
pub mod schema;
