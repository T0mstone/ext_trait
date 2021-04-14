# ext_trait

## `ext_trait`
`ext_trait` is a procedural macro which provides you with a shortcut to the [extension trait pattern][1]

[1]: https://github.com/rust-lang/rfcs/blob/master/text/0445-extension-trait-conventions.md
## Examples
Note that IntelliJ Rust really dislikes this macro, i.e. displays errors even if there are none.
This stems from the plugin's linter's limited understanding of procedural macros.

- Simple
```rust
use ext_trait::ext;

// No argument to the macro => A trait name is generated
#[ext]
impl u8 {
    fn foo(self) -> u8 { self + 1 }
}

// this controls the visibility of the generated trait (you can also provide `pub(crate)` etc.)
#[ext(pub)]
impl u16 {
    fn bar(self) -> u16 { self + 2 }
}

assert_eq!(1u8.foo(), 2);
assert_eq!(1u16.bar(), 3);
```

- One large example
```rust
use ext_trait::ext;

macro_rules! foo {
    () => { const BAR: usize = 2; }
}

trait SameType<T> {
    const OK: () = ();
}
impl<T> SameType<T> for T {}

// The argument is the name of the generated trait
#[ext(MyVecU8Ext)] // this trait is private (no `pub`)
impl Vec<u8> {
    const FOO: usize = 1;

    type Foo = usize;

    fn foo(&self) -> usize { 1 }

    // this specific case works as of now
    // but macros are expanded in the trait *and* in the impl
    // which can lead to problems
    foo!();
}

// named ext traits can also be public
#[ext(pub MyVecU16Ext)]
impl Vec<u16> {
    const FOO: usize = 2;
}

let v: Vec<u8> = vec![1, 2, 3];
assert_eq!(Vec::<u8>::FOO, 1);
let _assert_same_type: () = <usize as SameType<<Vec<u8> as MyVecU8Ext>::Foo>>::OK;
assert_eq!(v.foo(), 1);
assert_eq!(Vec::<u8>::BAR, 2);
assert_eq!(Vec::<u16>::FOO, 2);
```

- Generics
```rust
use ext_trait::ext;

#[ext(MyVecExt)]
impl<T: Clone> Vec<T>
where T: Eq, (): Copy
{
    pub fn second(&self) -> Option<&T> {
        self.get(1)
    }
}

let mut v = vec![1];
assert_eq!(v.second(), None);
v.push(2);
assert_eq!(v.second(), Some(&2));
```

## Comparison to similar crates
- [`easy_ext`](https://crates.io/crates/easy-ext) only supports methods and constants, not types and macro invokations; also, the implementation is different
    - to be fair, macro invokations are impossible to fully support with this pattern (as far as I can see)

## Quirks
- The generated trait doesn't retain implicit trait bounds, specifically impls for (implicitly) `Sized` types are not
   converted into traits that require `Self: Sized`
    - Mostly, this leads to no problem since the type is often either explicitly `?Sized` or
        the ext trait only gets implemented for sized types
    - In case of any problems, just add a `where Self: Sized` bound to the impl and all is good (see example below)
- Because the random trait names are created using hashing of the input, there is a tiny chance of a collision.
    - In that case, you can define a macro that expands to nothing and insert it into the impl. That should shake up the hash a bit.

### Example: Fixing `Sized`-Issue
The following code will not compile:
```compile_fail
use std::marker::PhantomData;
use ext_trait::ext;

pub struct AssertSized<T>(PhantomData<T>);

#[ext]
impl<T> T {
    fn foo(self) -> AssertSized<Self> { AssertSized(PhantomData) }
}
```

Fixed code:
```rust
use std::marker::PhantomData;
use ext_trait::ext;

pub struct AssertSized<T>(PhantomData<T>);

#[ext]
impl<T> T
    where Self: Sized
{
    fn foo(self) -> AssertSized<Self> { AssertSized(PhantomData) }
}
```

Alternative Fix (in this case):
```rust
use std::marker::PhantomData;
use ext_trait::ext;

pub struct AssertSized<T>(PhantomData<T>);

#[ext]
impl<T: Sized> T {
    fn foo(self) -> AssertSized<Self> { AssertSized(PhantomData) }
}
```

Note also that something like `#[ext] impl<T> [T] where Self: Sized { … }` will compile, but won't do anything since `[T]` is never `Sized`.
