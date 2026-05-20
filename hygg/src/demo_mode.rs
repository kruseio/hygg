use crate::args::Args;

pub(crate) fn handle_demo_modes(
  args: &Args,
) -> Result<bool, Box<dyn std::error::Error>> {
  if args.list_demos {
    use cli_text_reader::demo_registry::list_all_demos;
    println!("Available demos:");
    for (id, name, description) in list_all_demos() {
      println!("  {id} - {name} : {description}");
    }
    return Ok(true);
  }

  if args.list_components {
    use cli_text_reader::demo_components::list_all_components;
    println!("Available demo components:");
    for component in list_all_components() {
      println!(
        "  {} - {} : {}",
        component.id, component.name, component.description
      );
    }
    return Ok(true);
  }

  if let Some(component_list) = &args.demo_compose {
    println!(
      "Demo composition from command line is not yet fully implemented."
    );
    println!("Components requested: {component_list}");
    println!("Please use predefined demos with --demo <ID>");
    return Ok(true);
  }

  if let Some(demo_id) = args.demo {
    cli_text_reader::run_cli_text_reader_with_demo_id(
      vec![],
      args.col,
      demo_id,
    )?;
    return Ok(true);
  }

  if args.tutorial_demo {
    cli_text_reader::run_cli_text_reader_with_demo(vec![], args.col, true)?;
    return Ok(true);
  }

  Ok(false)
}
