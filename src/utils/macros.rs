macro_rules! export_crate {
    ($name:ident) => {
        pub(crate) use $name as $name;
    };
}
export_crate![export_crate];

pub mod async_rt {
    use super::export_crate;

    macro_rules! block_on_io {
        ($async:expr) => {
            tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build()
                .expect("Could not build async runtime with io.")
                .block_on($async);
        };
    }
    export_crate![block_on_io];

    macro_rules! block_on {
        ($async:expr) => {
            tokio::runtime::Builder::new_current_thread()
                .build()
                .expect("Could not build runtime.")
                .block_on($async);
        };
    }
    export_crate![block_on];
}

pub mod multithread {
    use super::export_crate;

    macro_rules! acquire_lock_panic {
        ($lock_expr:expr, $source:literal) => {
            {
                match $lock_expr {
                    #[allow(unused_mut)]
                    Ok(mut result) => { result },
                    Err(poison) => {
                        $crate::utils::logger::Logger::error(
                            format!(
                                "{}\n{}\n{}",
                                format!(
                                    concat!(
                                        "!!FATAL ERROR!! A thread panicked while holding the lock in '",
                                        $source,
                                        "' at line {}.",
                                    ),
                                    line!(),
                                ),
                                format!("Source: {}", poison.source().unwrap()),
                                format!("Description: {}", poison),
                            ).as_str()
                        );
                        panic!();
                    },
                }
            }
        };
    }
    export_crate![acquire_lock_panic];
}

pub mod utilities {
    use super::export_crate;

    macro_rules! expand_option {
        () => { None };
        ($val:expr) => { Some($val) };
    }
    export_crate![expand_option];
}
