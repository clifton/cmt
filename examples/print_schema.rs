use cmt::template_mod::CommitTemplate;
use rstructor::SchemaType;

fn main() {
    let schema = CommitTemplate::schema();
    println!("{}", serde_json::to_string_pretty(&schema.schema).unwrap());
}
