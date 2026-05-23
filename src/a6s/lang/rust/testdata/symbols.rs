// Test fixture: Basic symbol types

fn standalone_function() {}

fn main() {}

pub fn public_function() {}

struct MyStruct {
    field: i32,
}

pub enum MyEnum {
    Variant,
}

trait MyTrait {
    fn method(&self);
}

mod my_module {
    pub fn inner_fn() {}
}

const MAX: usize = 100;

pub static REF: &str = "value";

type Alias = String;

#[test]
fn test_function() {}

#[bench]
fn bench_function(b: &mut Bencher) {}

macro_rules! my_macro {
    () => {};
}
