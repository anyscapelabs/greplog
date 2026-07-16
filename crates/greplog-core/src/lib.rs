pub mod gen {
    include!(concat!(env!("OUT_DIR"), "/greplog.v1.rs"));
}

pub mod arrow_schema;
pub mod redact;
pub mod schema;
