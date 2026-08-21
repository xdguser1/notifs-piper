macro_rules! export_crate {
    ($name:ident) => {
        pub(crate) use $name;
    };
}

pub mod async_rt {
    macro_rules! block_on_io {
        ($async:expr) => {
            {
                tokio::runtime::Builder::new_current_thread()
                    .enable_io()
                    .build()
                    .expect("Could not build async runtime with io.")
                    .block_on($async)
            }
        };
    }
    export_crate![block_on_io];
}

pub mod multithread {
    macro_rules! acquire_lock_panic {
        ($lock_expr:expr, $source:literal, $($rest:stmt)*) => {
            {
                match $lock_expr {
                    #[allow(unused_mut)]
                    Ok(mut result) => { result },
                    Err(poison) => {
                        $crate::utils::logger::Logger::error(
                            format!(
                                "{}\n{}",
                                format!(
                                    concat!(
                                        "!!FATAL ERROR!! A thread panicked while holding the lock in '",
                                        $source,
                                        "' at line {}.",
                                    ),
                                    line!(),
                                ),
                                format!("Description: {}", poison),
                            ).as_str()
                        );
                        $($rest)*
                        panic!();
                    },
                }
            }
        };
    }
    export_crate![acquire_lock_panic];
}

pub mod parse {
    macro_rules! split_once {
        ($data:ident, $delimiter:literal) => {{
            $data.split_once($delimiter).ok_or_else(|| {
                $crate::server::transmission::PayloadError::new(
                    $data.to_owned(),
                    concat!(
                        "Pattern [",
                        stringify!($delimiter),
                        "] was not found in split.",
                    )
                    .to_owned(),
                    String::new(),
                )
            })
        }};
        ($data:ident, $delimiter:pat_param) => {{
            $data.split_once($delimiter).ok_or_else(|| {
                $crate::server::transmission::PayloadError::new(
                    $data.to_owned(),
                    concat!(
                        "Pattern [",
                        stringify!($delimiter),
                        "] was not found in split.",
                    )
                    .to_owned(),
                    String::new(),
                )
            })
        }};
    }
    export_crate![split_once];

    macro_rules! parse {
        ($data:ident, $type:ty, $struct:literal) => {{
            $data.parse::<$type>().map_err(|err| {
                $crate::server::transmission::PayloadError::new(
                    $data.to_owned(),
                    concat!(
                        "Invalid parsing for type '",
                        stringify!($type),
                        "' in structure '",
                        $struct,
                        "'.",
                    )
                    .to_owned(),
                    err.to_string(),
                )
            })
        }};
    }
    export_crate![parse];
}

pub mod utilities {
    macro_rules! expand_option {
        () => {
            None
        };
        ($val:expr) => {
            Some($val)
        };
    }
    export_crate![expand_option];

    macro_rules! try_block {
        ($pat:expr, $out:lifetime) => {(
            match $pat {
                Ok(val) => { val },
                Err(err) => { break $out Err(err.into()); },
            }
        )};
    }
    export_crate![try_block];
}
