pub mod opamp {
    //! The opamp module contains all those entities defined by the
    //! [Opamp specification](https://github.com/open-telemetry/opamp-spec/blob/main/specification.md)
    pub mod proto {
        //! The proto module contains the protobuffers structures defined by the
        //! [Opamp specification](https://github.com/open-telemetry/opamp-spec/blob/main/specification.md)
        include!(concat!(env!("OUT_DIR"), "/opamp.proto.rs"));
    }
}
