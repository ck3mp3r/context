// Test fixture: Visibility modifiers

fn private_fn() {}

pub fn public_fn() {}

pub(crate) fn crate_visible() {}

pub(super) fn super_visible() {}

pub mod visible_mod {}

mod private_mod {}

pub struct PubStruct;

struct PrivStruct;

pub enum PubEnum {
    A,
}

enum PrivEnum {
    B,
}
