use clap::Parser;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::path::Path;
use unreal_asset::exports::Export;
use unreal_asset::exports::ExportBaseTrait;
use unreal_asset::exports::ExportNormalTrait;
use unreal_asset::properties::object_property::ObjectProperty;
use unreal_asset::properties::str_property::NameProperty;
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
    disable_import: Vec<String>,

    /// Name of import to rename (syntax: oldname>newname)
    #[arg(short, long)]
    rename_import: Vec<String>,

    /// Name of actor to disable (name may match multiple actors)
    #[arg(long)]
    disable_actor_by_name: Vec<String>,

    /// Index of actor to disable
    #[arg(long)]
    disable_actor_by_index: Vec<String>,

    /// Export index and property to edit (syntax: 42.propname=newvalue)
    #[arg(long)]
    edit_export: Vec<String>,

    /// Print out every import and export in asset
    #[arg(long, default_value_t = false)]
    dump: bool,

    /// Uasset file to extract actors from
    #[arg(long)]
    transplant_donor: Option<String>,

    /// Actor to extract from transplant donor
    #[arg(long)]
    actor_to_transplant: Vec<i32>,
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
            if let Some(normal_export) = export.get_normal_export() {
                for prop in &normal_export.properties {
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

    for disable_import in &args.disable_import {
        for import in &mut asset.imports {
            if &import.object_name.get_owned_content() == disable_import {
                let original_index = import.outer_index.index;
                import.outer_index.index = 0;
                println!(
                    "Updated import: {}: {} -> {}",
                    disable_import, original_index, import.outer_index.index
                );
            }
        }
    }

    for rename_import in &args.rename_import {
        let mut tokens = rename_import.split(">");
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

    let mut actor_indices_to_disable = vec![];
    for actor in &args.disable_actor_by_name {
        for (i, export) in asset.asset_data.exports.iter().enumerate() {
            if export.get_base_export().object_name.get_owned_content() == *actor {
                actor_indices_to_disable.push(i);
            }
        }
    }
    for i in &args.disable_actor_by_index {
        let i = usize::from_str_radix(i, 10).unwrap();
        actor_indices_to_disable.push(i - 1);
    }
    if !actor_indices_to_disable.is_empty() {
        for index in &actor_indices_to_disable {
            let index = PackageIndex::new(*index as i32 + 1);
            println!(
                "Removed actor from PersistentLevel: {}: {}",
                index.index,
                asset
                    .get_export(index)
                    .unwrap()
                    .get_base_export()
                    .object_name
                    .get_owned_content()
            );
        }
        let actor_indices_to_disable: HashSet<i32> = actor_indices_to_disable
            .into_iter()
            .map(|i| i as i32 + 1)
            .collect();
        let persistent_level_index = find_persistent_level_index(&asset).unwrap();
        if let Export::LevelExport(persistent_level) =
            asset.get_export_mut(persistent_level_index).unwrap()
        {
            persistent_level.actors = persistent_level
                .actors
                .clone()
                .into_iter()
                .filter(|i| !actor_indices_to_disable.contains(&i.index))
                .collect();
        } else {
            panic!();
        }
    }

    // split at equal sign and parse left and right side separately
    // e.g. 123.RelativeLocation.RelativeLocation=1,2,3
    // e.g. 123.PlayerStartTag=mycooltag
    for edit_export in &args.edit_export {
        let Some((lhs, rhs)) = edit_export.split_once("=") else {
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
                let x = rhs_fields[0].parse::<f64>().unwrap();
                let y = rhs_fields[1].parse::<f64>().unwrap();
                let z = rhs_fields[2].parse::<f64>().unwrap();
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
            .unwrap();
        let export = export.get_normal_export_mut().unwrap();
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
                    }
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
                    }
                    _ => continue,
                },
            }
        }
        if !found_prop {
            eprintln!("did not find property named '{}'", prop_name);
            panic!();
        }
        println!(
            "Edited export: {}: {}.{} = {}",
            lhs_fields[0],
            export.get_base_export().object_name.get_owned_content(),
            &lhs_fields[1..].join("."),
            rhs
        );
    }

    if let Some(donor_uasset_path) = args.transplant_donor {
        let donor_uasset_path = Path::new(&donor_uasset_path);
        let donor_uasset_file = File::open(donor_uasset_path).unwrap();
        let donor_uexp_path = donor_uasset_path.with_extension("uexp");
        let donor_uexp_file_maybe = File::open(donor_uexp_path).ok();

        let donor_asset = Asset::new(
            donor_uasset_file,
            donor_uexp_file_maybe,
            unreal_asset::engine_version::EngineVersion::VER_UE5_1,
            None,
        )
        .unwrap();

        let persistent_level_index = find_persistent_level_index(&asset).unwrap();
        let donor_persistent_level_index = find_persistent_level_index(&donor_asset).unwrap();
        for root_index in &args.actor_to_transplant {
            let mut exports_to_transplant = vec![];
            let mut export_map = HashMap::new();
            {
                let mut export_stack = vec![*root_index];
                while let Some(cur) = export_stack.pop() {
                    let cur_exp = donor_asset.get_export(PackageIndex::new(cur)).unwrap();
                    exports_to_transplant.push(cur_exp.clone());
                    export_map.insert(cur, exports_to_transplant.len() as i32);
                    for dep in &cur_exp
                        .get_base_export()
                        .create_before_serialization_dependencies
                    {
                        if dep.index < 1 {
                            continue;
                        }
                        if export_map.contains_key(&dep.index) {
                            continue;
                        }
                        export_stack.push(dep.index);
                    }
                }
            }
            // TODO figure out if import already exists and re-use
            let mut imports_to_transplant = vec![];
            let mut import_map = HashMap::new();
            {
                for export in &exports_to_transplant {
                    for dep in export
                        .get_base_export()
                        .create_before_serialization_dependencies
                        .iter()
                        .chain(
                            export
                                .get_base_export()
                                .serialization_before_create_dependencies
                                .iter(),
                        )
                    {
                        if dep.index >= 0 {
                            continue;
                        }
                        if import_map.contains_key(&dep.index) {
                            continue;
                        }
                        let import = donor_asset.get_import(*dep).unwrap();
                        imports_to_transplant.push(import.clone());
                        import_map.insert(dep.index, imports_to_transplant.len() as i32);
                        if import_map.contains_key(&import.outer_index.index) {
                            continue;
                        }
                        let parent_import = donor_asset.get_import(import.outer_index).unwrap();
                        imports_to_transplant.push(parent_import.clone());
                        import_map
                            .insert(import.outer_index.index, imports_to_transplant.len() as i32);
                    }
                }
            }

            let mut export_tuples: Vec<(i32, i32)> = export_map
                .iter()
                .map(|(&k, &v)| (k, v + asset.asset_data.exports.len() as i32))
                .collect();
            let mut import_tuples: Vec<(i32, i32)> = import_map
                .iter()
                .map(|(&k, &v)| (k, -(asset.imports.len() as i32 + v)))
                .collect();

            export_tuples.sort_by_key(|&(_, dst)| dst);
            import_tuples.sort_by_key(|&(_, dst)| dst);

            for &(src, dst) in &export_tuples {
                let name = donor_asset
                    .get_export(PackageIndex::new(src))
                    .unwrap()
                    .get_base_export()
                    .object_name
                    .get_owned_content();
                println!("Transplanting export: {} <- {} \"{}\"", dst, src, name);
            }
            for &(src, dst) in import_tuples.iter().rev() {
                let name = donor_asset
                    .get_import(PackageIndex::new(src))
                    .unwrap()
                    .object_name
                    .get_owned_content();
                println!("Transplanting import: {} <- {} \"{}\"", dst, src, name);
            }

            let export_map: HashMap<i32, i32> = export_tuples.into_iter().collect();
            let import_map: HashMap<i32, i32> = import_tuples.into_iter().collect();

            let expected_combined_size = export_map.len() + import_map.len() + 1;
            let mut combined_map = HashMap::new();
            combined_map.extend(export_map);
            combined_map.extend(import_map);
            combined_map.insert(
                donor_persistent_level_index.index,
                persistent_level_index.index,
            );
            assert_eq!(expected_combined_size, combined_map.len());

            for export in &mut exports_to_transplant {
                let base_export = export.get_base_export_mut();
                base_export.object_name =
                    asset.add_fname(&base_export.object_name.get_owned_content());
                base_export.class_index.index = *combined_map
                    .get(&base_export.class_index.index)
                    .unwrap_or(&base_export.class_index.index);
                base_export.super_index.index = *combined_map
                    .get(&base_export.super_index.index)
                    .unwrap_or(&base_export.super_index.index);
                base_export.template_index.index = *combined_map
                    .get(&base_export.template_index.index)
                    .unwrap_or(&base_export.template_index.index);
                base_export.outer_index.index = *combined_map
                    .get(&base_export.outer_index.index)
                    .unwrap_or(&base_export.outer_index.index);
                for dep in &mut base_export.create_before_serialization_dependencies {
                    dep.index = *combined_map.get(&dep.index).unwrap_or(&dep.index);
                }
                for dep in &mut base_export.serialization_before_create_dependencies {
                    dep.index = *combined_map.get(&dep.index).unwrap_or(&dep.index);
                }
                for dep in &mut base_export.create_before_create_dependencies {
                    dep.index = *combined_map.get(&dep.index).unwrap_or(&dep.index);
                }
                for_each_prop(
                    &mut export.get_normal_export_mut().unwrap().properties,
                    &mut |prop| {
                        match prop {
                            Property::NameProperty(p) => {
                                p.name = asset.add_fname(&p.name.get_owned_content())
                            }
                            Property::ObjectProperty(p) => {
                                p.name = asset.add_fname(&p.name.get_owned_content())
                            }
                            Property::ArrayProperty(p) => {
                                p.name = asset.add_fname(&p.name.get_owned_content())
                            }
                            Property::StructProperty(p) => {
                                p.name = asset.add_fname(&p.name.get_owned_content());
                                // setting struct type is necessary or else unreal_asset fails to parse
                                // it in the dst asset
                                let st = p.struct_type.clone();
                                if p.struct_type.is_some() {
                                    p.struct_type
                                        .replace(asset.add_fname(&st.unwrap().get_owned_content()));
                                }
                            }
                            Property::VectorProperty(p) => {
                                p.name = asset.add_fname(&p.name.get_owned_content())
                            }
                            Property::RotatorProperty(p) => {
                                p.name = asset.add_fname(&p.name.get_owned_content())
                            }
                            Property::ByteProperty(p) => {
                                p.name = asset.add_fname(&p.name.get_owned_content())
                            }
                            Property::FloatProperty(p) => {
                                p.name = asset.add_fname(&p.name.get_owned_content())
                            }
                            Property::IntProperty(p) => {
                                p.name = asset.add_fname(&p.name.get_owned_content())
                            }
                            Property::BoolProperty(p) => {
                                p.name = asset.add_fname(&p.name.get_owned_content())
                            }
                            Property::EnumProperty(p) => {
                                p.name = asset.add_fname(&p.name.get_owned_content());
                                let ev = p.value.clone();
                                if p.value.is_some() {
                                    p.value
                                        .replace(asset.add_fname(&ev.unwrap().get_owned_content()));
                                }
                                // unclear if necessary
                                let et = p.enum_type.clone();
                                if p.enum_type.is_some() {
                                    p.enum_type
                                        .replace(asset.add_fname(&et.unwrap().get_owned_content()));
                                }
                            }
                            Property::MulticastSparseDelegateProperty(p) => {
                                p.name = asset.add_fname(&p.name.get_owned_content())
                            }
                            _ => {
                                print!("unhandled property type: ");
                                dbg!(&prop);
                                panic!();
                            }
                        }
                    },
                );
                for_each_obj_prop(
                    &mut export.get_normal_export_mut().unwrap().properties,
                    &mut |obj_prop| {
                        if obj_prop.value.index != 0 {
                            obj_prop.value.index =
                                *combined_map.get(&obj_prop.value.index).unwrap();
                        }
                    },
                );
                for_each_name_prop(
                    &mut export.get_normal_export_mut().unwrap().properties,
                    &mut |name_prop| {
                        name_prop.value = asset.add_fname(&name_prop.value.get_owned_content());
                        name_prop.name = asset.add_fname(&name_prop.name.get_owned_content());
                    },
                );
            }

            for import in &mut imports_to_transplant {
                import.class_package = asset.add_fname(&import.class_package.get_owned_content());
                import.class_name = asset.add_fname(&import.class_name.get_owned_content());
                import.object_name = asset.add_fname(&import.object_name.get_owned_content());
                if import.outer_index.index != 0 {
                    import.outer_index.index =
                        *combined_map.get(&import.outer_index.index).unwrap();
                }
            }

            if let Export::LevelExport(persistent_level) =
                asset.get_export_mut(persistent_level_index).unwrap()
            {
                let actor_index = PackageIndex::new(*combined_map.get(&root_index).unwrap());
                persistent_level.actors.push(actor_index);
                persistent_level
                    .get_base_export_mut()
                    .create_before_serialization_dependencies
                    .push(actor_index);
            } else {
                panic!();
            }

            asset
                .asset_data
                .exports
                .extend_from_slice(&exports_to_transplant);
            asset.imports.extend_from_slice(&imports_to_transplant);
        }
    }

    asset
        .write_data(&mut output_uasset_file, Some(&mut output_uexp_file))
        .unwrap();
}

