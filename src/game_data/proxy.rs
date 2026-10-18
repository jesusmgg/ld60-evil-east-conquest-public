use macroquad::math::IVec2;
use nanoserde::{DeBin, SerBin};
use std::collections::HashMap;

fn tile_proxy_sort_key<T>(entry: &(IVec2Proxy, T)) -> (i32, i32) {
    (entry.0.y, entry.0.x)
}

macro_rules! vec2_proxy_impl {
    ($original:ty, $proxy:ident, $vec_proxy:ident, $constructor:path) => {
        impl Into<$proxy> for &$original {
            fn into(self) -> $proxy {
                $proxy {
                    x: self.x,
                    y: self.y,
                }
            }
        }

        impl Into<$original> for &$proxy {
            fn into(self) -> $original {
                $constructor(self.x, self.y)
            }
        }

        impl Into<$original> for $proxy {
            fn into(self) -> $original {
                $constructor(self.x, self.y)
            }
        }

        impl Into<$vec_proxy> for &Vec<$original> {
            fn into(self) -> $vec_proxy {
                let mut out = Vec::with_capacity(self.len());
                for v in self.iter() {
                    out.push(v.into());
                }
                $vec_proxy(out)
            }
        }

        impl Into<Vec<$original>> for &$vec_proxy {
            fn into(self) -> Vec<$original> {
                let mut out = Vec::with_capacity(self.0.len());
                for v in self.0.iter() {
                    out.push(v.into());
                }
                out
            }
        }
    };
}

#[derive(Clone, Copy, DeBin, SerBin)]
pub struct IVec2Proxy {
    pub x: i32,
    pub y: i32,
}

#[derive(DeBin, SerBin)]
pub struct VecIVec2Proxy(pub Vec<IVec2Proxy>);

vec2_proxy_impl!(IVec2, IVec2Proxy, VecIVec2Proxy, IVec2::new);

macro_rules! hashmap_ivec2_proxy_impl {
    ($value_type:ty, $proxy_name:ident) => {
        impl Into<$proxy_name> for &HashMap<IVec2, $value_type> {
            fn into(self) -> $proxy_name {
                let mut out = Vec::with_capacity(self.len());
                for (k, v) in self.iter() {
                    out.push((k.into(), v.clone()));
                }
                out.sort_by_key(tile_proxy_sort_key);
                $proxy_name(out)
            }
        }

        impl Into<HashMap<IVec2, $value_type>> for &$proxy_name {
            fn into(self) -> HashMap<IVec2, $value_type> {
                let mut out = HashMap::with_capacity(self.0.len());
                for (k, v) in self.0.iter() {
                    out.insert(k.into(), v.clone());
                }
                out
            }
        }
    };
}

#[derive(DeBin, SerBin)]
pub struct HashMapIVec2TileTypeProxy(pub Vec<(IVec2Proxy, crate::level::TileType)>);

hashmap_ivec2_proxy_impl!(crate::level::TileType, HashMapIVec2TileTypeProxy);

#[derive(DeBin, SerBin)]
pub struct HashMapIVec2InteractableProxy(pub Vec<(IVec2Proxy, crate::level::Interactable)>);

hashmap_ivec2_proxy_impl!(crate::level::Interactable, HashMapIVec2InteractableProxy);

#[derive(DeBin, SerBin)]
pub struct HashMapIVec2UnitSpawnDataProxy(
    pub Vec<(IVec2Proxy, crate::unit_manager::UnitSpawnData)>,
);

hashmap_ivec2_proxy_impl!(
    crate::unit_manager::UnitSpawnData,
    HashMapIVec2UnitSpawnDataProxy
);

#[derive(DeBin, SerBin)]
pub struct HashMapIVec2StringProxy(pub Vec<(IVec2Proxy, String)>);

hashmap_ivec2_proxy_impl!(String, HashMapIVec2StringProxy);
