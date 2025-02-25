use cmt::template_mod::CommitTemplate;
use schemars::schema_for;

fn main() {
    let schema = schema_for!(CommitTemplate);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
