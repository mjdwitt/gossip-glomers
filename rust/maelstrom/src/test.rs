use ctor::ctor;

#[ctor]
fn setup() {
    runtime::setup().unwrap();
}
