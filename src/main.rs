use ext_trait::ext_trait;

#[ext_trait(A)]
impl<'a, T: Eq> Vec<&'a T> {
    fn foo(&self) -> Option<&'a T> {
        self.first().copied()
    }
}

fn main() {
    let v = Vec::<&()>::new();
    assert_eq!(v.foo(), None);
    let v = vec![&()];
    assert_eq!(v.foo(), Some(&()));
}
