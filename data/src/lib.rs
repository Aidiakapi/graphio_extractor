extern crate num_bigint;
extern crate num_rational;
extern crate serde;
extern crate string_interner;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;

mod serde_int;
mod serde_option_ratio;
mod serde_ratio;

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::num::NonZeroU32;
use std::ops::Deref;
use std::u32;
use std::sync::RwLock;
use serde::{Serialize, Serializer, Deserialize, Deserializer};

pub type Int = num_bigint::BigInt;
pub type Ratio = num_rational::BigRational;

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug)]
pub struct Str(NonZeroU32);

// ID definitions

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub struct ItemID(pub Str);
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub struct FluidID(pub Str);
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub struct RecipeID(pub Str);
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub struct MachineID(pub Str);
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub struct BeaconID(pub Str);

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug)]
pub enum ID {
    Item(ItemID),
    Fluid(FluidID),
    Recipe(RecipeID),
    Machine(MachineID),
    Beacon(BeaconID),
}

// Data definitions

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemID,
    #[serde(flatten)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fluid {
    pub id: FluidID,
    #[serde(flatten)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub id: RecipeID,
    #[serde(flatten)]
    pub metadata: Metadata,
    #[serde(with = "serde_ratio")]
    pub time: Ratio,
    pub ingredients: Vec<Ingredient>,
    pub products: Vec<Product>,
    pub crafted_in: HashSet<MachineID>,
    pub supported_modules: HashSet<ItemID>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ingredient {
    #[serde(flatten)]
    pub resource: IngredientResource,
    #[serde(with = "serde_ratio")]
    pub amount: Ratio,
    #[serde(with = "serde_ratio")]
    pub catalyst_amount: Ratio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngredientResource {
    Item {
        id: ItemID,
    },
    Fluid {
        id: FluidID,
        #[serde(
            with = "serde_option_ratio",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        minimum_temperature: Option<Ratio>,
        #[serde(
            with = "serde_option_ratio",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        maximum_temperature: Option<Ratio>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    #[serde(flatten)]
    pub resource: ProductResource,
    #[serde(flatten)]
    pub amount: ProductAmount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductResource {
    Item {
        id: ItemID,
    },
    Fluid {
        id: FluidID,
        #[serde(with = "serde_ratio")]
        temperature: Ratio,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductAmount {
    Fixed {
        #[serde(with = "serde_ratio")]
        amount: Ratio,
        #[serde(with = "serde_ratio")]
        catalyst_amount: Ratio,
    },
    Probability {
        #[serde(with = "serde_ratio")]
        amount_min: Ratio,
        #[serde(with = "serde_ratio")]
        amount_max: Ratio,
        #[serde(with = "serde_ratio")]
        probability: Ratio,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Machine {
    pub id: MachineID,
    #[serde(flatten)]
    pub metadata: Metadata,
    #[serde(with = "serde_ratio")]
    pub crafting_speed: Ratio,
    #[serde(with = "serde_ratio")]
    pub energy_consumption: Ratio,
    #[serde(with = "serde_ratio")]
    pub energy_drain: Ratio,
    #[serde(with = "serde_int")]
    pub module_slots: Int,
    pub supported_modules: HashSet<ItemID>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Beacon {
    pub id: BeaconID,
    #[serde(flatten)]
    pub metadata: Metadata,
    #[serde(with = "serde_ratio")]
    pub distribution_effectivity: Ratio,
    pub supported_modules: HashSet<ItemID>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub id: ItemID,
    #[serde(with = "serde_ratio")]
    pub modifier_energy: Ratio,
    #[serde(with = "serde_ratio")]
    pub modifier_speed: Ratio,
    #[serde(with = "serde_ratio")]
    pub modifier_productivity: Ratio,
    #[serde(with = "serde_ratio")]
    pub modifier_pollution: Ratio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub localised_name: Str,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub localised_description: Option<Str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<Icon>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileMetadata {
    pub tile_size: (u32, u32),
    pub tile_count: u32,
    pub image_size: (u32, u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_metadata: Option<TileMetadata>,
    pub items: HashSet<Item>,
    pub fluids: HashSet<Fluid>,
    pub recipes: HashSet<Recipe>,
    pub machines: HashSet<Machine>,
    pub beacons: HashSet<Beacon>,
    pub modules: HashSet<Module>,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub struct Icon(NonZeroU32);

pub trait GameObject {
    type Target;
    fn try_resolve<'s, 'd>(&'s self, game_data: &'d GameData) -> Option<&'d Self::Target>;
    fn resolve<'s, 'd>(&'s self, game_data: &'d GameData) -> &'d Self::Target {
        self.try_resolve(game_data).expect("unable to resolve game object")
    }
}

pub trait MetadataObject {
    fn try_metadata<'s, 'd>(&'s self, game_data: &'d GameData) -> Option<&'d Metadata>;
    fn metadata<'s, 'd>(&'s self, game_data: &'d GameData) -> &'d Metadata {
        self.try_metadata(game_data).expect("unable to resolve game object")
    }
}

// Objects implement Hash, PartialEq, Eq, and Borrow in order
// to use the IDs to access the full objects whilst stored in
// a hashset. The identity of any object is determined by the
// ID, and not by any other field.
// In an actual correct instance of GameData, this can never
// be an issue, but filling it with arbitrary data, it can be
// an issue.

macro_rules! hash_by_id {
    ($id:ty, $t:ty) => {
        impl PartialEq for $t {
            fn eq(&self, other: &Self) -> bool {
                self.id.eq(&other.id)
            }
        }

        impl Eq for $t {}

        impl Hash for $t {
            fn hash<H: Hasher>(&self, h: &mut H) {
                self.id.hash(h);
            }
        }

        impl ::std::borrow::Borrow<$id> for $t {
            fn borrow(&self) -> &$id {
                &self.id
            }
        }
    };
}

macro_rules! implement_game_object {
    ($id:ty, $t:ty, $collection:ident) => {
        hash_by_id!($id, $t);

        impl $id {
            pub fn str(&self) -> &'static str { self.0.str() }
        }

        impl AsRef<Str> for $id {
            fn as_ref(&self) -> &Str { &self.0 }
        }

        impl GameObject for $id {
            type Target = $t;
            fn try_resolve<'s, 'd>(&'s self, game_data: &'d GameData) -> Option<&'d $t> {
                game_data.$collection.get(self)
            }
        }

        impl MetadataObject for $id {
            fn try_metadata<'s, 'd>(&'s self, game_data: &'d GameData) -> Option<&'d Metadata> {
                self.try_resolve(game_data).map(|x| &x.metadata)
            }
        }
    };
}

implement_game_object!(ItemID, Item, items);
implement_game_object!(FluidID, Fluid, fluids);
implement_game_object!(RecipeID, Recipe, recipes);
implement_game_object!(MachineID, Machine, machines);
implement_game_object!(BeaconID, Beacon, beacons);
hash_by_id!(ItemID, Module);

macro_rules! forward_to_id_variant {
    ($self:ident, $method:ident) => {
        forward_to_id_variant!($self, $method, )
    };
    ($self:ident, $method:ident, $($expr:expr),*) => {
        match $self {
            ID::Item(id) => id.$method($($expr),*),
            ID::Fluid(id) => id.$method($($expr),*),
            ID::Recipe(id) => id.$method($($expr),*),
            ID::Machine(id) => id.$method($($expr),*),
            ID::Beacon(id) => id.$method($($expr),*),
        }
    };
}

impl ID {
    pub fn str(&self) -> &'static str {
        forward_to_id_variant!(self, str)
    }
}

impl AsRef<Str> for ID {
    fn as_ref(&self) -> &Str {
        forward_to_id_variant!(self, as_ref)
    }
}

impl MetadataObject for ID {
    fn try_metadata<'s, 'd>(&'s self, game_data: &'d GameData) -> Option<&'d Metadata> {
        forward_to_id_variant!(self, try_metadata, game_data)
    }
    
    fn metadata<'s, 'd>(&'s self, game_data: &'d GameData) -> &'d Metadata {
        forward_to_id_variant!(self, metadata, game_data)
    }
}

impl Icon {
    pub fn position(&self, tile_metadata: &TileMetadata) -> (u32, u32) {
        let columns = tile_metadata.image_size.0 / tile_metadata.tile_size.0;
        let idx = self.index() as u32;
        let x = idx % columns;
        let y = idx / columns;
        (x * tile_metadata.tile_size.0, y * tile_metadata.tile_size.1)
    }

    pub fn index(&self) -> usize {
        self.0.get() as usize - 1
    }

    pub fn new(idx: usize) -> Icon {
        assert!(idx < u32::MAX as usize);
        Icon(unsafe { NonZeroU32::new_unchecked(idx as u32 + 1) })
    }
}

impl GameData {
    pub fn modify_metadata<E, F>(&mut self, f: F) -> Result<(), E>
        where F : Fn(ID, &Metadata) -> Result<Metadata, E>
    {
        macro_rules! set_metadata {
            ($field:ident, $type:ident) => {
                self.$field = self.$field
                    .iter()
                    .map(|entry| {
                        let metadata = f(ID::$type(entry.id), &entry.metadata)?;
                        Ok($type {
                            metadata,
                            ..entry.clone()
                        })
                    })
                    .collect::<Result<HashSet<_>, E>>()?;
            };
        }
        set_metadata!(items, Item);
        set_metadata!(fluids, Fluid);
        set_metadata!(recipes, Recipe);
        set_metadata!(machines, Machine);
        set_metadata!(beacons, Beacon);
        Ok(())
    }
}

// String interning and (de)serializing
type Interner = string_interner::StringInterner<StrSym>;
lazy_static! {
    static ref INTERNER: RwLock<Interner> = {
        RwLock::new(Interner::new())
    };
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug)]
struct StrSym(NonZeroU32);

impl string_interner::Symbol for StrSym {
    /// # Panics
    /// Will panic if `val` >= `u32::MAX`.
    fn from_usize(val: usize) -> Self {
        assert!(val < u32::MAX as usize);
        StrSym(unsafe { NonZeroU32::new_unchecked((val + 1) as u32) })
    }

    fn to_usize(self) -> usize {
        (self.0.get() as usize) - 1
    }
}

impl Str {
    pub fn new(s: &str) -> Str {
        let mut lock = INTERNER.write().unwrap();
        Str(lock.get_or_intern(s).0)
    }

    pub fn str(&self) -> &'static str {
        let lock = INTERNER.read().unwrap();
        unsafe {
            let ptr = lock.resolve_unchecked(StrSym(self.0)) as *const str;
            &*ptr
        }
    }
}

impl Deref for Str {
    type Target = str;

    fn deref(&self) -> &str { self.str() }
}

impl Serialize for Str {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        str::serialize(self.str(), serializer)
    }
}

impl<'de> Deserialize<'de> for Str {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Str, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Str::new(&s))
    }
}
