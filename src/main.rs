#[macro_use]
extern crate clap;
extern crate dirs;
extern crate graphio_rs_data;
extern crate image;
extern crate itertools;
extern crate num_traits;
extern crate serde_json;

mod factorio_io;
mod parsing;

use crate::factorio_io::{
    create_dir_safely, write_file_safely, FactorioPaths, TempDirectory, TempFile,
};
use graphio_rs_data::{self as data, GameData};
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn main() {
    match main_io() {
        Ok(_) => (),
        Err(err) => {
            eprintln!("{}", err);
        }
    }
}

enum PruneLevel {
    NoPruning,
    BasicPruning,
    ExtensivePruning,
}

fn main_io() -> io::Result<()> {
    use clap::{App, Arg};
    let app = App::new("graphio_rs_extractor")
        .version(crate_version!())
        .about("Tool to extract data from the game Factorio, for use in the Graphio tool.")
        .arg(
            Arg::with_name("directory")
                .index(1)
                .help("The directory of the Factorio game")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("stage")
                .long("stage")
                .help("What stage of the extraction to perform.")
                .takes_value(true)
                .possible_values(&[
                    "all",
                    "data",
                    "icons",
                    "extract_data",
                    "transform_data",
                    "extract_icons",
                    "transform_icons",
                ])
                .default_value("all")
                .required(true),
        )
        .arg(
            Arg::with_name("prune_level")
                .long("prune_level")
                .help("The level of pruning of game data to perform during extract_data.")
                .takes_value(true)
                .possible_values(&["0", "1", "2"])
                .default_value("1"),
        )
        .arg(
            Arg::with_name("no_transform_log")
                .long("no_transform_log")
                .help(
                    "Disables printing which entries have been encountered during transform_data.",
                ),
        )
        .arg(
            Arg::with_name("extract_interval")
                .long("extract_interval")
                .help("Time in frames to wait for every icon during extract_icons.")
                .takes_value(true)
                .validator(|value| {
                    value
                        .parse::<u16>()
                        .map_err(|_| "should be a positive integer".to_owned())?;
                    Ok(())
                })
                .default_value("5"),
        )
        .get_matches();

    let directory = app.value_of_os("directory").unwrap();
    let paths = factorio_io::get_factorio_paths(&directory)?;

    let prune_level = match app.value_of("prune_level").unwrap() {
        "0" => PruneLevel::NoPruning,
        "1" => PruneLevel::BasicPruning,
        "2" => PruneLevel::ExtensivePruning,
        _ => unreachable!(),
    };
    let no_transform_log = app.is_present("no_transform_log");
    let extract_interval = app
        .value_of("extract_interval")
        .unwrap()
        .parse::<usize>()
        .unwrap();

    fn to_io_error(err: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }

    match app.value_of("stage").unwrap() {
        "all" => {
            let prototypes = extract_data(&paths, prune_level)?;
            let game_data = transform_data(prototypes, !no_transform_log).map_err(to_io_error)?;
            let icon_directory = extract_icons(&paths, &game_data, extract_interval)?;
            let _icon_directory_temp = TempDirectory::new(&icon_directory);
            let game_data = transform_icons(&paths, &game_data, icon_directory, true)?;
            store_game_data(&paths, &game_data, false)?;
        }
        "data" => {
            let prototypes = extract_data(&paths, prune_level)?;
            let game_data = transform_data(prototypes, !no_transform_log).map_err(to_io_error)?;
            store_game_data(&paths, &game_data, false)?;
        }
        "icons" => {
            let game_data = load_game_data(&paths)?;
            let icon_directory = extract_icons(&paths, &game_data, extract_interval)?;
            let _icon_directory_temp = TempDirectory::new(&icon_directory);
            let game_data = transform_icons(&paths, &game_data, icon_directory, true)?;
            store_game_data(&paths, &game_data, true)?;
        }
        "extract_data" => {
            let prototypes = extract_data(&paths, prune_level)?;
            store_prototypes(&paths, &prototypes)?;
        }
        "transform_data" => {
            let prototypes = load_prototypes(&paths)?;
            let game_data = transform_data(prototypes, !no_transform_log).map_err(to_io_error)?;
            store_game_data(&paths, &game_data, false)?;
        }
        "extract_icons" => {
            let game_data = load_game_data(&paths)?;
            let icon_directory = extract_icons(&paths, &game_data, extract_interval)?;
            println!(
                "extracted icons to: {}",
                icon_directory.as_os_str().to_string_lossy()
            );
        }
        "transform_icons" => {
            let game_data = load_game_data(&paths)?;
            let mut icon_directory = paths.script_output_directory.clone();
            icon_directory.push("graphio_extracted_icons");
            let game_data = transform_icons(&paths, &game_data, icon_directory, false)?;
            store_game_data(&paths, &game_data, true)?;
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn store_prototypes(paths: &FactorioPaths, prototypes: &Vec<String>) -> io::Result<()> {
    let serialized = serde_json::ser::to_string_pretty(&prototypes)?;
    let mut output_dir = TempDirectory::ensure(&paths.script_output_directory)?;
    let output_file = write_file_safely(
        &paths.script_output_directory,
        "prototypes",
        "json",
        serialized.as_bytes(),
    )?;
    output_dir.release();
    println!(
        "stored prototype data to: {}",
        output_file.as_os_str().to_string_lossy()
    );
    Ok(())
}

fn load_prototypes(paths: &FactorioPaths) -> io::Result<Vec<String>> {
    let mut input_file_path = paths.script_output_directory.clone();
    input_file_path.push("prototypes.json");
    println!(
        "loading prototype data from: {}",
        input_file_path.as_os_str().to_string_lossy()
    );
    let input_file = fs::read(input_file_path)?;
    Ok(serde_json::de::from_slice(&input_file)?)
}

fn store_game_data(paths: &FactorioPaths, game_data: &GameData, overwrite: bool) -> io::Result<()> {
    let serialized = serde_json::ser::to_string_pretty(&game_data)?;
    let mut output_dir = TempDirectory::ensure(&paths.script_output_directory)?;
    let output_file = if overwrite {
        let mut path = paths.script_output_directory.clone();
        path.push("game_data.json");
        fs::write(&path, serialized.as_bytes())?;
        path
    } else {
        write_file_safely(
            &paths.script_output_directory,
            "game_data",
            "json",
            serialized.as_bytes(),
        )?
    };
    output_dir.release();
    println!(
        "stored game data to: {}",
        output_file.as_os_str().to_string_lossy()
    );
    Ok(())
}

fn load_game_data(paths: &FactorioPaths) -> io::Result<GameData> {
    let mut input_file_path = paths.script_output_directory.clone();
    input_file_path.push("game_data.json");
    println!(
        "loading prototype data from: {}",
        input_file_path.as_os_str().to_string_lossy()
    );
    let input_file = fs::read(input_file_path)?;
    Ok(serde_json::de::from_slice(&input_file)?)
}

fn extract_data(paths: &FactorioPaths, prune_level: PruneLevel) -> io::Result<Vec<String>> {
    let _scenarios_directory = TempDirectory::ensure(&paths.scenarios_directory)?;

    let scenario_directory = TempDirectory::new(create_dir_safely(
        &paths.scenarios_directory,
        "graphio_exporter",
    )?);
    let scenario_path = scenario_directory.path().clone();
    let scenario_name = scenario_path
        .iter()
        .next_back()
        .unwrap()
        .to_os_string()
        .to_string_lossy()
        .into_owned();

    let mut control_lua_path = scenario_path;
    control_lua_path.push("control.lua");

    let export_script = get_export_script(prune_level);
    fs::write(&control_lua_path, export_script)?;
    let _control_lua_file = TempFile::new(control_lua_path);

    println!("extracting prototypes by running factorio, this may take a while...");

    let output = Command::new(&paths.executable)
        .arg("--scenario2map")
        .arg(&scenario_name)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    let output = String::from_utf8(output.stdout)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        .replace("\r\n", "\n");

    println!("stripping important information...");

    let marker_start = output.find('\x01').ok_or(io::Error::new(
        io::ErrorKind::InvalidData,
        "no start marker in output",
    ))?;
    let marker_end = output.rfind('\x04').ok_or(io::Error::new(
        io::ErrorKind::InvalidData,
        "no end marker in output",
    ))?;

    let output = &output[marker_start + 1..marker_end];
    let lines: Vec<String> = output
        .chars()
        .batching(|it| {
            while let Some(x) = it.next() {
                if x != '\x02' {
                    continue;
                }
                let mut res = String::new();
                while let Some(y) = it.next() {
                    if y == '\x03' {
                        return Some(res);
                    }
                    res.push(y);
                }
                break;
            }
            None
        })
        .collect();

    println!("done");

    Ok(lines)
}

fn get_export_script(prune_level: PruneLevel) -> String {
    const EXPORT_SCRIPT: &'static str = include_str!("export_prototypes.lua");
    let mut export_script = String::with_capacity(EXPORT_SCRIPT.len() + 22);
    export_script.push_str("local prune_level = ");
    export_script.push(match prune_level {
        PruneLevel::NoPruning => '0',
        PruneLevel::BasicPruning => '1',
        PruneLevel::ExtensivePruning => '2',
    });
    export_script.push('\n');
    export_script.push_str(EXPORT_SCRIPT);
    export_script
}

fn transform_data(lines: Vec<String>, log_entries: bool) -> Result<GameData, &'static str> {
    let mut iter = lines.into_iter();

    let (machine_count, beacon_count, recipe_count, item_count, fluid_count) = {
        let lengths = iter.next().ok_or("unexpected end")?;
        let lengths = lengths
            .split('\x1f')
            .map(|entry| entry.parse())
            .collect::<Result<Vec<usize>, _>>()
            .map_err(|_| "cannot read lengths from the first line")?;
        if lengths.len() != 5 {
            return Err("expected 5 lengths on the first line");
        }

        (lengths[0], lengths[1], lengths[2], lengths[3], lengths[4])
    };

    let (items, fluids, recipes, machines, beacons, modules) = {
        use self::data::*;
        use crate::num_traits::identities::Zero;
        use crate::parsing::*;
        let iter = &mut iter;

        // Load primary data (machines, recipes, items, and fluids)

        let mut machines = (0..machine_count)
            .map(|_| {
                let id = MachineID(read_str(iter)?);
                let metadata = read_metadata(iter)?;
                let crafting_speed = read_ratio(iter)?;
                let energy_consumption = read_ratio(iter)?;
                let energy_drain = read_ratio(iter)?;
                let module_slots = read_int(iter)?;

                let allowed_effects = read_allowed_effects(iter)?;

                if log_entries {
                    println!(
                        "machine {} (\"{}\")",
                        id.0.str(),
                        metadata.localised_name.str()
                    );
                }

                Ok((
                    id,
                    (
                        Machine {
                            id: id,
                            metadata,
                            crafting_speed,
                            energy_consumption,
                            energy_drain,
                            module_slots,
                            supported_modules: HashSet::new(),
                        },
                        allowed_effects,
                    ),
                ))
            })
            .collect::<Result<HashMap<_, _>>>()?;
        if machines.len() != machine_count {
            return Err("duplicate machines in exported data set");
        }

        let mut beacons = (0..beacon_count)
            .map(|_| {
                let id = BeaconID(read_str(iter)?);
                let metadata = read_metadata(iter)?;
                let distribution_effectivity = read_ratio(iter)?;
                let allowed_effects = read_allowed_effects(iter)?;

                if log_entries {
                    println!(
                        "beacon {} (\"{}\")",
                        id.0.str(),
                        metadata.localised_name.str()
                    );
                }

                Ok((
                    id,
                    (
                        Beacon {
                            id,
                            metadata,
                            distribution_effectivity,
                            supported_modules: HashSet::new(),
                        },
                        allowed_effects,
                    ),
                ))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        let mut recipes = (0..recipe_count).map(|_| {
            let id = RecipeID(read_str(iter)?);
            let metadata = read_metadata(iter)?;
            let time = read_ratio(iter)?;

            let ingredient_count = read_usize(iter)?;
            let ingredients = (0..ingredient_count).map(|_| {

                let kind = read_line(iter)?;
                let id = read_str(iter)?;
                let amount = read_ratio(iter)?;
                let catalyst_amount = read_ratio(iter)?;

                let resource = match kind.as_str() {
                    "item" => IngredientResource::Item {
                            id: ItemID(id),
                        },
                    "fluid" => {
                        let flags = read_line(iter)?;
                        let flags = flags.as_bytes();
                        if flags.len() != 2 {
                            return Err("expected optional field flags in ingredient fluid to be 2 bits")
                        }
                        let minimum_temperature = match flags[0] {
                            b'0' => None,
                            b'1' => Some(read_ratio(iter)?),
                            _ => return Err("expected optional field flags in ingredient fluid to be 0 or 1"),
                        };
                        let maximum_temperature = match flags[1] {
                            b'0' => None,
                            b'1' => Some(read_ratio(iter)?),
                            _ => return Err("expected optional field flags in ingredient fluid to be 0 or 1"),
                        };
                        IngredientResource::Fluid {
                            id: FluidID(id),
                            minimum_temperature,
                            maximum_temperature,
                        }
                    },
                    _ => return Err("unknown recipe ingredient kind")
                };

                Ok(Ingredient {
                    resource,
                    amount,
                    catalyst_amount,
                })
            })
                .collect::<Result<Vec<_>>>()?;

            let product_count = read_usize(iter)?;
            let products = (0..product_count).map(|_| {
                let kind = read_line(iter)?;
                let id = read_str(iter)?;
                let resource = match kind.as_str() {
                    "item" => ProductResource::Item{ 
                        id: ItemID(id),
                    },
                    "fluid" => ProductResource::Fluid {
                        id: FluidID(id),
                        temperature: read_ratio(iter)?,
                    },
                    _ => return Err("unknown recipe product kind"),
                };

                let kind = read_line(iter)?;
                let amount = match kind.as_str() {
                    "fixed" =>{
                        let amount = read_ratio(iter)?;
                        let catalyst_amount = read_ratio(iter)?;
                        ProductAmount::Fixed {
                            amount,
                            catalyst_amount,
                        }
                    },
                    "probability" => {
                        let amount_min = read_ratio(iter)?;
                        let amount_max = read_ratio(iter)?;
                        let probability = read_ratio(iter)?;
                        ProductAmount::Probability {
                            amount_min,
                            amount_max,
                            probability,
                        }
                    },
                    _ => return Err("unknown recipe product amount kind"),
                };

                Ok(Product {
                    resource,
                    amount,
                })
            }).collect::<Result<Vec<_>>>()?;

            let crafted_in_count = read_usize(iter)?;
            let crafted_in = (0..crafted_in_count)
                .map(|_| Ok(MachineID(read_str(iter)?)))
                .collect::<Result<HashSet<_>>>()?;

            if log_entries {
                println!("recipe {} (\"{}\")",
                    id.str(),
                    metadata.localised_name.str()
                );
            }

            Ok(Recipe {
                id,
                metadata,
                time,
                ingredients,
                products,
                crafted_in,
                supported_modules: HashSet::new(),
            })
        }).collect::<Result<HashSet<Recipe>>>()?;
        if recipes.len() != recipe_count {
            return Err("duplicate recipes in exported data set");
        }

        let mut modules = HashSet::new();

        let items = (0..item_count)
            .map(|_| {
                let id = ItemID(read_str(iter)?);
                let metadata = read_metadata(iter)?;

                let is_module = read_line(iter)?;
                let is_module = match is_module.as_str() {
                    "0" => false,
                    "1" => true,
                    _ => return Err("expected module flag on item to be 0 or 1"),
                };
                if is_module {
                    let modifier_energy = read_ratio(iter)?;
                    let modifier_speed = read_ratio(iter)?;
                    let modifier_productivity = read_ratio(iter)?;
                    let modifier_pollution = read_ratio(iter)?;
                    modules.insert(Module {
                        id,
                        modifier_energy,
                        modifier_speed,
                        modifier_productivity,
                        modifier_pollution,
                    });

                    let has_limitations = read_line(iter)?;
                    let has_limitations = match has_limitations.as_str() {
                        "0" => false,
                        "1" => true,
                        _ => return Err("expected limitations flag on item to be 0 or 1"),
                    };

                    let limitations: HashSet<RecipeID> = if has_limitations {
                        let limitation_count = read_usize(iter)?;
                        (0..limitation_count)
                            .map(|_| Ok(RecipeID(read_str(iter)?)))
                            .collect::<Result<_>>()?
                    } else {
                        recipes.iter().map(|recipe| recipe.id).collect()
                    };

                    for limitation in limitations {
                        let mut recipe = recipes
                            .take(&limitation)
                            .ok_or("module limitation contains non-existent recipe")?;
                        recipe.supported_modules.insert(id);
                        recipes.insert(recipe);
                    }
                }

                if log_entries {
                    println!("item {} (\"{}\")", id.str(), metadata.localised_name.str());
                }

                Ok(Item { id, metadata })
            })
            .collect::<Result<HashSet<_>>>()?;
        if items.len() != item_count {
            return Err("duplicate items in exported data set");
        }

        let fluids = (0..fluid_count)
            .map(|_| {
                let id = FluidID(read_str(iter)?);
                let metadata = read_metadata(iter)?;

                if log_entries {
                    println!("fluid {} (\"{}\")", id.str(), metadata.localised_name.str());
                }

                Ok(Fluid { id, metadata })
            })
            .collect::<Result<HashSet<_>>>()?;
        if fluids.len() != fluid_count {
            return Err("duplicate fluids in exported data set");
        }

        // Combine data
        fn get_allowed_modules(
            modules: &HashSet<Module>,
            allowed_effects: &AllowedEffects,
        ) -> HashSet<ItemID> {
            modules
                .iter()
                .filter(|module| {
                    (allowed_effects.energy || module.modifier_energy.is_zero())
                        && (allowed_effects.speed || module.modifier_speed.is_zero())
                        && (allowed_effects.productivity || module.modifier_productivity.is_zero())
                        && (allowed_effects.pollution || module.modifier_pollution.is_zero())
                })
                .map(|module| module.id)
                .collect()
        }

        for (_, (machine, allowed_effects)) in machines.iter_mut() {
            machine.supported_modules = get_allowed_modules(&modules, allowed_effects);
        }
        for (_, (beacon, allowed_effects)) in beacons.iter_mut() {
            beacon.supported_modules = get_allowed_modules(&modules, allowed_effects);
        }
        let machines = machines
            .into_iter()
            .map(|(_, (machine, _))| machine)
            .collect::<HashSet<Machine>>();
        let beacons = beacons
            .into_iter()
            .map(|(_, (beacon, _))| beacon)
            .collect::<HashSet<Beacon>>();

        (items, fluids, recipes, machines, beacons, modules)
    };

    Ok(GameData {
        tile_metadata: None,
        items,
        fluids,
        recipes,
        machines,
        beacons,
        modules,
    })
}

fn extract_icons(
    paths: &FactorioPaths,
    game_data: &GameData,
    extract_interval: usize,
) -> io::Result<PathBuf> {
    let _scenarios_directory = TempDirectory::ensure(&paths.scenarios_directory)?;
    let scenario_directory = TempDirectory::new(create_dir_safely(
        &paths.scenarios_directory,
        "graphio_extract_icons",
    )?);

    let scenario_path = scenario_directory.path().clone();
    println!(
        "please start a new game with scenario {}",
        scenario_path
            .iter()
            .next_back()
            .unwrap()
            .to_os_string()
            .to_string_lossy()
    );

    let mut script_output_directory = TempDirectory::ensure(&paths.script_output_directory)?;
    let icon_directory = TempDirectory::new(create_dir_safely(
        &paths.script_output_directory,
        "graphio_extracted_icons",
    )?);
    let icon_directory_name = icon_directory
        .path()
        .iter()
        .next_back()
        .unwrap()
        .to_os_string()
        .to_string_lossy()
        .into_owned();

    let extraction_script =
        get_icon_extract_script(&game_data, &icon_directory_name, extract_interval)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

    let mut control_lua_path = scenario_path;
    control_lua_path.push("control.lua");
    fs::write(&control_lua_path, extraction_script.as_bytes())?;
    let _control_lua_file = TempFile::new(control_lua_path);

    let output = Command::new(&paths.executable)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    let output = String::from_utf8(output.stdout)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        .replace("\r\n", "\n");

    if output.find("\x01done\x04").is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "image extract script didn't properly run",
        ));
    }

    script_output_directory.release();
    Ok(icon_directory.release_into())
}

fn get_icon_extract_script(
    game_data: &GameData,
    output_directory_name: &str,
    extract_interval: usize,
) -> Result<String, &'static str> {
    const EXTRACT_IMAGES: &'static str = include_str!("extract_icons.lua");
    let mut extract_script = String::new();

    extract_script.push_str("local output_folder = \'");
    extract_script.push_str(output_directory_name);
    extract_script.push_str("'\nlocal extract_interval = ");
    extract_script.push_str(&extract_interval.to_string());
    extract_script.push_str("\n\n");

    fn bits_4_to_hex_char(b: u8) -> char {
        let b = b & 0x0f;
        (if b < 0xa { b + b'0' } else { b - 0xa + b'a' }) as char
    }
    fn write(out: &mut String, line: &str) -> () {
        out.push_str("        '");
        for b in line.bytes() {
            match b {
                b'\x07' => out.push_str("\\a"),
                b'\x08' => out.push_str("\\b"),
                b'\x0C' => out.push_str("\\f"),
                b'\n' => out.push_str("\\n"),
                b'\r' => out.push_str("\\r"),
                b'\t' => out.push_str("\\t"),
                b'\x0B' => out.push_str("\\v"),
                b'\\' => out.push_str("\\\\"),
                b'\'' => out.push_str("\\'"),
                x if x >= 0x20 && x < 0x7f => out.push(x as char),
                x => {
                    out.push_str("\\x");
                    out.push(bits_4_to_hex_char(x >> 4));
                    out.push(bits_4_to_hex_char(x));
                }
            }
        }
        out.push_str("',\n");
    }

    {
        let extract_script = &mut extract_script;
        extract_script.push_str("local extract_data = {\n    items = {\n");
        let mut any = false;
        for item in &game_data.items {
            any = true;
            write(extract_script, item.id.str());
        }
        extract_script.push_str("    },\n    fluids = {\n");
        for fluid in &game_data.fluids {
            any = true;
            write(extract_script, fluid.id.str());
        }
        extract_script.push_str("    },\n    recipes = {\n");
        for recipe in &game_data.recipes {
            any = true;
            write(extract_script, recipe.id.str());
        }
        extract_script.push_str("    },\n    entities = {\n");
        for id in itertools::chain(
            game_data.machines.iter().map(|machine| machine.id.0),
            game_data.beacons.iter().map(|beacon| beacon.id.0),
        )
        .unique()
        {
            any = true;
            write(extract_script, id.str());
        }
        extract_script.push_str("    },\n}\n\n");
        if !any {
            return Err("game data is empty");
        }
    }

    extract_script.push_str(EXTRACT_IMAGES);
    Ok(extract_script)
}

