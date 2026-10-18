#[macro_export]
macro_rules! impl_versioned_migration {
    ($type:ty; $version:ident, $offset:ident, $bytes:ident, $debug_log:ident {
        $(
            $ver:pat => default
        ),* $(,)?
    }) => {
        impl $crate::game_data::game_data::Versioned for $type {
            fn migrate_from(
                $version: usize,
                $offset: &mut usize,
                $bytes: &[u8],
                _path: &str,
                $debug_log: &mut $crate::debug::DebugLog,
            ) -> Option<Self> {
                use $crate::game_data::game_data::DATA_VERSION;
                use nanoserde::DeBin;

                if $version == DATA_VERSION {
                    return Self::de_bin($offset, $bytes).ok();
                }
                if $version > DATA_VERSION {
                    return None;
                }

                match $version {
                    $( $ver )|* => {
                        $debug_log.log(format!(
                            "Migrated {} from version {} to version {}",
                            std::any::type_name::<Self>(),
                            $version,
                            DATA_VERSION
                        ));
                        return Self::de_bin($offset, $bytes).ok();
                    }
                    _ => return None,
                }
            }
        }
    };

    ($type:ty; $version:ident, $offset:ident, $bytes:ident, $debug_log:ident {
        $(
            $ver:pat => direct {
                $( $field:ident = $deser:expr ),* $(,)?
            }
        ),* $(,)?
    }) => {
        impl $crate::game_data::game_data::Versioned for $type {
            fn migrate_from(
                $version: usize,
                $offset: &mut usize,
                $bytes: &[u8],
                _path: &str,
                $debug_log: &mut $crate::debug::DebugLog,
            ) -> Option<Self> {
                use $crate::game_data::game_data::DATA_VERSION;
                use nanoserde::DeBin;

                if $version == DATA_VERSION {
                    return Self::de_bin($offset, $bytes).ok();
                }
                if $version > DATA_VERSION {
                    return None;
                }

                match $version {
                    $(
                        $ver => {
                            $( let $field = $deser; )*
                            $debug_log.log(format!(
                                "Migrated {} from version {} to version {}",
                                std::any::type_name::<Self>(),
                                $version,
                                DATA_VERSION
                            ));
                            Some(Self { $( $field, )* })
                        }
                    )*
                    _ => None,
                }
            }
        }
    };
}

#[macro_export]
macro_rules! deser_proxy {
    ($proxy_type:ty, $offset:expr, $bytes:expr) => {{
        let proxy = <$proxy_type>::de_bin($offset, $bytes).ok()?;
        (&proxy).into()
    }};

    ($proxy_type:ty => $target_type:ty, $offset:expr, $bytes:expr) => {{
        let proxy = <$proxy_type>::de_bin($offset, $bytes).ok()?;
        let result: $target_type = (&proxy).into();
        result
    }};
}

#[macro_export]
macro_rules! define_struct_hashmap_deserializer {
    (
        $fn_name:ident,
        $key_proxy:ty => $key:ty,
        $struct:ident; $version:ident, $offset:ident, $bytes:ident => {
            $(
                $ver:pat => {
                    $( $field:ident = $deser:expr ),* $(,)?
                }
            ),* $(,)?
        }
    ) => {
        pub fn $fn_name(
            $version: usize,
            $offset: &mut usize,
            $bytes: &[u8],
        ) -> Option<std::collections::HashMap<$key, $struct>> {
            use nanoserde::DeBin;
            use std::collections::HashMap;

            let len = usize::de_bin($offset, $bytes).ok()?;
            let mut map = HashMap::new();

            match $version {
                $(
                    $ver => {
                        for _ in 0..len {
                            let key: $key = <$key_proxy>::de_bin($offset, $bytes).ok()?.into();
                            $( let $field = $deser; )*
                            map.insert(key, $struct { $( $field, )* });
                        }
                    }
                )*
                _ => {
                    for _ in 0..len {
                        let key: $key = <$key_proxy>::de_bin($offset, $bytes).ok()?.into();
                        let value = <$struct>::de_bin($offset, $bytes).ok()?;
                        map.insert(key, value);
                    }
                }
            }

            Some(map)
        }
    };
}
