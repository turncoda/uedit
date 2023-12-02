use clap::Parser;
use std::fs::File;
use std::path::Path;
use unreal_asset::exports::ExportBaseTrait;
use unreal_asset::exports::ExportNormalTrait;
use unreal_asset::properties::Property;
use unreal_asset::types::PackageIndex;
use unreal_asset::Asset;

/// Edit cooked Unreal Engine assets
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to input uasset file
    #[arg(short, long)]
    input: String,

    /// Path to write modified uasset file
    #[arg(short, long)]
    output: Option<String>,

    /// Name of import to disable (set outer_index to zero)
    #[arg(short, long)]
    disabled_imports: Vec<String>,

    /// Name of import to rename (syntax: oldname>newname)
    #[arg(short, long)]
    renamed_imports: Vec<String>,

    /// Export index and property to edit (syntax: 42.propname=newvalue)
    #[arg(long)]
    edit_export: Vec<String>,

    /// Dump
    #[arg(long, default_value_t = false)]
    dump: bool,
}

enum PropType {
    Vec3,
    Name,
}

#[derive(Debug, Default)]
struct Vec3d {
    x: f64,
    y: f64,
    z: f64,
}

impl std::fmt::Display for Vec3d {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{},{}", self.x, self.y, self.z)
    }
}