const TILE_WIDTH: u32 = 32;
const TILE_HEIGHT: u32 = 32;

fn load_image(path: &PathBuf) -> io::Result<image::RgbImage> {
    let image = image::open(path)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        .to_rgb();
    if image.width() != TILE_WIDTH || image.height() != TILE_HEIGHT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "expected image to be 32x32",
        ));
    }
    Ok(image)
}

fn combine_image(dark: image::RgbImage, light: image::RgbImage) -> image::RgbaImage {
    use image::RgbaImage;

    let mut combined = RgbaImage::new(dark.width(), dark.height());
    combined.enumerate_pixels_mut().for_each(|(x, y, pixel)| {
        let d = dark.get_pixel(x, y);
        let l = light.get_pixel(x, y);
        // d = a * rgb
        // l = a * rgb + (1 - a)
        // l - d = 1 - a
        // d - l = a - 1
        // a = d - l + 1
        let d = [
            d.data[0] as f64 / 255f64,
            d.data[1] as f64 / 255f64,
            d.data[2] as f64 / 255f64,
        ];
        let l = [
            l.data[0] as f64 / 255f64,
            l.data[1] as f64 / 255f64,
            l.data[2] as f64 / 255f64,
        ];

        let dr = d[0] - l[0] + 1f64;
        let dg = d[1] - l[1] + 1f64;
        let db = d[2] - l[2] + 1f64;

        // Average the alpha based on the 3 channels
        let a = (dr + dg + db) / 3f64;

        // d = a * rgb
        // rgb = d / a
        let r1 = d[0] / a;
        let g1 = d[1] / a;
        let b1 = d[2] / a;

        // l = a * rgb + (1 - a)
        // l - 1 + a = a * rgb
        // rgb = (l - 1 + a) / a
        //     = (l - 1) / a + 1
        let r2 = (l[0] - 1f64) / a + 1f64;
        let g2 = (l[1] - 1f64) / a + 1f64;
        let b2 = (l[2] - 1f64) / a + 1f64;

        // Average color based on both images
        let r = (r1 + r2) / 2f64;
        let g = (g1 + g2) / 2f64;
        let b = (b1 + b2) / 2f64;

        pixel.data = [
            f64::max(0f64, f64::min(255f64, r * 255f64)).round() as u8,
            f64::max(0f64, f64::min(255f64, g * 255f64)).round() as u8,
            f64::max(0f64, f64::min(255f64, b * 255f64)).round() as u8,
            f64::max(0f64, f64::min(255f64, a * 255f64)).round() as u8,
        ];
    });

    combined
}

