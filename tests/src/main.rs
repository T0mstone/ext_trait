//! Tests that work if they compile
//!
//! This way (instead of doctests) has the advantage of easier `cargo expand`ability

use ext_trait::ext;
use std::marker::PhantomData;

#[ext]
impl<T> T {
    fn foo() {}
}

macro_rules! nothing {
    () => {};
}

#[ext(pub)]
impl<T> T {
    nothing!();
    fn foo() {}
}

#[ext(GenericExtTrait)]
impl<T> T {
    fn bar() {}
}

pub trait A {}

pub struct AssertTrait<T: ?Sized + A>(PhantomData<T>);

#[ext(pub GenericCool)]
impl<T: A> T {
    type X = AssertTrait<T>;
    type Y = AssertTrait<Self>;
}

// should fail
// #[ext(pub A B)]
// impl<T> T {}

fn main() {}