fn main() {
    let args = Args::parse();

    let input_uasset_path = Path::new(&args.input);
    let input_uasset_file = File::open(input_uasset_path).unwrap();
    let input_uexp_path = input_uasset_path.with_extension("uexp");
    let input_uexp_file_maybe = File::open(input_uexp_path).ok();

    let mut asset = Asset::new(
        input_uasset_file,
        input_uexp_file_maybe,
        unreal_asset::engine_version::EngineVersion::VER_UE5_1,
        None,
    )
    .unwrap();

    if args.dump {
        for (i, import) in asset.imports.iter().enumerate() {
            println!(
                "{}: {}",
                -(i as i32 + 1),
                import.object_name.get_owned_content()
            );
        }
        for (i, export) in asset.asset_data.exports.iter().enumerate() {
            println!(
                "{}: {}",
                i as i32 + 1,
                export.get_base_export().object_name.get_owned_content()
            );
            for prop in &export.get_normal_export().unwrap().properties {
                match prop {
                    Property::NameProperty(prop) => println!(
                        "  (Name) {} \"{}\"",
                        prop.name.get_owned_content(),
                        prop.value.get_owned_content()
                    ),
                    Property::StructProperty(prop) => {
                        println!("  (Struct) {}", prop.name.get_owned_content());
                        for prop in &prop.value {
                            match prop {
                                Property::VectorProperty(prop) => println!(
                                    "    (Vector) {} {{ {:.2}, {:.2}, {:.2} }}",
                                    prop.name.get_owned_content(),
                                    prop.value.x.0,
                                    prop.value.y.0,
                                    prop.value.z.0
                                ),
                                Property::RotatorProperty(prop) => println!(
                                    "    (Rotator) {} {{ {:.2}, {:.2}, {:.2} }}",
                                    prop.name.get_owned_content(),
                                    prop.value.x.0,
                                    prop.value.y.0,
                                    prop.value.z.0
                                ),
                                _ => (),
                            };
                        }
                    }
                    Property::ObjectProperty(prop) => println!(
                        "  (Object) {} -> {}",
                        prop.name.get_owned_content(),
                        prop.value.index
                    ),
                    _ => (),
                };
            }
        }
        return;
    }

    let output_uasset_path = Path::new(args.output.as_ref().unwrap());
    let mut output_uasset_file = File::create(output_uasset_path).unwrap();
    let output_uexp_path = output_uasset_path.with_extension("uexp");
    let mut output_uexp_file = File::create(output_uexp_path).unwrap();

    let input_uasset_name = input_uasset_path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let output_uasset_name = output_uasset_path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let name_map = asset.get_name_map();
    let num_names = name_map.borrow().get_name_map_index_list().len();
    for i in 0..(num_names as i32) {
        let name_copy = name_map.borrow().get_owned_name(i);
        if name_copy.contains(&input_uasset_name) {
            let mut name_map = name_map.borrow_mut();
            let name_ref = name_map.get_name_reference_mut(i);
            name_ref.clear();
            name_ref.push_str(&name_copy.replace(&input_uasset_name, &output_uasset_name));
            println!("Updated FName: {} -> {}", name_copy, name_ref);
        }
    }

    for disabled_import in &args.disabled_imports {
        for import in &mut asset.imports {
            if &import.object_name.get_owned_content() == disabled_import {
                let original_index = import.outer_index.index;
                import.outer_index.index = 0;
                println!(
                    "Updated import: {}: {} -> {}",
                    disabled_import, original_index, import.outer_index.index
                );
            }
        }
    }

    for renamed_import in &args.renamed_imports {
        let mut tokens = renamed_import.split(">");
        let old_name = tokens.next().unwrap();
        let new_name = tokens.next().unwrap();
        let new_fname = asset.add_fname(new_name);
        let mut import_found = false;
        for import in &mut asset.imports {
            if &import.object_name.get_owned_content() == old_name {
                import.object_name = new_fname;
                import_found = true;
                println!("Renamed import: {} -> {}", old_name, new_name);
                break;
            }
        }
        if !import_found {
            eprintln!("Warning: import '{}' not found", old_name);
        }
    }

    // split at equal sign and parse left and right side separately
    // e.g. 123.RelativeLocation.RelativeLocation=1,2,3
    // e.g. 123.PlayerStartTag=mycooltag
    for edit_export_expr in &args.edit_export {
        let Some((lhs, rhs)) = edit_export_expr.split_once("=") else {
            panic!();
        };
        let lhs_fields: Vec<_> = lhs.split(".").collect();
        let rhs_fields: Vec<_> = rhs.split(",").collect();
        let prop_type = match rhs_fields.len() {
            1 => PropType::Name,
            3 => PropType::Vec3,
            _ => {
                eprintln!("expression on the right of the = has unrecognized format");
                panic!();
            }
        };
        let new_name_value = match prop_type {
            PropType::Name => Some(asset.add_fname(rhs_fields[0])),
            _ => None,
        };
        let new_vec_value = match prop_type {
            PropType::Vec3 => {
                let x = f64::from(i32::from_str_radix(rhs_fields[0], 10).unwrap());
                let y = f64::from(i32::from_str_radix(rhs_fields[1], 10).unwrap());
                let z = f64::from(i32::from_str_radix(rhs_fields[2], 10).unwrap());
                Some(Vec3d { x, y, z })
            }
            _ => None,
        };

        assert!(
            lhs_fields.len() == 2 || lhs_fields.len() == 3,
            "there must be 2-3 fields in the LHS"
        );
        let Ok(export_index) = i32::from_str_radix(lhs_fields[0], 10) else {
            eprintln!("first field of LHS should be the export index");
            panic!();
        };

        let export = asset
            .get_export_mut(PackageIndex::new(export_index))
            .unwrap()
            .get_normal_export_mut()
            .unwrap();
        let mut props = &mut export.properties;
        let mut prop_name = lhs_fields[1];
        if lhs_fields.len() == 3 {
            let mut new_props: Option<&mut Vec<Property>> = None;
            for prop in &mut export.properties {
                let Property::StructProperty(struct_prop) = prop else {
                    continue;
                };
                if struct_prop.name.get_owned_content() != lhs_fields[1] {
                    continue;
                }
                new_props.replace(&mut struct_prop.value);
                break;
            }
            let Some(v_mut) = new_props else {
                eprintln!("did not find struct property named '{}'", lhs_fields[1]);
                panic!();
            };
            props = v_mut;
            prop_name = lhs_fields[2];
        }
        let mut found_prop = false;
        for prop in props {
            match prop_type {
                PropType::Name => {
                    let Property::NameProperty(name_prop) = prop else {
                        continue;
                    };
                    if name_prop.name.get_owned_content() != prop_name {
                        continue;
                    }
                    found_prop = true;
                    name_prop.value = new_name_value.unwrap();
                    break;
                }
                PropType::Vec3 => match prop {
                    Property::RotatorProperty(prop) => {
                        if prop.name.get_owned_content() != prop_name {
                            continue;
                        }
                        found_prop = true;
                        let v = new_vec_value.unwrap();
                        prop.value.x.0 = v.x;
                        prop.value.y.0 = v.y;
                        prop.value.z.0 = v.z;
                        break;
                    },
                    Property::VectorProperty(prop) => {
                        if prop.name.get_owned_content() != prop_name {
                            continue;
                        }
                        found_prop = true;
                        let v = new_vec_value.unwrap();
                        prop.value.x.0 = v.x;
                        prop.value.y.0 = v.y;
                        prop.value.z.0 = v.z;
                        break;
                    },
                    _ => continue,
                },
            }
        }
        if !found_prop {
            eprintln!("did not find property named '{}'", prop_name);
            panic!();
        }
    }

    asset
        .write_data(&mut output_uasset_file, Some(&mut output_uexp_file))
        .unwrap();
}
