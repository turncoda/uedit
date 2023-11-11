use clap::Parser;
use std::fs::File;
use std::path::Path;
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
    output: String,

    /// Name of import to disable (set outer_index to zero)
    #[arg(short, long)]
    disabled_imports: Vec<String>
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

    let output_uasset_path = Path::new(&args.output);
    let mut output_uasset_file = File::create(output_uasset_path).unwrap();
    let output_uexp_path = output_uasset_path.with_extension("uexp");
    let mut output_uexp_file = File::create(output_uexp_path).unwrap();

    let input_uasset_name = input_uasset_path.file_stem().unwrap().to_string_lossy().to_string();
    let output_uasset_name = output_uasset_path.file_stem().unwrap().to_string_lossy().to_string();

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
                println!("Updated import: {}: {} -> {}", disabled_import, original_index, import.outer_index.index);
            }
        }
    }

    asset
        .write_data(&mut output_uasset_file, Some(&mut output_uexp_file))
        .unwrap();

}