fn transform_icons(
    paths: &FactorioPaths,
    game_data: &GameData,
    icon_directory: PathBuf,
    delete_icons: bool,
) -> io::Result<GameData> {
    use self::data::*;

    fn resolve_image<'a, ID: AsRef<Str> + ::std::hash::Hash + Eq>(
        temp_str: &'a mut String,
        dark_path: &'a mut PathBuf,
        light_path: &'a mut PathBuf,
        images: &'a mut HashMap<Vec<u8>, usize>,
        delete_icons: bool,
        iter: impl Iterator<Item = ID>,
    ) -> io::Result<HashMap<ID, usize>> {
        let mut sorted = iter
            .map(|id| {
                let s = id.as_ref().str();
                (id, s)
            })
            .collect::<Vec<(ID, &'static str)>>();
        sorted.sort_by_key(|&(_, s)| s);
        sorted
            .into_iter()
            .map(|(id, s)| {
                temp_str.push_str(s);
                temp_str.push_str(".png");
                light_path.push(&temp_str);
                dark_path.push(&temp_str);
                temp_str.clear();

                let dark_img = load_image(&dark_path)?;
                let light_img = load_image(&light_path)?;

                if delete_icons {
                    let _ = fs::remove_file(&dark_path);
                    let _ = fs::remove_file(&light_path);
                }

                light_path.pop();
                dark_path.pop();

                let image = combine_image(dark_img, light_img);
                let image = image.into_raw();

                let image_count = images.len();
                let index = *images.entry(image).or_insert(image_count);
                Ok((id, index))
            })
            .collect::<io::Result<HashMap<ID, usize>>>()
    }

    println!("loading exported images...");

    // Handle all the image manipulation
    let (tile_metadata, item_icons, fluid_icons, recipe_icons, machine_icons, beacon_icons) = {
        let mut images: HashMap<Vec<u8>, usize> = HashMap::new();
        let mut temp_str = String::new();

        let mut light_path = icon_directory.clone();
        light_path.push("light");
        let mut dark_path = icon_directory;
        dark_path.push("dark");

        light_path.push("items");
        dark_path.push("items");
        let item_icons = resolve_image(
            &mut temp_str,
            &mut dark_path,
            &mut light_path,
            &mut images,
            delete_icons,
            game_data.items.iter().map(|item| item.id),
        )?;
        if delete_icons {
            let _ = fs::remove_dir(&light_path);
            let _ = fs::remove_dir(&dark_path);
        }
        light_path.pop();
        dark_path.pop();

        light_path.push("fluids");
        dark_path.push("fluids");
        let fluid_icons = resolve_image(
            &mut temp_str,
            &mut dark_path,
            &mut light_path,
            &mut images,
            delete_icons,
            game_data.fluids.iter().map(|fluid| fluid.id),
        )?;
        if delete_icons {
            let _ = fs::remove_dir(&light_path);
            let _ = fs::remove_dir(&dark_path);
        }
        light_path.pop();
        dark_path.pop();

        light_path.push("recipes");
        dark_path.push("recipes");
        let recipe_icons = resolve_image(
            &mut temp_str,
            &mut dark_path,
            &mut light_path,
            &mut images,
            delete_icons,
            game_data.recipes.iter().map(|recipe| recipe.id),
        )?;
        if delete_icons {
            let _ = fs::remove_dir(&light_path);
            let _ = fs::remove_dir(&dark_path);
        }
        light_path.pop();
        dark_path.pop();

        light_path.push("entities");
        dark_path.push("entities");
        let machine_icons = resolve_image(
            &mut temp_str,
            &mut dark_path,
            &mut light_path,
            &mut images,
            delete_icons,
            game_data.machines.iter().map(|machine| machine.id),
        )?;
        let beacon_icons = resolve_image(
            &mut temp_str,
            &mut dark_path,
            &mut light_path,
            &mut images,
            delete_icons,
            game_data.beacons.iter().map(|beacon| beacon.id),
        )?;
        if delete_icons {
            let _ = fs::remove_dir(&light_path);
            let _ = fs::remove_dir(&dark_path);
        }
        light_path.pop();
        dark_path.pop();
        if delete_icons {
            let _ = fs::remove_dir(&light_path);
            let _ = fs::remove_dir(&dark_path);
            light_path.pop();
            let _ = fs::remove_dir(light_path);
        }

        let images = {
            let mut buf = Vec::new();
            buf.resize(images.len(), Vec::default());
            for (image, index) in images {
                buf[index] = image;
            }
            buf
        };

        assert!(images.len() != 0);
        println!("combining {} images", images.len());

        let columns = ((images.len() as f64).sqrt().ceil()) as u32;
        let rows = (images.len() as u32 + columns - 1) / columns;

        let target_width = columns * TILE_WIDTH;
        let target_height = rows * TILE_HEIGHT;
        let mut tileset = Vec::new();
        tileset.resize((4 * target_width * target_height) as usize, 0);

        for (index, image) in images.iter().enumerate() {
            let index = index as u32;
            let bx = (index % columns) * TILE_WIDTH;
            let by = (index / columns) * TILE_HEIGHT;
            for y in 0..TILE_HEIGHT {
                for x in 0..TILE_WIDTH {
                    for b in 0..4 {
                        let src = image[((y * TILE_WIDTH + x) * 4 + b) as usize];
                        tileset[(((y + by) * target_width + x + bx) * 4 + b) as usize] = src;
                    }
                }
            }
        }

        use image::*;
        let mut tileset_image = Vec::new();
        DynamicImage::ImageRgba8(
            RgbaImage::from_raw(target_width, target_height, tileset).ok_or(io::Error::new(
                io::ErrorKind::Other,
                "failed to encode image",
            ))?,
        )
        .write_to(&mut tileset_image, ImageFormat::PNG)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let output_file = write_file_safely(
            &paths.script_output_directory,
            "game_icons",
            "png",
            &tileset_image,
        )?;
        println!("output image stored at: {}", output_file.to_string_lossy());

        let tile_metadata = TileMetadata {
            tile_size: (TILE_WIDTH, TILE_HEIGHT),
            tile_count: images.len() as u32,
            image_size: (target_width, target_height),
        };

        (
            tile_metadata,
            item_icons,
            fluid_icons,
            recipe_icons,
            machine_icons,
            beacon_icons,
        )
    };

    let mut game_data = game_data.clone();
    game_data.tile_metadata = Some(tile_metadata);
    game_data
        .modify_metadata::<(), _>(|id, meta| {
            let icon = Some(Icon::new(*match id {
                ID::Item(id) => item_icons.get(&id).unwrap(),
                ID::Fluid(id) => fluid_icons.get(&id).unwrap(),
                ID::Recipe(id) => recipe_icons.get(&id).unwrap(),
                ID::Machine(id) => machine_icons.get(&id).unwrap(),
                ID::Beacon(id) => beacon_icons.get(&id).unwrap(),
            }));
            Ok(Metadata { icon, ..*meta })
        })
        .unwrap();

    Ok(game_data)
}
