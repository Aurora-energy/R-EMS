//! R-EMS Schema Library
//!
//! Re-exports generated protobuf modules so that other crates can depend on a
//! stable interface. The actual definitions will be populated in Phase 1.

pub mod ems {
    pub mod core {
        pub mod v1 {
            tonic::include_proto!("ems.core.v1");
        }
    }
}
