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

pub mod parse {
    use super::export_crate;

    macro_rules! split_once {
        ($data:ident, $delimiter:literal) => {
            {
                $data.split_once($delimiter).ok_or_else(|| $crate::server::transmission::PayloadError::new(
                    $data.to_owned(),
                    concat!(
                        "Pattern [",
                        stringify!($delimiter),
                        "] was not found in split.",
                    ).to_owned(),
                    String::new(),
                ))?
            }
        };
        ($data:ident, $delimiter:pat_param) => {
            {
                $data.split_once($delimiter).ok_or_else(|| $crate::server::transmission::PayloadError::new(
                    $data.to_owned(),
                    concat!(
                        "Pattern [",
                        stringify!($delimiter),
                        "] was not found in split.",
                    ).to_owned(),
                    String::new(),
                ))?
            }
        };
    }
    export_crate![split_once];

    macro_rules! parse {
        ($data:ident, $type:ty, $struct:literal) => {
            {
                $data.parse::<$type>().map_err(|err| {
                    $crate::server::transmission::PayloadError::new(
                        $data.to_owned(),
                        concat!(
                            "Invalid parsing for type '",
                            stringify!($type),
                            "' in structure '",
                            $struct,
                            "'.",
                        ).to_owned(),
                        err.to_string(),
                    )
                })?
            }
        };
    }
    export_crate![parse];
}

pub mod utilities {
    use super::export_crate;

    macro_rules! expand_option {
        () => { None };
        ($val:expr) => { Some($val) };
    }
    export_crate![expand_option];
}
