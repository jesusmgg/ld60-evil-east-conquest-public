#[macro_export]
macro_rules! simple_enum_serbin {
    ($enum_name:ident, $const_name:ident { $($variant:ident => $discriminant:expr),* $(,)? }) => {
        simple_enum_serbin!($enum_name { $($variant => $discriminant),* });

        pub const $const_name: &[$enum_name] = &[
            $(
                $enum_name::$variant,
            )*
        ];
    };

    ($enum_name:ident { $($variant:ident => $discriminant:expr),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $enum_name {
            $(
                $variant,
            )*
        }

        impl $enum_name {
            pub fn to_index(&self) -> usize {
                match self {
                    $(
                        $enum_name::$variant => $discriminant,
                    )*
                }
            }

            pub fn from_index(index: usize) -> Self {
                match index {
                    $(
                        $discriminant => $enum_name::$variant,
                    )*
                    _ => Self::all_variants()[0],
                }
            }

            pub fn variant_names() -> Vec<String> {
                vec![$( stringify!($variant).to_string() ),*]
            }

            pub fn name(&self) -> &'static str {
                match self {
                    $(
                        $enum_name::$variant => stringify!($variant),
                    )*
                }
            }

            pub fn all_variants() -> &'static [Self] {
                &[$( $enum_name::$variant ),*]
            }

            pub fn to_sequential_index(&self) -> usize {
                for (i, v) in Self::all_variants().iter().enumerate() {
                    if v == self {
                        return i;
                    }
                }
                0
            }

            pub fn from_sequential_index(index: usize) -> Self {
                let variants = Self::all_variants();
                variants.get(index).copied().unwrap_or(variants[0])
            }
        }

        impl ::nanoserde::SerBin for $enum_name {
            fn ser_bin(&self, output: &mut Vec<u8>) {
                let discriminant: u16 = match self {
                    $(
                        $enum_name::$variant => $discriminant,
                    )*
                };
                discriminant.ser_bin(output);
            }
        }

        impl ::nanoserde::DeBin for $enum_name {
            fn de_bin(offset: &mut usize, bytes: &[u8]) -> Result<Self, ::nanoserde::DeBinErr> {
                let discriminant = u16::de_bin(offset, bytes)?;
                match discriminant {
                    $(
                        $discriminant => Ok($enum_name::$variant),
                    )*
                    _ => Err(::nanoserde::DeBinErr::new(*offset, 2, 0)),
                }
            }
        }
    };
}

#[macro_export]
macro_rules! enum_wrapper_serbin {
    ($enum_name:ident, $const_name:ident { $($variant:ident($inner_type:ty) => $discriminant:expr),* $(,)? }) => {
        pub fn $const_name() -> Vec<$enum_name> {
            vec![
                $(
                    $enum_name::$variant(<$inner_type>::default()),
                )*
            ]
        }

        impl $enum_name {
            paste::paste! {
                $(
                    pub const [<DISC_ $variant:upper>]: usize = $discriminant;
                )*
            }
        }

        impl ::nanoserde::SerBin for $enum_name {
            fn ser_bin(&self, output: &mut Vec<u8>) {
                match self {
                    $(
                        $enum_name::$variant(inner) => {
                            ($discriminant as u16).ser_bin(output);
                            inner.ser_bin(output);
                        }
                    )*
                }
            }
        }

        impl ::nanoserde::DeBin for $enum_name {
            fn de_bin(offset: &mut usize, bytes: &[u8]) -> Result<Self, ::nanoserde::DeBinErr> {
                let discriminant = u16::de_bin(offset, bytes)?;
                match discriminant {
                    $(
                        $discriminant => {
                            let inner = <$inner_type>::de_bin(offset, bytes)?;
                            Ok($enum_name::$variant(inner))
                        }
                    )*
                    _ => Err(::nanoserde::DeBinErr::new(*offset, 2, 0)),
                }
            }
        }
    };

    ($enum_name:ident { $($variant:ident($inner_type:ty) => $discriminant:expr),* $(,)? }) => {
        impl $enum_name {
            paste::paste! {
                $(
                    pub const [<DISC_ $variant:upper>]: usize = $discriminant;
                )*
            }
        }

        impl ::nanoserde::SerBin for $enum_name {
            fn ser_bin(&self, output: &mut Vec<u8>) {
                match self {
                    $(
                        $enum_name::$variant(inner) => {
                            ($discriminant as u16).ser_bin(output);
                            inner.ser_bin(output);
                        }
                    )*
                }
            }
        }

        impl ::nanoserde::DeBin for $enum_name {
            fn de_bin(offset: &mut usize, bytes: &[u8]) -> Result<Self, ::nanoserde::DeBinErr> {
                let discriminant = u16::de_bin(offset, bytes)?;
                match discriminant {
                    $(
                        $discriminant => {
                            let inner = <$inner_type>::de_bin(offset, bytes)?;
                            Ok($enum_name::$variant(inner))
                        }
                    )*
                    _ => Err(::nanoserde::DeBinErr::new(*offset, 2, 0)),
                }
            }
        }
    };
}
