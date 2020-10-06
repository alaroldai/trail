pub fn select_one(options: Vec<String>) -> Option<String> {
  use std::io::{BufReader, Cursor};

  skim::Skim::run_with(&skim::SkimOptionsBuilder::default().build().unwrap(), {
    let sr = Cursor::new(options.join("\n"));
    let reader = BufReader::new(sr);
    Some(Box::new(reader))
  })
  .and_then(|out| out.selected_items.first().cloned())
  .map(|arc| String::from(arc.get_output_text()))
}