fn find_persistent_level_index(asset: &Asset<File>) -> Option<PackageIndex> {
    for (i, export) in asset.asset_data.exports.iter().enumerate() {
        let Export::LevelExport(export) = export else {
            continue;
        };
        if export.get_base_export().object_name.get_owned_content() != "PersistentLevel" {
            continue;
        }
        return Some(PackageIndex::new(i as i32 + 1));
    }
    None
}

fn for_each_prop<F>(props: &mut [Property], f: &mut F)
where
    F: FnMut(&mut Property),
{
    for prop in props.iter_mut() {
        f(prop);
        match prop {
            Property::ArrayProperty(p) => for_each_prop(&mut p.value, f),
            Property::StructProperty(p) => for_each_prop(&mut p.value, f),
            _ => (),
        };
    }
}

fn for_each_obj_prop<F>(props: &mut [Property], f: &mut F)
where
    F: FnMut(&mut ObjectProperty),
{
    for prop in props.iter_mut() {
        match prop {
            Property::ObjectProperty(p) => f(p),
            Property::ArrayProperty(p) => for_each_obj_prop(&mut p.value, f),
            Property::StructProperty(p) => for_each_obj_prop(&mut p.value, f),
            _ => (),
        };
    }
}

fn for_each_name_prop<F>(props: &mut [Property], f: &mut F)
where
    F: FnMut(&mut NameProperty),
{
    for prop in props.iter_mut() {
        match prop {
            Property::NameProperty(p) => f(p),
            Property::ArrayProperty(p) => for_each_name_prop(&mut p.value, f),
            Property::StructProperty(p) => for_each_name_prop(&mut p.value, f),
            _ => (),
        };
    }
}
