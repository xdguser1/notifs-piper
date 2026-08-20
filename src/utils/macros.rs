pub mod async_rt {
    macro_rules! block_on_io {
        ($async:expr) => {
            tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build()
                .expect("Could not build async runtime with io.")
                .block_on($async);
        };
    }
    pub(crate) use block_on_io as block_on_io;

    macro_rules! block_on {
        ($async:expr) => {
            tokio::runtime::Builder::new_current_thread()
                .build()
                .expect("Could not build runtime.")
                .block_on($async);
        };
    }
    pub(crate) use block_on as block_on;
}

pub mod utilities {
    macro_rules! expand_option {
        () => { None };
        ($val:expr) => { Some($val) };
    }
    pub(crate) use expand_option as expand_option;
}
